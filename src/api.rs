use {
    crate::{
        game::{choose_play, Card, Cards, GameState, Play, PlayError, Relative, Seat, SeatMap},
        hx,
    },
    async_trait::async_trait,
    axum::{
        debug_handler,
        extract::{FromRequestParts, State},
        response::{
            sse::{self, KeepAlive, Sse},
            IntoResponse, Response,
        },
        routing::{get, post, put},
        Form, RequestPartsExt, Router, TypedHeader,
    },
    axum_core::response::{IntoResponseParts, ResponseParts},
    axum_extra::extract::CookieJar,
    cookie::{Cookie, Expiration, SameSite},
    futures::future::BoxFuture,
    html_node::{html, text, Node},
    http::{request::Parts, status::StatusCode},
    rand::{seq::SliceRandom, thread_rng, Rng},
    serde::{Deserialize, Serialize},
    serde_with::{serde_as, DurationMilliSeconds},
    std::{
        collections::HashMap,
        convert::Infallible,
        fmt::{self, Display},
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
    tokio_stream::{wrappers::UnboundedReceiverStream, StreamExt},
    uuid::Uuid,
};

const BOT_ACTION_TIMER: Duration = Duration::from_secs(1);
const INACTIVE_BEFORE_DISCONNECT: Duration = Duration::from_secs(60);

pub fn api<S>() -> Router<S> {
    Router::new()
        .route("/connect", post(connect))
        .route("/subscribe", get(subscribe))
        .route("/state", get(state))
        .route("/keep-alive", post(keep_alive))
        .route("/timer", put(timer))
        .route("/start", post(start))
        .route("/play", post(play))
        .route("/playable", post(playable))
        .with_state(ApiState::new())
}

#[debug_handler(state=Arc<Mutex<ApiState>>)]
async fn connect(
    State(state): State<Arc<Mutex<ApiState>>>,
    user_session: Option<UserSession<Authenticated>>,
    TypedHeader(hx::CurrentUrl(mut url)): TypedHeader<hx::CurrentUrl>,
) -> Result<impl IntoResponse> {
    let url_session_id = url
        .query_pairs()
        .find_map(|(k, v)| (k == "session_id").then_some(v))
        .and_then(|url_session_id| SessionId::from_str(&url_session_id).ok());

    // if the cookie session agrees with the session in the url, don't do anything
    // if they don't agree, or there is no cookie session, try to join the one in the url
    // if there is no session at all, join a new game
    let mut state = state.lock().await;
    let (auth, reconnect_reason) = match (user_session, url_session_id) {
        (Some(user_session), Some(url_session_id))
            if user_session.auth.session_id == url_session_id =>
        {
            (user_session.auth, None)
        }
        (_, Some(url_session_id)) => match state.connect_existing(url_session_id).await {
            Ok(auth) => (auth, None),
            Err(reconnect_reason) => {
                let auth = state.connect_new().await?;
                (auth, Some(reconnect_reason))
            }
        },
        (_, None) => {
            let auth = state.connect_new().await?;
            (auth, None)
        }
    };

    url.query_pairs_mut()
        .clear()
        .append_pair("session_id", &format!("{}", auth.session_id));

    let reconnect_reason = if let Some(reconnect_reason) = reconnect_reason {
        text!("{}", reconnect_reason)
    } else {
        text!("")
    };
    let html = html! {
        <div hx-trigger="every 30s" hx-post="./api/keep-alive" hx-swap="none"></div>
        <div class="scene" hx-ext="sse" sse-connect="./api/subscribe">
            <main hx-trigger="load, sse:update" hx-get="./api/state" hx-swap="innerHTML"></main>
            <div>{ reconnect_reason }</div>
        </div>
    };

    Ok((*auth, TypedHeader(hx::ReplaceUrl(url)), html))
}

#[debug_handler(state=Arc<Mutex<ApiState>>)]
async fn keep_alive(mut user_session: UserSession<Authenticated>) -> Result<impl IntoResponse> {
    user_session.session.keep_alive(user_session.auth).await
}

#[debug_handler(state=Arc<Mutex<ApiState>>)]
async fn subscribe(mut user_session: UserSession<Authenticated>) -> Result<impl IntoResponse> {
    let (tx, rx) = mpsc::unbounded_channel();
    user_session
        .session
        .subscribe(user_session.auth, tx)
        .await?;
    let rx = UnboundedReceiverStream::new(rx)
        .map(|data| html! { <div>{text!("{}", data)}</div> })
        .map(|data| sse::Event::default().data(data.to_string()).event("update"))
        .map(Ok::<_, Infallible>);
    Ok(Sse::new(rx).keep_alive(KeepAlive::default()))
}

#[debug_handler(state=Arc<Mutex<ApiState>>)]
async fn state(mut user_session: UserSession<Authenticated>) -> Result<impl IntoResponse> {
    user_session.session.state(user_session.auth).await
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
async fn start(mut user_session: UserSession<HostAuthenticated>) -> Result<impl IntoResponse> {
    user_session.session.start(user_session.auth).await
}

#[debug_handler(state=Arc<Mutex<ApiState>>)]
async fn playable(
    mut user_session: UserSession<Authenticated>,
    Form(play_request): Form<Vec<(Card, Card)>>,
) -> Result<impl IntoResponse> {
    let cards = Cards::from_iter(play_request.into_iter().map(|(card, _)| card));
    let playable = user_session
        .session
        .playable(user_session.auth, cards)
        .await;

    Ok(match playable {
        Ok(play) if play.is_pass() => {
            html! { <button class="playable-ok" type="submit" hx-post="./api/play" title="pass">"pass"</button> }
        }
        Ok(_play) => {
            html! { <button class="playable-ok" type="submit" hx-post="./api/play" title="play">"play"</button> }
        }
        Err(Error::NotCurrent) => {
            html! { <button class="playable-off-turn" type="submit" hx-post="./api/play" title="play" disabled>"play"</button> }
        }
        Err(Error::PlayError(e)) => {
            html! { <button class="playable-error" type="submit" hx-post="./api/play" title={e.to_string()} disabled>"play"</button> }
        }
        Err(e) => return Err(e),
    })
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

    async fn connect_new(&mut self) -> Result<Authenticated> {
        let session_id = SessionId::random();
        let session = Session::new();
        self.sessions.insert(session_id, Arc::clone(&session));
        let auth = session.lock().await.connect(session_id).await?;

        let this = Weak::clone(&self.this);
        let session = Arc::downgrade(&session);
        task::spawn(Self::inactive_disconnect(this, session, auth));

        Ok(auth)
    }

    async fn connect_existing(&mut self, session_id: SessionId) -> Result<Authenticated> {
        let session = self.get_session(session_id).await?;
        let auth = session.lock().await.connect(session_id).await?;

        let this = Weak::clone(&self.this);
        let session = Arc::downgrade(&session);
        task::spawn(Self::inactive_disconnect(this, session, auth));

        Ok(auth)
    }

    async fn inactive_disconnect(
        this: Weak<Mutex<Self>>,
        session: Weak<Mutex<Session>>,
        auth: Authenticated,
    ) {
        let mut interval = interval(INACTIVE_BEFORE_DISCONNECT);
        interval.set_missed_tick_behavior(MissedTickBehavior::Skip);

        let session = loop {
            interval.tick().await;
            let Some(session) = session.upgrade() else {
                return;
            };
            let mut session = session.lock_owned().await;
            if session.inactive_disconnect(auth).await.is_break() {
                break session;
            }
        };

        if session.is_empty() {
            let Some(this) = this.upgrade() else { return };
            let mut this = this.lock().await;
            this.sessions.remove(&auth.session_id);
        }
    }

    async fn get_session(&self, session_id: SessionId) -> Result<Arc<Mutex<Session>>> {
        let session = self.sessions.get(&session_id).ok_or(Error::NoSession)?;
        Ok(Arc::clone(session))
    }
}

#[derive(Debug)]
struct Session {
    timer: Option<Duration>,
    deadline: Option<Instant>,
    phase: Phase,
    card_id_cypher: HashMap<Card, String>,
    game_state: GameState,
    humans: SeatMap<Option<Human>>,
    this: Weak<Mutex<Session>>,
}

impl Session {
    pub fn new() -> Arc<Mutex<Self>> {
        Arc::new_cyclic(move |this| {
            let mut card_id_cypher: Vec<_> = (0..52)
                .map(|card| format!("card-id-cypher-{}", card))
                .collect();
            card_id_cypher.shuffle(&mut thread_rng());
            let card_id_cypher = Cards::ENTIRE_DECK.into_iter().zip(card_id_cypher).collect();
            Mutex::new(Self {
                timer: None,
                deadline: None,
                phase: Phase::Lobby,
                card_id_cypher,
                game_state: GameState::new(),
                humans: SeatMap::default(),
                this: Weak::clone(this),
            })
        })
    }

    pub async fn connect(&mut self, session_id: SessionId) -> Result<Authenticated> {
        if !matches!(self.phase, Phase::Lobby) {
            return Err(Error::BadPhase);
        }

        let seat = self
            .humans
            .iter()
            .find_map(|(seat, humans)| humans.is_none().then_some(seat))
            .ok_or(Error::Full)?;

        let user_secret = UserSecret::random();
        let host = self.is_empty();
        self.humans[seat] = Some(Human {
            host,
            user_secret,
            last_active: Instant::now(),
            tx: None,
        });

        self.alert_all(Update::Connected).await;

        Ok(Authenticated {
            auth: Unauthenticated {
                user_secret,
                seat,
                session_id,
            },
        })
    }

    pub fn is_empty(&self) -> bool {
        self.humans.iter().all(|(_, human)| human.is_none())
    }

    pub async fn inactive_disconnect(&mut self, auth: Authenticated) -> ControlFlow<()> {
        let Some(human) = self.humans[auth.seat].as_mut() else {
            return ControlFlow::Break(());
        };
        let is_active =
            Instant::now().duration_since(human.last_active) < INACTIVE_BEFORE_DISCONNECT;
        if is_active {
            return ControlFlow::Continue(());
        }
        if human.host {
            let new_host = self
                .humans
                .iter_mut()
                .find_map(|(seat, human)| human.as_mut().map(|human| (seat, human)));
            if let Some((new_host_seat, new_host)) = new_host {
                new_host.host = true;
                self.alert(new_host_seat, Update::Host).await;
            }
        }

        self.alert_all(Update::Connected).await;
        if matches!(self.phase, Phase::Active) && self.game_state.current_player() == auth.seat {
            self.solicit().await;
        }
        ControlFlow::Break(())
    }

    pub async fn keep_alive(&mut self, auth: Authenticated) -> Result<()> {
        self.humans[auth.seat]
            .as_mut()
            .ok_or(Error::Absent)?
            .last_active = Instant::now();
        Ok(())
    }

    pub async fn subscribe(
        &mut self,
        auth: Authenticated,
        tx: UnboundedSender<Update>,
    ) -> Result<()> {
        self.humans[auth.seat].as_mut().ok_or(Error::Absent)?.tx = Some(tx);
        self.alert(auth.seat, Update::Deal).await;
        Ok(())
    }

    pub fn is_host(&self, seat: Seat) -> bool {
        self.humans[seat].as_ref().is_some_and(|human| human.host)
    }

    pub fn host_authenticate(&mut self, auth: Authenticated) -> Result<HostAuthenticated> {
        if !self.is_host(auth.seat) {
            return Err(Error::NotHost);
        }
        Ok(HostAuthenticated { auth })
    }

    pub fn authenticate(&mut self, auth: Unauthenticated) -> Result<Authenticated> {
        let human = self.humans[auth.seat].as_ref().ok_or(Error::Absent)?;
        if human.user_secret != auth.user_secret {
            return Err(Error::BadAuthentication);
        }
        Ok(Authenticated { auth })
    }

    pub async fn state(&mut self, auth: Authenticated) -> Result<Node> {
        Ok(html! {
            <img class="table" alt="" />
            <section class="table">
                <div class="cards">
                { self.game_state
                    .cards_on_table()
                    .map(Play::cards)
                    .unwrap_or_default()
                    .into_iter()
                    .map(|card| html! {
                        <div class="card" id={&self.card_id_cypher[&card]}>
                            <img src={format!("./cards/{card}.svg")} alt={&card}>
                        </div>
                    })
                }
                </div>
            </section>

            { Relative::ALL
                .into_iter()
                .map(|relative| {
                    let seat = auth.seat.relative(relative);
                    let is_hosting = self.is_host(seat);
                    let my_player = seat == auth.seat;

                    let host_controls = if matches!(self.phase, Phase::Lobby) && my_player && is_hosting {
                        html! {
                            <form id="timer-controls">
                                <label>
                                    "enable action timer"
                                    <input type="checkbox" name="enable-timer" hx-trigger="load, change" hx-include="#timer-controls" hx-put="./api/timer" />
                                </label>
                                <input type="range" min="1" max="120000" value="30000" name="timer-value" hx-trigger="load, change" hx-include="#timer-controls" hx-put="./api/timer" />
                            </form>
                            <button hx-post="./api/start">"start the game"</button>
                        }
                    } else {
                        html! {}
                    };

                    let cards = self.game_state.hand(seat);
                    let cards = match (self.phase, my_player) {
                        (Phase::Lobby, _) => html! {},
                        (Phase::Active | Phase::Post, true) => html! {
                            <form>
                                <div class="cards">
                                { cards.into_iter()
                                    .map(|card| {
                                        html! {
                                            <label class="card" id={&self.card_id_cypher[&card]}>
                                                <input type="checkbox" name={&card} value={&card} hx-trigger="change" hx-post="./api/playable" hx-target="next button" hx-swap="outerHTML">
                                                <img src={format!("./cards/{card}.svg")} alt={&card}>
                                            </label>
                                        }
                                    })
                                }
                                </div>
                                <button hx-trigger="load" hx-post="./api/playable" hx-swap="outerHTML"></button>
                            </form>
                        },
                        (Phase::Active | Phase::Post, false) => html! {
                            <div class="cards">
                            { cards.into_iter()
                                .map(|card| {
                                    html! {
                                        <label class="card" id={&self.card_id_cypher[&card]}>
                                            <img src="./cards/back.svg" alt="hidden card">
                                        </label>
                                    }
                                })
                            }
                            </div>
                        },
                    };

                    let load = if matches!(self.phase, Phase::Active | Phase::Post) {
                        html! { {text!("{}", self.game_state.hand(seat).len())} }
                    } else {
                        html! {}
                    };
                    let bot = if self.is_human(seat) {
                        html! { }
                    } else {
                        html! { <img src="./bot.svg" alt="bot player" /> }
                    };
                    let turn = if matches!(self.phase, Phase::Active) && self.game_state.current_player() == seat {
                        html! { <img src="./turn.svg" alt="player's turn" /> }
                    } else {
                        html!{ }
                    };
                    let pass = if matches!(self.phase, Phase::Active) && self.game_state.passed(seat) {
                        html! { <img src="./pass.svg" alt="player passed" /> }
                    } else {
                        html! { }
                    };
                    let control = if matches!(self.phase, Phase::Active) && self.game_state.has_control(seat) {
                        html! { <img src="./control.svg" alt="player has control" /> }
                    } else {
                        html! {}
                    };
                    let win = if self.game_state.winning_player() == Some(seat) {
                        html! { <img src="./win.svg" alt="player win" /> }
                    } else {
                        html! { }
                    };
                    html! {
                        <section class=format!("player {}", relative)>
                            <h2 class="name">{text!("{}", seat)}</h2>
                            <div class="load">{text!("{}", load)}</div>
                            <div class="bot">{ bot }</div>
                            <div class="timer"> </div>
                            <div class="turn">{ turn }</div>
                            <div class="pass">{ pass }</div>
                            <div class="control">{ control }</div>
                            <div class="win">{ win }</div>
                            <div class="host-controls">{ host_controls }</div>
                            <div class="cards-form">{ cards }</div>
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
        self.timer = timer;
        Ok(())
    }

    pub async fn start(&mut self, _auth: HostAuthenticated) -> Result<()> {
        if !matches!(self.phase, Phase::Lobby) {
            return Err(Error::BadPhase);
        }
        self.phase = Phase::Active;
        self.alert_all(Update::Deal).await;
        self.solicit().await;

        Ok(())
    }

    pub async fn playable(&mut self, auth: Authenticated, cards: Cards) -> Result<Play> {
        if !matches!(self.phase, Phase::Active) {
            return Err(Error::BadPhase);
        }
        let current_player = self.game_state.current_player();
        if auth.seat != current_player {
            return Err(Error::NotCurrent);
        }
        Ok(self.game_state.playable(cards)?)
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
        self.alert_all(Update::Play).await;

        let win = self.game_state.winning_player().is_some();
        if win {
            self.phase = Phase::Post;
            self.alert_all(Update::Win).await;
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

        self.alert_all(Update::Turn).await;

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

    async fn alert(&self, seat: Seat, update: Update) {
        let Some(tx) = self.humans[seat]
            .as_ref()
            .and_then(|human| human.tx.as_ref())
        else {
            return;
        };
        let _ = tx.send(update);
    }

    async fn alert_all(&mut self, update: Update) {
        for seat in Seat::ALL {
            self.alert(seat, update.clone()).await;
        }
    }

    fn is_human(&self, seat: Seat) -> bool {
        self.humans[seat].is_some()
    }
}

#[derive(Clone, Copy, Hash, Eq, PartialEq, PartialOrd, Ord, Debug, Serialize, Deserialize)]
enum Phase {
    Lobby,
    Active,
    Post,
}

#[derive(Debug)]
struct Human {
    user_secret: UserSecret,
    last_active: Instant,
    host: bool,
    tx: Option<UnboundedSender<Update>>,
}

#[derive(Clone, Serialize)]
enum Update {
    Deal,
    Turn,
    Connected,
    Host,
    Play,
    Win,
}

impl Display for Update {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Deal => write!(f, "deal"),
            Self::Turn => write!(f, "solicit"),
            Self::Connected => write!(f, "connected"),
            Self::Host => write!(f, "host"),
            Self::Play => write!(f, "play"),
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
        let auth = parts
            .extract::<Unauthenticated>()
            .await
            .map_err(|err| err.into_response())?;
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
        let auth = serde_urlencoded::to_string(self).unwrap();
        let cookie = Cookie::build("auth", auth)
            .secure(true)
            .http_only(true)
            .expires(Expiration::Session)
            .same_site(SameSite::Strict)
            .finish();
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
        let auth = serde_urlencoded::from_str(auth).map_err(|err| {
            (
                StatusCode::UNAUTHORIZED,
                format!("could not deserialize auth cookie: {}", err),
            )
                .into_response()
        })?;
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
            Self::Full => write!(f, "can only connect sessions that aren't full"),
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
