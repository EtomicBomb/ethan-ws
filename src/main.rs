#![allow(dead_code)]
#![allow(unused_imports)]

use async_trait::async_trait;
use axum::{
    debug_handler,
    extract::{FromRequestParts, Query, State},
    response::{IntoResponse, Response},
    routing::{get, post, put},
    Json, RequestPartsExt, Router, TypedHeader,
};
use axum_core::response::{IntoResponseParts, ResponseParts};
//use axum_server::tls_rustls::RustlsConfig;
use base64::{engine::general_purpose, Engine as _};
use futures::future::BoxFuture;
use headers::{authorization::Bearer, Authorization, HeaderMapExt};
use http::{header::HeaderValue, request::Parts, status::StatusCode};
use rand::{seq::SliceRandom, thread_rng, Rng};
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DurationMilliSeconds};
use std::{
    collections::HashMap, convert::Infallible, fmt, mem, net::SocketAddr, sync::Arc, sync::Weak,
    time::Duration,
};
use tokio::{
    sync::mpsc::{self, UnboundedSender},
    sync::Mutex,
    task,
    time::{sleep_until, Instant},
};
use tokio_stream::wrappers::UnboundedReceiverStream;
use tower_http::{
    services::{ServeDir, ServeFile},
    set_header::SetResponseHeaderLayer,
};
use uuid::Uuid;

mod game;
mod json_stream;
use crate::game::{choose_play, Cards, GameState, PlayError, Seat};
use crate::json_stream::JsonStream;

const MAX_ACTION_TIMER: Duration = Duration::from_secs(1000);
const BOT_ACTION_TIMER: Duration = Duration::from_secs(1);

#[tokio::main]
async fn main() {
    //    let config = RustlsConfig::from_pem_file(
    //        "secret/cert.pem",
    //        "secret/key.pem",
    //    )
    //    .await
    //    .unwrap();

    let app = app();

    let addr = SocketAddr::from(([127, 0, 0, 1], 8000));
    eprintln!("listening on {}", addr);

    //    axum_server::bind_rustls(addr, config)
    axum_server::bind(addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

fn app() -> Router {
    let api = Router::new()
        .route("/test", get(test))
        .route("/join", post(join))
        .route("/username", put(username))
        .route("/timer", put(timer))
        .route("/start", post(start))
        .route("/play", post(play))
        .route("/playable", post(playable))
        .with_state(ApiState::new());

    let serve_dir = ServeDir::new("www")
        .not_found_service(ServeFile::new("www/not_found.html"))
        .append_index_html_on_directories(true);

    Router::new()
        .nest("/api", api)
        .nest_service("/", serve_dir)
        .layer(SetResponseHeaderLayer::overriding(
            // TODO: revert debug
            http::header::CACHE_CONTROL,
            HeaderValue::from_static("no-store, must-revalidate"),
        ))
}

#[debug_handler(state=Arc<Mutex<ApiState>>)]
async fn test() -> impl IntoResponse {
    use tokio::time::sleep;
    let (tx, stream) = mpsc::unbounded_channel();
    task::spawn(async move {
        for i in 0.. {
            let Ok(_) = tx.send(i) else { break };
            sleep(Duration::from_millis(1000)).await;
        }
    });
    let stream = UnboundedReceiverStream::new(stream);
    JsonStream { stream }
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
    let stream = JsonStream { stream };

    let auth = state.lock().await.join(session_id, tx).await?;

    task::spawn(async move {
        disconnected.closed().await;
        let mut state = state.lock().await;
        state.disconnect(auth).await;
    });

    Ok((auth, stream))
}

#[derive(Deserialize)]
struct UsernameRequest {
    username: String,
}

#[debug_handler(state=Arc<Mutex<ApiState>>)]
async fn username(
    State(state): State<Arc<Mutex<ApiState>>>,
    auth: Auth,
    Json(UsernameRequest { username }): Json<UsernameRequest>,
) -> Result<impl IntoResponse> {
    let phase = state.lock().await.check_auth(auth).await?;
    let mut phase = phase.lock().await;
    phase.username(auth.seat, username).await
}

#[serde_as]
#[derive(Deserialize)]
struct ActionTimerRequest {
    #[serde_as(as = "Option<DurationMilliSeconds<u64>>")]
    millis: Option<Duration>,
}

#[debug_handler(state=Arc<Mutex<ApiState>>)]
async fn timer(
    State(state): State<Arc<Mutex<ApiState>>>,
    auth: Auth,
    Json(ActionTimerRequest { millis }): Json<ActionTimerRequest>,
) -> Result<impl IntoResponse> {
    let phase = state.lock().await.check_auth(auth).await?;
    phase.lock().await.timer(auth.seat, millis).await?;
    Ok(())
}

#[debug_handler(state=Arc<Mutex<ApiState>>)]
async fn start(State(state): State<Arc<Mutex<ApiState>>>, auth: Auth) -> Result<impl IntoResponse> {
    let phase = state.lock().await.check_auth(auth).await?;
    let mut phase = phase.lock().await;
    phase.start(auth.seat).await
}

#[derive(Deserialize)]
struct PlayRequest {
    cards: Cards,
}

#[debug_handler(state=Arc<Mutex<ApiState>>)]
async fn playable(
    State(state): State<Arc<Mutex<ApiState>>>,
    auth: Auth,
    Json(PlayRequest { cards }): Json<PlayRequest>,
) -> Result<impl IntoResponse> {
    let phase = state.lock().await.check_auth(auth).await?;
    let phase = phase.lock().await;
    let phase = phase.playable(auth.seat, cards);
    phase.await
}

#[debug_handler(state=Arc<Mutex<ApiState>>)]
async fn play(
    State(state): State<Arc<Mutex<ApiState>>>,
    auth: Auth,
    Json(PlayRequest { cards }): Json<PlayRequest>,
) -> Result<impl IntoResponse> {
    let phase = state.lock().await.check_auth(auth).await?;
    let mut phase = phase.lock().await;
    phase.human_play(auth.seat, cards).await
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

    async fn join(
        &mut self,
        session_id: Option<SessionId>,
        tx: UnboundedSender<Respond>,
    ) -> Result<Auth> {
        let phase = match session_id {
            Some(session_id) => Arc::clone(self.phases.get(&session_id).ok_or(Error::NoSession)?),
            None => {
                let session_id = SessionId::random();
                let phase: Arc<Mutex<dyn Phase>> = Lobby::new(session_id, Weak::clone(&self.this));
                self.phases.insert(session_id, Arc::clone(&phase));
                phase
            }
        };
        let auth = phase.lock().await.join(tx).await?;
        Ok(auth)
    }

    async fn check_auth(&self, auth: Auth) -> Result<Arc<Mutex<dyn Phase>>> {
        let phase = self.phases.get(&auth.session_id).ok_or(Error::NoSession)?;
        let check_auth = phase.lock().await;
        let check_auth = check_auth.check_auth(auth);
        check_auth.await?;
        let phase = Arc::clone(phase);
        Ok(phase)
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
        self.phases.insert(session_id, Arc::clone(&new_phase));
    }
}

#[async_trait]
trait Phase: Send {
    async fn check_auth(&self, auth: Auth) -> Result<()>;
    async fn join(&mut self, tx: UnboundedSender<Respond>) -> Result<Auth>;
    async fn timer(&mut self, seat: Seat, timer: Option<Duration>) -> Result<()>;
    async fn username(&mut self, seat: Seat, username: String) -> Result<()>;
    async fn start(&mut self, seat: Seat) -> Result<()>;
    async fn playable(&self, seat: Seat, cards: Cards) -> Result<()>;
    async fn human_play(&mut self, seat: Seat, cards: Cards) -> Result<()>;
    async fn disconnect(&mut self, auth: Auth) -> Result<bool>;
}

struct Lobby {
    timer: Option<Duration>,
    usernames: HashMap<Seat, String>,
    common: Common,
}

impl Lobby {
    fn new(session_id: SessionId, api_state: Weak<Mutex<ApiState>>) -> Arc<Mutex<Self>> {
        Arc::new_cyclic(|_this| {
            let timer = None;
            let common = Common::new(session_id, api_state);
            let usernames = HashMap::default();
            Mutex::new(Self {
                timer,
                usernames,
                common,
            })
        })
    }

    fn is_host(&self, seat: Seat) -> bool {
        self.common.host() == Some(seat)
    }
}

#[async_trait]
impl Phase for Lobby {
    async fn check_auth(&self, auth: Auth) -> Result<()> {
        self.common.check_auth(auth)
    }

    async fn join(&mut self, tx: UnboundedSender<Respond>) -> Result<Auth> {
        let auth = self.common.new_user(tx)?;

        self.common.respond(auth.seat, Respond::Welcome {}).await;
        if self.is_host(auth.seat) {
            self.common.respond(auth.seat, Respond::Host {}).await;
        }
        for other in Seat::ALL {
            if other != auth.seat {
                self.common
                    .respond(other, Respond::Connected { seat: auth.seat })
                    .await;
            }
            if self.common.is_human(other) && other != auth.seat {
                self.common
                    .respond(auth.seat, Respond::Connected { seat: other })
                    .await;
            }
            if let Some(username) = self.usernames.get(&other) {
                self.common
                    .respond(
                        auth.seat,
                        Respond::Username {
                            seat: other,
                            username: username.clone(),
                        },
                    )
                    .await;
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

    async fn username(&mut self, seat: Seat, username: String) -> Result<()> {
        self.usernames.insert(seat, username.clone());
        for other in Seat::ALL {
            if seat == other {
                continue;
            }
            let username = username.clone();
            self.common
                .respond(other, Respond::Username { seat, username })
                .await;
        }
        Ok(())
    }

    async fn start(&mut self, seat: Seat) -> Result<()> {
        if !self.is_host(seat) {
            return Err(Error::NotHost);
        }
        let timer = self.timer;
        let api_state = self.common.api_state();
        let session_id = self.common.session_id();
        let common = mem::take(&mut self.common);
        let active = Active::new(timer, common).await;
        api_state.lock().await.transition(session_id, active);
        Ok(())
    }

    async fn playable(&self, _seat: Seat, _cards: Cards) -> Result<()> {
        Err(Error::BadPhase)
    }

    async fn human_play(&mut self, _seat: Seat, _cards: Cards) -> Result<()> {
        Err(Error::BadPhase)
    }

    async fn disconnect(&mut self, auth: Auth) -> Result<bool> {
        let (now_empty, host_disconnect) = self.common.disconnect(auth)?;

        for other in Seat::ALL {
            self.common
                .respond(other, Respond::Disconnected { seat: auth.seat })
                .await;
        }

        if host_disconnect {
            if let Some(new_host) = self.common.host() {
                self.common.respond(new_host, Respond::Host {}).await;
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
            self.common.respond(other, Respond::Deal { cards }).await;
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
            self.common
                .respond(
                    other,
                    Respond::Play {
                        seat,
                        load,
                        pass,
                        cards,
                    },
                )
                .await;
        }

        if win {
            self.win().await;
        } else {
            let _ = self.solicit().await;
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
            self.common
                .respond(
                    other,
                    Respond::Turn {
                        seat: current_player,
                        control,
                        millis: timer,
                    },
                )
                .await;
        }

        let this = Weak::clone(&self.this);
        task::spawn(async move {
            let _ = Self::force_play(this).await;
        });
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

    async fn win(&mut self) {
        let api_state = self.common.api_state();
        let session_id = self.common.session_id();
        let common = mem::take(&mut self.common);
        let phase = Win::new(common);
        api_state.lock().await.transition(session_id, phase);
    }
}

#[async_trait]
impl Phase for Active {
    async fn check_auth(&self, auth: Auth) -> Result<()> {
        self.common.check_auth(auth)
    }

    async fn join(&mut self, _tx: UnboundedSender<Respond>) -> Result<Auth> {
        Err(Error::BadPhase)
    }

    async fn timer(&mut self, _seat: Seat, _timer: Option<Duration>) -> Result<()> {
        Err(Error::BadPhase)
    }

    async fn username(&mut self, _seat: Seat, _username: String) -> Result<()> {
        Err(Error::BadPhase)
    }

    async fn start(&mut self, _seat: Seat) -> Result<()> {
        Err(Error::BadPhase)
    }

    async fn playable(&self, seat: Seat, cards: Cards) -> Result<()> {
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
            self.common
                .respond(other, Respond::Disconnected { seat: auth.seat })
                .await;
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
    fn new(common: Common) -> Arc<Mutex<Self>> {
        Arc::new_cyclic(|_weak| Mutex::new(Self { common }))
    }
}

#[async_trait]
impl Phase for Win {
    async fn check_auth(&self, auth: Auth) -> Result<()> {
        self.common.check_auth(auth)
    }

    async fn join(&mut self, _tx: UnboundedSender<Respond>) -> Result<Auth> {
        Err(Error::BadPhase)
    }

    async fn timer(&mut self, _seat: Seat, _timer: Option<Duration>) -> Result<()> {
        Err(Error::BadPhase)
    }

    async fn username(&mut self, _seat: Seat, _username: String) -> Result<()> {
        Err(Error::BadPhase)
    }

    async fn start(&mut self, _seat: Seat) -> Result<()> {
        Err(Error::BadPhase)
    }

    async fn playable(&self, _seat: Seat, _cards: Cards) -> Result<()> {
        Err(Error::BadPhase)
    }

    async fn human_play(&mut self, _seat: Seat, _cards: Cards) -> Result<()> {
        Err(Error::BadPhase)
    }

    async fn disconnect(&mut self, auth: Auth) -> Result<bool> {
        let (now_empty, _host_disconnect) = self.common.disconnect(auth)?;
        for other in Seat::ALL {
            self.common
                .respond(other, Respond::Disconnected { seat: auth.seat })
                .await;
        }
        Ok(now_empty)
    }
}

struct UserInfo {
    user_secret: UserSecret,
    tx: UnboundedSender<Respond>,
}

#[derive(Default)]
struct Common {
    session_id: SessionId,
    user_info: HashMap<Seat, UserInfo>,
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

    fn api_state(&self) -> Arc<Mutex<ApiState>> {
        self.api_state
            .upgrade()
            .expect("api_state should persist the entire lifetime of the program")
    }

    fn session_id(&self) -> SessionId {
        self.session_id
    }

    fn host(&self) -> Option<Seat> {
        self.host
    }

    fn is_human(&self, seat: Seat) -> bool {
        self.user_info.contains_key(&seat)
    }

    fn new_user(&mut self, tx: UnboundedSender<Respond>) -> Result<Auth> {
        let seat = self
            .seats
            .iter()
            .rev()
            .cloned()
            .find(|&seat| !self.is_human(seat))
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

    fn check_auth(&self, auth: Auth) -> Result<()> {
        if self.user_info[&auth.seat].user_secret != auth.user_secret {
            return Err(Error::BadAuthentication);
        }
        Ok(())
    }

    fn disconnect(&mut self, auth: Auth) -> Result<(bool, bool)> {
        self.user_info.remove(&auth.seat).ok_or(Error::Absent)?;
        let host_disconnect = self.host == Some(auth.seat);
        if host_disconnect {
            self.host = self
                .seats
                .iter()
                .rev()
                .cloned()
                .find(|&new_host| self.is_human(new_host));
        }
        let now_empty = self.user_info.is_empty();
        Ok((now_empty, host_disconnect))
    }

    async fn respond(&mut self, seat: Seat, response: Respond) {
        let Some(UserInfo { tx, .. }) = self.user_info.get(&seat) else { return };
        let _ = tx.send(response);
    }
}

#[serde_as]
#[derive(Serialize)]
#[serde(tag = "event", content = "data")]
#[serde(rename_all = "camelCase")]
enum Respond {
    Welcome {},
    Host {},
    Connected {
        seat: Seat,
    },
    Username {
        seat: Seat,
        username: String,
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
    Disconnected {
        seat: Seat,
    },
}

#[derive(Copy, Clone, Serialize, Deserialize)]
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

#[async_trait]
impl<S> FromRequestParts<S> for Auth
where
    S: Send + Sync,
{
    type Rejection = Response;
    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let TypedHeader(Authorization(token)) = parts
            .extract::<TypedHeader<Authorization<Bearer>>>()
            .await
            .map_err(|_err| (Error::BadAuthentication.into_response()))?;
        let token = token.token();
        let token = general_purpose::STANDARD
            .decode(token)
            .map_err(|_| Error::BadAuthentication.into_response())?;
        let auth =
            serde_json::from_slice(&token).map_err(|_| Error::BadAuthentication.into_response())?;
        Ok(auth)
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
            Error::BadAuthentication => StatusCode::FORBIDDEN,
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
