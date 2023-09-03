use {
    crate::{
        game::{choose_play, Card, Cards, GameState, PlayError, Relative, Seat},
        hx,
        json_seq::JsonSeq,
    },
    async_trait::async_trait,
    axum::{
        debug_handler,
        extract::{FromRequestParts, Query, State},
        response::{
            sse::{self, KeepAlive, Sse},
            IntoResponse, Response,
        },
        routing::{get, post, put},
        Form, Json, RequestPartsExt, Router, TypedHeader,
    },
    axum_core::response::{IntoResponseParts, ResponseParts},
    axum_extra::extract::CookieJar,
    base64::{engine::general_purpose, Engine as _},
    //	axum_server::tls_rustls::RustlsConfig,
    cookie::{Cookie, Expiration, SameSite},
    futures::future::BoxFuture,
    headers::{authorization::Bearer, Authorization, HeaderMapExt},
    html_node::{html, text, Node},
    http::{header::HeaderValue, request::Parts, status::StatusCode, Uri},
    rand::{seq::SliceRandom, thread_rng, Rng},
    serde::{Deserialize, Serialize},
    serde_with::{serde_as, DisplayFromStr, DurationMilliSeconds},
    std::{
        collections::HashMap,
        convert::Infallible,
        fmt::{self, Display},
        future::Future,
        mem,
        net::{Ipv4Addr, SocketAddr},
        ops::{ControlFlow, Deref},
        str::FromStr,
        sync::Arc,
        sync::Weak,
        time::Duration,
    },
    tokio::{
        sync::mpsc::{self, UnboundedSender},
        sync::{Mutex, OwnedMutexGuard},
        task,
        time::{interval, sleep_until, Instant, MissedTickBehavior},
    },
    tokio_stream::wrappers::UnboundedReceiverStream,
    tower_http::{
        services::{ServeDir, ServeFile},
        set_header::SetResponseHeaderLayer,
    },
    uuid::Uuid,
};

const BOT_ACTION_TIMER: Duration = Duration::from_secs(1);
const INACTIVE_BEFORE_DISCONNECT: Duration = Duration::from_secs(2 * 60);

pub fn api<S>() -> Router<S> {
    Router::new()
        .route("/", get(index))
        .route("/api/test", get(test))
        .route("/api/join", post(join))
        .route("/api/subscribe", get(subscribe))
        .route("/api/state", get(state))
        .route("/api/keep_alive", post(keep_alive))
        .route("/api/timer", put(timer))
        .route("/api/start", post(start))
        .route("/api/play", post(play))
        .route("/api/playable", get(playable))
        .with_state(ApiState::new())
}

#[debug_handler]
async fn test() -> impl IntoResponse {
    let (tx, rx) = mpsc::unbounded_channel();
    task::spawn(async move {
        let mut interval = interval(Duration::from_millis(1000));
        for i in 0.. {
            interval.tick().await;
            let event = sse::Event::default().json_data(i).unwrap();
            let event = Ok::<_, Infallible>(event);
            let Ok(_) = tx.send(event) else { break };
        }
    });
    let rx = UnboundedReceiverStream::new(rx);
    Sse::new(rx).keep_alive(KeepAlive::default())
}

#[debug_handler(state=Arc<Mutex<ApiState>>)]
async fn index() -> Result<impl IntoResponse> {
    Ok(html! {
        <!DOCTYPE html>
        <html lang="en-US">
            <head>
                <title>Pusoy</title>
                <meta charset="utf-8">
                <base href=".">
                <link rel="stylesheet" href="/index.css">
                <meta name="viewport" content="width=device-width, initial-scale=1" />
                <script src="/assets/htmx-1.9.5.min.js"></script>
                <script src="/assets/htmx-sse-1.9.5.js"></script>
            </head>
            <body>
                <main hx-post="/api/join" hx-trigger="load" hx-swap="outerHTML"></main>
            </body>
        </html>
    })
}

#[debug_handler(state=Arc<Mutex<ApiState>>)]
async fn join(
    State(state): State<Arc<Mutex<ApiState>>>,
    TypedHeader(hx::CurrentUrl(mut url)): TypedHeader<hx::CurrentUrl>,
) -> Result<impl IntoResponse> {
    let session_id = url
        .query_pairs()
        .find_map(|(k, v)| (k == "session_id").then_some(v));
    let session_id = session_id.and_then(|session_id| SessionId::from_str(&session_id).ok());
    let (auth, session_id) = state.lock().await.join(session_id).await?;

    url.query_pairs_mut()
        .clear()
        .append_pair("session_id", &format!("{}", session_id));

    let html = html! {
        <main hx-ext="sse" sse-connect="/api/events" hx-trigger="sse:update" hx-get="/api/state" hx-swap="innerHTML">
        </main>
    };

    Ok((*auth, TypedHeader(hx::ReplaceUrl(url)), html))
}

#[debug_handler(state=Arc<Mutex<ApiState>>)]
async fn keep_alive(mut user_session: UserSession<Authenticated>) -> Result<impl IntoResponse> {
    user_session.session.keep_alive(user_session.auth)
}

#[debug_handler(state=Arc<Mutex<ApiState>>)]
async fn subscribe(mut user_session: UserSession<Authenticated>) -> Result<impl IntoResponse> {
    let (tx, rx) = mpsc::unbounded_channel();
    user_session
        .session
        .subscribe(user_session.auth, tx)
        .await?;
    let rx = UnboundedReceiverStream::new(rx);
    Ok(Sse::new(rx).keep_alive(KeepAlive::default()))
}

#[debug_handler(state=Arc<Mutex<ApiState>>)]
async fn state(mut user_session: UserSession<Authenticated>) -> Result<impl IntoResponse> {
    user_session.session.state(user_session.auth).await?;
    Ok(())
}

#[serde_as]
#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
struct TimerRequest {
    #[serde_as(as = "DurationMilliSeconds<u64>")]
    timer_value: Duration,
    enable_timer: Option<String>,
}

#[debug_handler(state=Arc<Mutex<ApiState>>)]
async fn timer(
    mut user_session: UserSession<HostAuthenticated>,
    Form(timer_request): Form<TimerRequest>,
) -> Result<impl IntoResponse> {
    let timer = timer_request
        .enable_timer
        .map(|_| timer_request.timer_value);
    user_session.session.timer(user_session.auth, timer).await
}

#[debug_handler(state=Arc<Mutex<ApiState>>)]
async fn start(
    mut user_session: UserSession<HostAuthenticated>,
    Form(_timer_request): Form<TimerRequest>,
) -> Result<impl IntoResponse> {
    user_session.session.start(user_session.auth).await
}

#[debug_handler(state=Arc<Mutex<ApiState>>)]
async fn playable(
    mut user_session: UserSession<Authenticated>,
    Form(play_request): Form<Vec<(Card, Card)>>,
) -> Result<impl IntoResponse> {
    let cards = Cards::from_iter(play_request.into_iter().map(|(card, _)| card));
    user_session
        .session
        .playable(user_session.auth, cards)
        .await
}

#[debug_handler(state=Arc<Mutex<ApiState>>)]
async fn play(
    mut user_session: UserSession<Authenticated>,
    Form(play_request): Form<Vec<(Card, Card)>>,
) -> Result<impl IntoResponse> {
    let cards = Cards::from_iter(play_request.into_iter().map(|(card, _)| card));
    user_session
        .session
        .human_play(user_session.auth, cards)
        .await
}

struct ApiState {
    sessions: HashMap<SessionId, Arc<Mutex<Session>>>,
    this: Weak<Mutex<ApiState>>,
}

impl ApiState {
    fn new() -> Arc<Mutex<Self>> {
        Arc::new_cyclic(|this| {
            Mutex::new(Self {
                sessions: HashMap::default(),
                this: Weak::clone(this),
            })
        })
    }

    async fn join(&mut self, session_id: Option<SessionId>) -> Result<(Authenticated, SessionId)> {
        let (session_id, session) = match session_id {
            Some(session_id) => (session_id, self.get_session(session_id).await?),
            None => {
                let session_id = SessionId::random();
                let session = Session::new(session_id);
                self.sessions.insert(session_id, Arc::clone(&session));
                (session_id, session)
            }
        };

        let auth = session.lock().await.join().await?;

        let session = Arc::downgrade(&session);
        let this = Weak::clone(&self.this);
        task::spawn(async move {
            let mut interval = interval(INACTIVE_BEFORE_DISCONNECT);
            interval.set_missed_tick_behavior(MissedTickBehavior::Skip);

            loop {
                interval.tick().await;
                let Some(session) = session.upgrade() else {
                    break;
                };
                let mut session = session.lock().await;
                match session.inactive_disconnect(auth).await {
                    ControlFlow::Continue(()) => {}
                    ControlFlow::Break(true) => {
                        let this = this
                            .upgrade()
                            .expect("ApiState should persist the lifetime of the program");
                        let mut this = this.lock().await;
                        this.sessions.remove(&session_id);
                        break;
                    }
                    ControlFlow::Break(false) => break,
                }
            }
        });

        Ok((auth, session_id))
    }

    async fn get_session(&self, session_id: SessionId) -> Result<Arc<Mutex<Session>>> {
        let session = self.sessions.get(&session_id).ok_or(Error::NoSession)?;
        Ok(Arc::clone(session))
    }
}

struct Session {
    timer: Option<Duration>,
    deadline: Option<Instant>,
    phase: Phase,
    game_state: GameState,
    humans: HashMap<Seat, Human>,
    session_id: SessionId,
    this: Weak<Mutex<Session>>,
}

impl Session {
    pub fn new(session_id: SessionId) -> Arc<Mutex<Self>> {
        Arc::new_cyclic(move |this| {
            Mutex::new(Self {
                timer: None,
                deadline: None,
                phase: Phase::Lobby,
                game_state: GameState::new(),
                humans: HashMap::default(),
                session_id,
                this: Weak::clone(this),
            })
        })
    }

    pub async fn join(&mut self) -> Result<Authenticated> {
        if !matches!(self.phase, Phase::Lobby) {
            return Err(Error::BadPhase);
        }

        let seat = Seat::ALL
            .into_iter()
            .find(|seat| !self.humans.contains_key(seat))
            .ok_or(Error::Full)?;
        let user_secret = UserSecret::random();
        let host = self.humans.is_empty();
        self.humans.insert(
            seat,
            Human {
                host,
                user_secret,
                last_active: Instant::now(),
                tx: None,
            },
        );

        self.alert_all(&Update::Connected).await;

        Ok(Authenticated {
            auth: Unauthenticated {
                user_secret,
                seat,
                session_id: self.session_id,
            },
        })
    }

    pub async fn inactive_disconnect(&mut self, auth: Authenticated) -> ControlFlow<bool> {
        let Some(human) = self.humans.get_mut(&auth.seat) else {
            return ControlFlow::Break(false);
        };
        let is_active =
            Instant::now().duration_since(human.last_active) < INACTIVE_BEFORE_DISCONNECT;
        if is_active {
            return ControlFlow::Continue(());
        }
        if human.host {
            let new_host = self.humans.values_mut().next();
            if let Some(new_host) = new_host {
                new_host.host = true;
            }
        }
        let now_empty = self.humans.is_empty();

        self.alert_all(&Update::Connected).await;
        if matches!(self.phase, Phase::Active) && self.game_state.current_player() == auth.seat {
            self.solicit().await;
        }
        ControlFlow::Break(now_empty)
    }

    pub fn keep_alive(&mut self, auth: Authenticated) -> Result<()> {
        self.humans
            .get_mut(&auth.seat)
            .ok_or(Error::Absent)?
            .last_active = Instant::now();
        Ok(())
    }

    pub async fn subscribe(&mut self, auth: Authenticated, tx: Tx) -> Result<()> {
        self.humans.get_mut(&auth.seat).ok_or(Error::Absent)?.tx = Some(tx);
        self.alert(auth.seat, &Update::Deal).await;
        Ok(())
    }

    pub fn is_host(&self, seat: Seat) -> bool {
        self.humans.get(&seat).is_some_and(|human| human.host)
    }
    pub fn host_authenticate(&mut self, auth: Authenticated) -> Result<HostAuthenticated> {
        if !self.is_host(auth.seat) {
            return Err(Error::NotHost);
        }
        Ok(HostAuthenticated { auth })
    }

    pub fn authenticate(&mut self, auth: Unauthenticated) -> Result<Authenticated> {
        let human = self.humans.get(&auth.seat).ok_or(Error::Absent)?;
        if human.user_secret != auth.user_secret {
            return Err(Error::BadAuthentication);
        }
        Ok(Authenticated { auth })
    }

    pub async fn state(&mut self, auth: Authenticated) -> Result<Node> {
        Ok(html! {
            <img class="table" alt="" />
            <section class="table">
            <div class="cards"></div>
            </section>
            { Relative::ALL
                .into_iter()
                .map(|relative| {

                    let seat = auth.seat.relative(relative);
                    let is_hosting = self.is_host(seat);
                    let my_player = seat == auth.seat;

                    let host_controls = if my_player && is_hosting {
                        html! {
                            <form hx-put="/api/timer">
                                <label class="host-config">
                                    "enable action timer"
                                    <input type="checkbox" name="enable-timer" hx-trigger="change" />
                                </label>
                                <input type="range" min="1" max="120000" value="30000" id="set-timer" name="timer-value" hx-trigger="change" />
                            </form>
                            <button hx-post="/api/start">"start the game"</button>
                        }
                    } else {
                        html! { <div>"no host controls"</div> }
                    };

                    let cards_form = if my_player {
                        let cards = self.game_state.hand(seat);
                        html! {
                            <form>
                                { cards.into_iter()
                                    .map(|card| {
                                        let card = format!("{}", card);
                                        html! {
                                            <label class="card" data-card={&card} id={&card}>
                                                <input type="checkbox" name={&card} hx-get="/api/playable" hx-trigger="change">
                                                <img src={format!("/assets/cards/{card}.svg")} alt="" class="card-face">
                                            </label>
                                        }
                                    })
                                }
                                <button type="submit" hx-post="/api/play">"Play"</button>
                            </form>
                        }
                    } else {
                        html! { <div>"no cards form"</div> }
                    };

                    html! {
                        <section class=format!("{}", relative)>
                            <h2 class="name">
                            { host_controls }
                            </h2>
                            <div class="load">
                            </div>
                            <div class="bot">
                            </div>
                            <div class="timer">
                            </div>
                            <div class="turn">
                            </div>
                            <div class="passed">
                            </div>
                            <div class="control">
                            </div>
                            <div class="win">
                            </div>
                            { cards_form }
                        </section>
                    }
                })
            }
        })
    }

    pub async fn timer(&mut self, _auth: HostAuthenticated, timer: Option<Duration>) -> Result<()> {
        if !matches!(self.phase, Phase::Lobby) {
            return Err(Error::BadPhase);
        }
        // TODO: maybe send update
        self.timer = timer;
        Ok(())
    }

    pub async fn start(&mut self, _auth: HostAuthenticated) -> Result<()> {
        if !matches!(self.phase, Phase::Lobby) {
            return Err(Error::BadPhase);
        }
        self.phase = Phase::Active;
        self.alert_all(&Update::Deal).await;
        self.solicit().await;

        Ok(())
    }

    pub async fn playable(&mut self, auth: Authenticated, cards: Cards) -> Result<()> {
        if !matches!(self.phase, Phase::Active) {
            return Err(Error::BadPhase);
        }
        let current_player = self.game_state.current_player();
        if auth.seat != current_player {
            return Err(Error::NotCurrent);
        }
        self.game_state.playable(cards)?;
        Ok(())
    }

    pub async fn human_play(&mut self, auth: Authenticated, cards: Cards) -> Result<()> {
        if !matches!(self.phase, Phase::Active) {
            return Err(Error::BadPhase);
        }
        let current_player = self.game_state.current_player();
        if auth.seat != current_player {
            return Err(Error::NotCurrent);
        }
        self.play(cards).await?;
        Ok(())
    }

    async fn play(&mut self, cards: Cards) -> Result<()> {
        assert_eq!(self.phase, Phase::Active);
        self.game_state.play(cards)?;
        self.alert_all(&Update::Play).await;

        let win = self.game_state.winning_player().is_some();
        if win {
            self.phase = Phase::Post;
            self.alert_all(&Update::Win).await;
        } else {
            self.solicit().await;
        }

        Ok(())
    }

    async fn solicit(&mut self) {
        let current_player = self.game_state.current_player();
        let timer = if self.is_human(current_player) {
            self.timer
        } else {
            Some(BOT_ACTION_TIMER)
        };

        self.deadline = timer.and_then(|timer| Instant::now().checked_add(timer));

        self.alert_all(&Update::Turn).await;

        let this = Weak::clone(&self.this);
        task::spawn(Self::force_play(this));
    }

    // BoxFuture to break recursion
    fn force_play(this: Weak<Mutex<Self>>) -> BoxFuture<'static, ()> {
        Box::pin(async move {
            let Some(deadline) = this.upgrade() else {
                return;
            };
            let Some(deadline) = deadline.lock().await.deadline else {
                return;
            };
            sleep_until(deadline).await;

            let Some(this) = this.upgrade() else { return };
            let mut this = this.lock().await;
            let Some(deadline) = this.deadline else {
                return;
            };
            if Instant::now() < deadline {
                return;
            }
            // TODO: can choose a worse bot here if we're forcing a human player
            if !matches!(this.phase, Phase::Active) {
                return;
            }
            let cards = choose_play(&this.game_state).cards;
            this.play(cards)
                .await
                .expect("our bots should always choose valid plays");
        })
    }

    async fn alert(&self, seat: Seat, message: &Update) {
        let Some(tx) = self.humans.get(&seat).and_then(|human| human.tx.as_ref()) else {
            return;
        };
        let data = html! { <div>{text!("{}", message)}</div> };
        let event = sse::Event::default().data(data.to_string()).event("update"); // TODO: change
        let _ = tx.send(Ok(event));
    }

    async fn alert_all(&mut self, message: &Update) {
        for seat in Seat::ALL {
            self.alert(seat, message).await;
        }
    }

    fn is_human(&self, seat: Seat) -> bool {
        self.humans.contains_key(&seat)
    }
}

#[derive(Clone, Copy, Hash, Eq, PartialEq, PartialOrd, Ord, Debug, Serialize, Deserialize)]
enum Phase {
    Lobby,
    Active,
    Post,
}

struct Human {
    user_secret: UserSecret,
    last_active: Instant,
    host: bool,
    tx: Option<Tx>,
}

type Tx = UnboundedSender<Result<sse::Event, Infallible>>;

#[derive(Serialize)]
enum Update {
    All,
    Welcome,
    Deal,
    Turn,
    Host,
    Connected,
    Play,
    Win,
}

impl Display for Update {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::All => write!(f, "all"),
            Self::Welcome => write!(f, "welcome"),
            Self::Host => write!(f, "host"),
            Self::Connected => write!(f, "connected"),
            Self::Deal => write!(f, "deal"),
            Self::Play => write!(f, "play"),
            Self::Turn => write!(f, "solicit"),
            Self::Win => write!(f, "win"),
        }
    }
}

struct UserSession<A> {
    session: OwnedMutexGuard<Session>,
    auth: A,
}

#[async_trait]
impl FromRequestParts<Arc<Mutex<ApiState>>> for UserSession<HostAuthenticated> {
    type Rejection = Response;
    async fn from_request_parts(
        parts: &mut Parts,
        state: &Arc<Mutex<ApiState>>,
    ) -> Result<Self, Self::Rejection> {
        let UserSession { mut session, auth } = parts
            .extract_with_state::<UserSession<Authenticated>, Arc<Mutex<ApiState>>>(state)
            .await
            .map_err(|err| err.into_response())?;
        let auth = session
            .host_authenticate(auth)
            .map_err(|err| err.into_response())?;
        Ok(UserSession { session, auth })
    }
}

#[async_trait]
impl FromRequestParts<Arc<Mutex<ApiState>>> for UserSession<Authenticated> {
    type Rejection = Response;
    async fn from_request_parts(
        parts: &mut Parts,
        state: &Arc<Mutex<ApiState>>,
    ) -> Result<Self, Self::Rejection> {
        let UserSession { mut session, auth } = parts
            .extract_with_state::<UserSession<Unauthenticated>, Arc<Mutex<ApiState>>>(state)
            .await
            .map_err(|err| err.into_response())?;
        let auth = session
            .authenticate(auth)
            .map_err(|err| err.into_response())?;
        Ok(UserSession { session, auth })
    }
}

#[async_trait]
impl FromRequestParts<Arc<Mutex<ApiState>>> for UserSession<Unauthenticated> {
    type Rejection = Response;
    async fn from_request_parts(
        parts: &mut Parts,
        state: &Arc<Mutex<ApiState>>,
    ) -> Result<Self, Self::Rejection> {
        let jar = parts
            .extract::<CookieJar>()
            .await
            .map_err(|err| err.into_response())?;
        let auth = jar
            .get("auth")
            .ok_or_else(|| (StatusCode::UNAUTHORIZED, "no auth cookie found").into_response())?
            .value();
        let auth: Unauthenticated = serde_json::from_str(auth)
            .map_err(|err| (StatusCode::UNAUTHORIZED, format!("{}", err)).into_response())?;
        let session = state
            .lock()
            .await
            .get_session(auth.session_id)
            .await
            .map_err(|err| err.into_response())?
            .lock_owned()
            .await;
        Ok(UserSession { session, auth })
    }
}

#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
struct HostAuthenticated {
    auth: Authenticated,
}

impl HostAuthenticated {
    fn auth(&self) -> Authenticated {
        self.auth
    }
}

impl Deref for HostAuthenticated {
    type Target = Authenticated;
    fn deref(&self) -> &Self::Target {
        &self.auth
    }
}

#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
struct Authenticated {
    auth: Unauthenticated,
}

impl Deref for Authenticated {
    type Target = Unauthenticated;
    fn deref(&self) -> &Self::Target {
        &self.auth
    }
}

#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
struct Unauthenticated {
    pub seat: Seat,
    pub session_id: SessionId,
    pub user_secret: UserSecret,
}

impl IntoResponseParts for Unauthenticated {
    type Error = Infallible;
    fn into_response_parts(self, res: ResponseParts) -> Result<ResponseParts, Self::Error> {
        let auth = serde_json::to_string(&self).unwrap();
        let mut cookie = Cookie::new("auth", auth);
        cookie.set_secure(Some(true));
        cookie.set_http_only(Some(true));
        cookie.set_same_site(Some(SameSite::Strict));
        cookie.set_expires(Expiration::Session);
        let jar = CookieJar::new().add(cookie);
        jar.into_response_parts(res)
    }
}

#[async_trait]
impl<S> FromRequestParts<S> for Unauthenticated
where
    S: Send + Sync,
{
    type Rejection = Response;
    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let jar = parts
            .extract::<CookieJar>()
            .await
            .map_err(|err| err.into_response())?;
        let auth = jar
            .get("auth")
            .ok_or_else(|| (StatusCode::UNAUTHORIZED, "no auth cookie found").into_response())?
            .value();
        let auth = serde_json::from_str(auth)
            .map_err(|err| (StatusCode::UNAUTHORIZED, format!("{}", err)).into_response())?;
        Ok(auth)
    }
}

#[derive(Default, Copy, Clone, Serialize, Deserialize, PartialEq, Eq, Debug, Hash)]
struct SessionId(Uuid);

impl SessionId {
    fn random() -> Self {
        let ret = thread_rng().gen();
        let ret = uuid::Builder::from_random_bytes(ret).into_uuid();
        Self(ret)
    }
}

impl Display for SessionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for SessionId {
    type Err = uuid::Error;
    fn from_str(string: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::from_str(string)?))
    }
}

#[derive(Default, Copy, Clone, Serialize, Deserialize, PartialEq, Eq, Debug, Hash)]
struct UserSecret(Uuid);

impl UserSecret {
    fn random() -> Self {
        let ret = thread_rng().gen();
        let ret = uuid::Builder::from_random_bytes(ret).into_uuid();
        Self(ret)
    }
}

type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug)]
enum Error {
    BadAuthentication,
    NoSession,
    Absent,
    BadPhase,
    NotHost,
    Full,
    NotCurrent,
    PlayError(PlayError),
}

impl From<PlayError> for Error {
    fn from(error: PlayError) -> Error {
        Self::PlayError(error)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BadAuthentication => write!(f, "authentication not valid for the player"),
            Self::NoSession => write!(f, "request session not found"),
            Self::Absent => write!(f, "player must be present in the game"),
            Self::BadPhase => write!(f, "request must be applicable to current phase"),
            Self::NotHost => write!(f, "requests must have from host&"),
            Self::Full => write!(f, "can only join sessions that aren't full"),
            Self::NotCurrent => write!(f, "this request should be made by the current player"),
            Self::PlayError(error) => write!(f, "{}", error),
        }
    }
}

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        let status = match self {
            Self::BadAuthentication => StatusCode::UNAUTHORIZED,
            Self::NoSession => StatusCode::BAD_REQUEST,
            Self::Absent => StatusCode::BAD_REQUEST,
            Self::BadPhase => StatusCode::BAD_REQUEST,
            Self::NotHost => StatusCode::FORBIDDEN,
            Self::Full => StatusCode::BAD_REQUEST,
            Self::NotCurrent => StatusCode::BAD_REQUEST,
            Self::PlayError(..) => StatusCode::BAD_REQUEST,
        };
        let body = self.to_string();
        (status, body).into_response()
    }
}
