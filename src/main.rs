#![allow(dead_code)]
#![allow(unused_imports)]

mod game;
mod json_seq;

use {
    crate::{
        game::{choose_play, Cards, GameState, PlayError, Seat},
        json_seq::JsonSeq,
    },
    async_trait::async_trait,
    axum::{
        debug_handler,
        extract::{FromRequestParts, Query, State},
        response::{IntoResponse, Response},
        routing::{get, post, put},
        Json, RequestPartsExt, Router, TypedHeader,
    },
    axum_core::response::{IntoResponseParts, ResponseParts},
    //	axum_server::tls_rustls::RustlsConfig,
    base64::{engine::general_purpose, Engine as _},
    futures::future::BoxFuture,
    headers::{authorization::Bearer, Authorization, HeaderMapExt},
    http::{header::HeaderValue, request::Parts, status::StatusCode},
    rand::{seq::SliceRandom, thread_rng, Rng},
    serde::{Deserialize, Serialize},
    serde_with::{serde_as, DurationMilliSeconds, DisplayFromStr},
    std::{
        collections::HashMap,
        convert::Infallible,
        fmt,
        future::Future,
        mem,
        net::{Ipv4Addr, SocketAddr},
        sync::Arc,
        sync::Weak,
        time::Duration,
    },
    tokio::{
        sync::mpsc::{self, UnboundedSender},
        sync::{Mutex, OwnedMutexGuard},
        task,
        time::{interval, sleep_until, Instant},
    },
    tokio_stream::wrappers::UnboundedReceiverStream,
    tower_http::{
        services::{ServeDir, ServeFile},
        set_header::SetResponseHeaderLayer,
    },
    uuid::Uuid,
    html_node::{html, text},
};


const MAX_ACTION_TIMER: Duration = Duration::from_secs(1000);
const BOT_ACTION_TIMER: Duration = Duration::from_secs(1);

#[tokio::main]
async fn main() {
    //    let tls_config = RustlsConfig::from_pem_file("secret/cert.pem", "secret/key.pem")
    //        .await
    //        .unwrap();

    let serve_api = Router::new()
        .route("/join", post(join))
        .route("/timer", put(timer))
        .route("/start", post(start))
        .route("/play", post(play))
        .route("/playable", post(playable))
        .with_state(ApiState::new())
        .route("/test", get(test));

    let serve_static = ServeDir::new("www")
        .not_found_service(ServeFile::new("www/not_found.html"))
        .append_index_html_on_directories(true);

    let serve = Router::new()
        .nest("/api", serve_api)
        .nest_service("/", serve_static)
        // TODO: deploy
        // TODO: mark Authorization header as sensitive
        .layer(SetResponseHeaderLayer::overriding(
            http::header::CACHE_CONTROL,
            HeaderValue::from_static("no-store, must-revalidate"),
        ));

    let addr = SocketAddr::from((Ipv4Addr::UNSPECIFIED, 8000));
    eprintln!("listening on {}", addr);

    //    axum_server::bind_rustls(addr, tls_config)
    axum_server::bind(addr)
        .serve(serve.into_make_service())
        .await
        .unwrap();
}

#[debug_handler]
async fn test() -> impl IntoResponse {
    let (tx, stream) = mpsc::unbounded_channel();
    task::spawn(async move {
        let mut interval = interval(Duration::from_millis(1000));
        for i in 0.. {
            interval.tick().await;
            let Ok(_) = tx.send(i) else { break };
        }
    });
    let stream = UnboundedReceiverStream::new(stream);
    JsonSeq { stream }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct JoinQuery {
    session_id: Option<SessionId>,
}

#[debug_handler(state=Arc<Mutex<ApiState>>)]
async fn join(
    State(state): State<Arc<Mutex<ApiState>>>,
    Query(JoinQuery { session_id }): Query<JoinQuery>,
) -> Result<impl IntoResponse> {
    let (tx, stream) = mpsc::unbounded_channel();
    let disconnected = tx.clone();
    let stream = UnboundedReceiverStream::new(stream);
    let stream = JsonSeq { stream };

    let auth = state.lock().await.join(session_id, tx).await?;

    task::spawn(async move {
        disconnected.closed().await;
        let mut state = state.lock().await;
        state.disconnect(auth).await;
    });

    Ok((auth, stream))
}

#[serde_as]
#[derive(Deserialize)]
struct ActionTimerRequest {
    #[serde_as(as = "Option<DurationMilliSeconds<u64>>")]
    millis: Option<Duration>,
}

#[debug_handler(state=Arc<Mutex<ApiState>>)]
async fn timer(
    mut session: UserSession,
    Json(ActionTimerRequest { millis }): Json<ActionTimerRequest>,
) -> Result<impl IntoResponse> {
    session.phase.timer(session.auth.seat, millis).await?;
    Ok(())
}

#[debug_handler(state=Arc<Mutex<ApiState>>)]
async fn start(mut session: UserSession) -> Result<impl IntoResponse> {
    session.phase.start(session.auth.seat).await
}

#[derive(Deserialize)]
struct PlayRequest {
    cards: Cards,
}

#[debug_handler(state=Arc<Mutex<ApiState>>)]
async fn playable(
    mut session: UserSession,
    Json(PlayRequest { cards }): Json<PlayRequest>,
) -> Result<impl IntoResponse> {
    session.phase.playable(session.auth.seat, cards).await
}

#[debug_handler(state=Arc<Mutex<ApiState>>)]
async fn play(
    mut session: UserSession,
    Json(PlayRequest { cards }): Json<PlayRequest>,
) -> Result<impl IntoResponse> {
    session.phase.human_play(session.auth.seat, cards).await
}

struct JoinResponse {
    auth: Auth,
    error: Option<Error>,
}

#[derive(Default)]
struct ApiState {
    phases: HashMap<SessionId, Arc<Mutex<dyn Phase>>>,
    this: Weak<Mutex<ApiState>>,
}

impl ApiState {
    fn new() -> Arc<Mutex<Self>> {
        Arc::new_cyclic(|this| {
            Mutex::new(Self {
                phases: HashMap::default(),
                this: Weak::clone(this),
            })
        })
    }


//    async fn join(
//        &mut self,
//        session_id: Option<SessionId>,
//        tx: UnboundedSender<Message>,
//    ) -> Auth {
//        let retry = match self.try_join(session_id, tx.clone()).await {
//            Ok(auth) => return auth,
//            Err(couldnt_join) => couldnt_join,
//        };
//
//        let phase = self.empty_lobby().await;
//        let auth = phase.lock().await.join(tx, Some(retry)).await
//            .expect("should always be able to join an empty lobby");
//        auth
//    }

    async fn join(
        &mut self,
        session_id: Option<SessionId>,
        tx: UnboundedSender<Message>,
    ) -> Result<Auth> {
        let phase = match session_id {
            Some(session_id) => self.get_phase(session_id).await?,
            None => self.empty_lobby().await,
        };
        let auth = phase.lock().await.join(tx, None).await?;
        Ok(auth)
    }
    
    async fn get_phase(&self, session_id: SessionId) -> Result<Arc<Mutex<dyn Phase>>> {
        let phase = self.phases.get(&session_id).ok_or(Error::NoSession)?;
        Ok(Arc::clone(phase))
    }

    async fn empty_lobby(&mut self) -> Arc<Mutex<dyn Phase>> {
        let session_id = SessionId::random();
        let phase = Lobby::new(session_id, Weak::clone(&self.this));
        self.transition(session_id, Arc::clone(&phase) as _);
        phase
    }
    
    async fn disconnect(&mut self, auth: Auth) {
        // there should be only one thingn in these maps
        let Some(phase) = self.phases.get(&auth.session_id) else { return };
        let phase = Arc::clone(phase);
        let mut phase = phase.lock().await;
        let Ok(now_empty) = phase.disconnect(auth).await else { return };
        if now_empty {
            self.phases.remove(&auth.session_id);
        }
    }

    fn transition(&mut self, session_id: SessionId, new_phase: Arc<Mutex<dyn Phase>>) {
        self.phases.insert(session_id, new_phase);
    }
}

#[async_trait]
trait Phase: Send + Sync {
    async fn check_auth(&mut self, auth: Auth) -> Result<()>;

    async fn join(&mut self, _tx: UnboundedSender<Message>, _retry: Option<Error>) -> Result<Auth> {
        Err(Error::BadPhase)
    }

    async fn timer(&mut self, _seat: Seat, _timer: Option<Duration>) -> Result<()> {
        Err(Error::BadPhase)
    }

    async fn start(&mut self, _seat: Seat) -> Result<()> {
        Err(Error::BadPhase)
    }

    async fn playable(&mut self, _seat: Seat, _cards: Cards) -> Result<()> {
        Err(Error::BadPhase)
    }

    async fn human_play(&mut self, _seat: Seat, _cards: Cards) -> Result<()> {
        Err(Error::BadPhase)
    }

    async fn disconnect(&mut self, _auth: Auth) -> Result<bool>;
}

struct Lobby {
    timer: Option<Duration>,
    common: Common,
}

impl Lobby {
    fn new(session_id: SessionId, api_state: Weak<Mutex<ApiState>>) -> Arc<Mutex<Self>> {
        Arc::new_cyclic(|_this| {
            let timer = None;
            let common = Common::new(session_id, api_state);
            Mutex::new(Self { timer, common })
        })
    }

    fn is_host(&self, seat: Seat) -> bool {
        self.common.host() == Some(seat)
    }
}

#[async_trait]
impl Phase for Lobby {
    async fn check_auth(&mut self, auth: Auth) -> Result<()> {
        self.common.check_auth(auth)
    }

    async fn join(&mut self, tx: UnboundedSender<Message>, retry: Option<Error>) -> Result<Auth> {
        let auth = self.common.new_user(tx)?;

        self.common.message(auth.seat, Message::Welcome {}).await;
        if let Some(error) = retry {
            self.common.message(auth.seat, Message::Retry { error }).await;
        }
        if self.is_host(auth.seat) {
            self.common.message(auth.seat, Message::Host {}).await;
        }
        for other in Seat::ALL {
            if other != auth.seat {
                let message = Message::Connected { seat: auth.seat };
                self.common.message(other, message).await;
            }
            if self.common.is_human(other) && other != auth.seat {
                let message = Message::Connected { seat: other };
                self.common.message(auth.seat, message).await;
            }
        }

        Ok(auth)
    }

    async fn timer(&mut self, seat: Seat, timer: Option<Duration>) -> Result<()> {
        if !self.is_host(seat) {
            return Err(Error::NotHost);
        }
        let too_long = timer.is_some_and(|timer| timer > MAX_ACTION_TIMER);
        self.timer = if too_long { None } else { timer };
        Ok(())
    }

    async fn start(&mut self, seat: Seat) -> Result<()> {
        if !self.is_host(seat) {
            return Err(Error::NotHost);
        }
        let timer = self.timer;
        self.common
            .transition(|common| async move { Active::new(timer, common).await as _ })
            .await;
        Ok(())
    }

    async fn disconnect(&mut self, auth: Auth) -> Result<bool> {
        let (now_empty, host_disconnect) = self.common.disconnect(auth)?;

        for other in Seat::ALL {
            let message = Message::Disconnected { seat: auth.seat };
            self.common.message(other, message).await;
        }

        if host_disconnect {
            if let Some(new_host) = self.common.host() {
                self.common.message(new_host, Message::Host {}).await;
            }
        }

        Ok(now_empty)
    }
}

struct Active {
    timer: Option<Duration>,
    game_state: GameState,
    deadline: Option<Instant>,
    common: Common,
    this: Weak<Mutex<Active>>,
}

impl Active {
    async fn new(timer: Option<Duration>, common: Common) -> Arc<Mutex<Self>> {
        let this = Arc::new_cyclic(|this| {
            let this = Weak::clone(this);
            let game_state = GameState::default();
            let deadline = None;
            Mutex::new(Self {
                timer,
                game_state,
                deadline,
                common,
                this,
            })
        });
        this.lock().await.deal().await;
        this
    }

    async fn deal(&mut self) {
        let hands = Seat::ALL.map(|other| (other, self.game_state.hand(other)));
        for (other, cards) in hands {
            self.common.message(other, Message::Deal { cards }).await;
        }
        self.solicit().await;
    }

    async fn auto_play(&mut self) {
        let current_player = self.game_state.current_player();
        let cards = choose_play(&self.game_state).cards;
        self.play(current_player, cards)
            .await
            .expect("our bots should always choose valid plays");
    }

    async fn play(&mut self, seat: Seat, cards: Cards) -> Result<()> {
        let play = self.game_state.play(cards)?;
        let pass = play.is_pass();
        let load = self.game_state.hand(seat).len();
        let win = load == 0;
        for other in Seat::ALL {
            let message = Message::Play {
                seat,
                load,
                pass,
                cards,
            };
            self.common.message(other, message).await;
        }

        if win {
            self.win(seat).await;
        } else {
            self.solicit().await;
        }

        Ok(())
    }

    async fn solicit(&mut self) {
        let current_player = self.game_state.current_player();
        let timer = if self.common.is_human(current_player) {
            self.timer
        } else {
            Some(BOT_ACTION_TIMER)
        };

        self.deadline = timer.and_then(|timer| Instant::now().checked_add(timer));

        let control = self.game_state.has_control();
        for other in Seat::ALL {
            let message = Message::Turn {
                seat: current_player,
                control,
                millis: timer,
            };
            self.common.message(other, message).await;
        }

        let this = Weak::clone(&self.this);
        task::spawn(Self::force_play(this));
    }

    // BoxFuture to break recursion
    fn force_play(this: Weak<Mutex<Self>>) -> BoxFuture<'static, ()> {
        Box::pin(async move {
            let Some(deadline) = this.upgrade() else { return };
            let Some(deadline) = deadline.lock().await.deadline else { return };
            sleep_until(deadline).await;

            let Some(this) = this.upgrade() else { return };
            let mut this = this.lock().await;
            let Some(deadline) = this.deadline else { return };
            if Instant::now() < deadline {
                return;
            }
            // TODO: can choose a worse bot here if we're forcing a human player
            this.auto_play().await;
        })
    }

    async fn win(&mut self, winner: Seat) {
        self.common
            .transition(|common| async move { Win::new(common, winner).await as _ })
            .await;
    }
}

#[async_trait]
impl Phase for Active {
    async fn check_auth(&mut self, auth: Auth) -> Result<()> {
        self.common.check_auth(auth)
    }

    async fn playable(&mut self, seat: Seat, cards: Cards) -> Result<()> {
        let current_player = self.game_state.current_player();
        if seat != current_player {
            return Err(Error::NotCurrent);
        }
        self.game_state.playable(cards)?;
        Ok(())
    }

    async fn human_play(&mut self, seat: Seat, cards: Cards) -> Result<()> {
        let current_player = self.game_state.current_player();
        if seat != current_player {
            return Err(Error::NotCurrent);
        }
        self.play(seat, cards).await?;
        Ok(())
    }

    async fn disconnect(&mut self, auth: Auth) -> Result<bool> {
        let (now_empty, _host_disconnect) = self.common.disconnect(auth)?;
        for other in Seat::ALL {
            let message = Message::Disconnected { seat: auth.seat };
            self.common.message(other, message).await;
        }
        if self.game_state.current_player() == auth.seat {
            self.solicit().await;
        }
        Ok(now_empty)
    }
}

struct Win {
    common: Common,
}

impl Win {
    async fn new(common: Common, winner: Seat) -> Arc<Mutex<Self>> {
        let ret = Arc::new_cyclic(|_weak| Mutex::new(Self { common }));
        ret.lock().await.announce(winner).await;
        ret
    }

    async fn announce(&mut self, winner: Seat) {
        for other in Seat::ALL {
            let message = Message::Win { seat: winner };
            self.common.message(other, message).await;
        }
    }
}

#[async_trait]
impl Phase for Win {
    async fn check_auth(&mut self, auth: Auth) -> Result<()> {
        self.common.check_auth(auth)
    }

    async fn disconnect(&mut self, auth: Auth) -> Result<bool> {
        let (now_empty, _host_disconnect) = self.common.disconnect(auth)?;
        for other in Seat::ALL {
            let message = Message::Disconnected { seat: auth.seat };
            self.common.message(other, message).await;
        }
        Ok(now_empty)
    }
}

#[derive(Debug)]
struct UserInfo {
    user_secret: UserSecret,
    tx: UnboundedSender<Message>,
}

#[derive(Default)]
struct Common {
    user_info: HashMap<Seat, UserInfo>,
    session_id: SessionId,
    api_state: Weak<Mutex<ApiState>>,
    seats: Vec<Seat>,
    host: Option<Seat>,
}

impl Common {
    fn new(session_id: SessionId, api_state: Weak<Mutex<ApiState>>) -> Self {
        let mut seats = Vec::from(Seat::ALL);
        seats.shuffle(&mut thread_rng());
        Self {
            session_id,
            seats,
            user_info: HashMap::default(),
            host: None,
            api_state,
        }
    }

    fn check_auth(&mut self, auth: Auth) -> Result<()> {
        let user_info = self.user_info.get(&auth.seat).ok_or(Error::Absent)?;
        if user_info.user_secret != auth.user_secret {
            return Err(Error::BadAuthentication);
        }
        Ok(())
    }

    async fn transition<F, T>(&mut self, new_phase: F)
    where
        F: FnOnce(Common) -> T,
        T: Future<Output = Arc<Mutex<dyn Phase>>>,
    {
        dbg!(&self.user_info, &self.session_id, &self.api_state, &self.seats, &self.host);
        let api_state = self
            .api_state
            .upgrade()
            .expect("api_state should persist the entire lifetime of the program");
        let session_id = self.session_id;
        let common = mem::take(self);
        let phase = new_phase(common).await;
        // What if someone's waiting on a lock for self, and acquires it
        // after this function returns and the transition is complete?
        // Since we took self, their call to check_auth will fail.
        api_state.lock().await.transition(session_id, phase);
    }

    fn host(&self) -> Option<Seat> {
        self.host
    }

    fn is_human(&self, seat: Seat) -> bool {
        self.user_info.contains_key(&seat)
    }

    fn new_user(&mut self, tx: UnboundedSender<Message>) -> Result<Auth> {
        let seat = *self
            .seats
            .iter()
            .rev()
            .find(|&&seat| !self.is_human(seat))
            .ok_or(Error::Full)?;
        let user_secret = UserSecret::random();
        let session_id = self.session_id;
        self.user_info.insert(seat, UserInfo { user_secret, tx });
        self.host.get_or_insert(seat);
        Ok(Auth {
            user_secret,
            seat,
            session_id,
        })
    }

    fn disconnect(&mut self, auth: Auth) -> Result<(bool, bool)> {
        self.user_info.remove(&auth.seat).ok_or(Error::Absent)?;
        let host_disconnect = self.host == Some(auth.seat);
        if host_disconnect {
            self.host = self
                .seats
                .iter()
                .rev()
                .find(|&&new_host| self.is_human(new_host))
                .cloned();
        }
        let now_empty = self.user_info.is_empty();
        Ok((now_empty, host_disconnect))
    }

    async fn message(&mut self, seat: Seat, message: Message) {
        let Some(UserInfo { tx, .. }) = self.user_info.get(&seat) else { return };
        let _ = tx.send(message);
    }
}

#[serde_as]
#[derive(Serialize)]
#[serde(tag = "event", content = "data")]
#[serde(rename_all = "camelCase")]
enum Message {
    Welcome {},
    Retry {
        #[serde_as(as = "DisplayFromStr")]
        error: Error,
    },
    Host {},
    Connected {
        seat: Seat,
    },
    Deal {
        cards: Cards,
    },
    Play {
        seat: Seat,
        load: usize,
        pass: bool,
        cards: Cards,
    },
    Turn {
        seat: Seat,
        control: bool,
        #[serde_as(as = "Option<DurationMilliSeconds<u64>>")]
        millis: Option<Duration>,
    },
    Win {
        seat: Seat,
    },
    Disconnected {
        seat: Seat,
    },
}

struct UserSession {
    phase: OwnedMutexGuard<dyn Phase>,
    auth: Auth,
}

#[async_trait]
impl FromRequestParts<Arc<Mutex<ApiState>>> for UserSession {
    type Rejection = Response;
    async fn from_request_parts(parts: &mut Parts, state: &Arc<Mutex<ApiState>>) -> Result<Self, Self::Rejection> {
        let TypedHeader(Authorization(token)) = parts
            .extract::<TypedHeader<Authorization<Bearer>>>()
            .await
            .map_err(|_err| (Error::BadAuthentication.into_response()))?;
        let token = token.token();
        let token = general_purpose::STANDARD
            .decode(token)
            .map_err(|_| Error::BadAuthentication.into_response())?;
        let auth: Auth =
            serde_json::from_slice(&token).map_err(|_| Error::BadAuthentication.into_response())?;
        let phase = state.lock().await.get_phase(auth.session_id).await
            .map_err(|err| err.into_response())?;
        let mut phase = phase.lock_owned().await;
        phase.check_auth(auth).await
            .map_err(|err| err.into_response())?;
        Ok(UserSession { phase, auth })
    }
}

#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Auth {
    seat: Seat,
    session_id: SessionId,
    user_secret: UserSecret,
}

impl IntoResponseParts for Auth {
    type Error = Infallible;
    fn into_response_parts(self, mut res: ResponseParts) -> Result<ResponseParts, Self::Error> {
        let auth = serde_json::to_string(&self).unwrap();
        let auth = general_purpose::STANDARD.encode(auth);
        res.headers_mut()
            .typed_insert(Authorization::bearer(&auth).unwrap());
        Ok(res)
    }
}

// TODO: for some reason it is considered harmful to use Uuid's (even crypto random 122 bits) as secrets
#[derive(Default, Copy, Clone, Serialize, Deserialize, PartialEq, Eq, Debug, Hash)]
struct SessionId(Uuid);

impl SessionId {
    fn random() -> Self {
        let ret = thread_rng().gen();
        let ret = uuid::Builder::from_random_bytes(ret).into_uuid();
        Self(ret)
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
        Error::PlayError(error)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::BadAuthentication => write!(f, "authentication not valid for the player"),
            Error::NoSession => write!(f, "request session not found"),
            Error::Absent => write!(f, "player must be present in the game"),
            Error::BadPhase => write!(f, "request must be applicable to current game phase"),
            Error::NotHost => write!(f, "requests must have from host"),
            Error::Full => write!(f, "can only join sessions that aren't full"),
            Error::NotCurrent => write!(f, "this request should be made by the current player"),
            Error::PlayError(error) => write!(f, "{}", error),
        }
    }
}

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        let status = match self {
            Error::BadAuthentication => StatusCode::UNAUTHORIZED,
            Error::NoSession => StatusCode::BAD_REQUEST,
            Error::Absent => StatusCode::BAD_REQUEST,
            Error::BadPhase => StatusCode::BAD_REQUEST,
            Error::NotHost => StatusCode::FORBIDDEN,
            Error::Full => StatusCode::BAD_REQUEST,
            Error::NotCurrent => StatusCode::BAD_REQUEST,
            Error::PlayError(..) => StatusCode::BAD_REQUEST,
        };
        let body = self.to_string();
        (status, body).into_response()
    }
}
