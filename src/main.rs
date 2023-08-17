#![allow(dead_code)]
#![allow(unused_imports)]

use async_trait::async_trait;
use axum_core::response::{IntoResponseParts, ResponseParts};
use axum::{
    TypedHeader,
    debug_handler,
    extract::{FromRequestParts, Query, State},
    response::{
        sse::{self, KeepAlive, Sse},
        Response,
        IntoResponse,
    },
    routing::{get, post, put},
    Json, RequestPartsExt, Router,
};
//use axum_server::tls_rustls::RustlsConfig;
use futures::future::{BoxFuture, LocalBoxFuture};
use http::{
    header::{HeaderValue},
    request::Parts,
    status::StatusCode,
    HeaderMap,
};
use rand::{seq::SliceRandom, thread_rng, Rng};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap, convert::Infallible, net::SocketAddr, sync::Arc, sync::Weak,
    time::Duration,
    mem,
    fmt,
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
use serde_with::serde_as;
use base64::{Engine as _, engine::{general_purpose}};
use headers::{HeaderMapExt, Authorization, authorization::Bearer};


mod game;
mod json_stream;
use crate::game::{choose_play, Cards, GameState, Seat, PlayError};

use crate::json_stream::JsonStream;

mod text_stream;
use crate::text_stream::TextStream;
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
    host_id: Option<UserId>,
}

#[debug_handler(state=Arc<Mutex<ApiState>>)]
async fn join(
    State(state): State<Arc<Mutex<ApiState>>>,
    Query(JoinQuery { host_id }): Query<JoinQuery>,
) -> Result<impl IntoResponse> {
    dbg!(host_id);
    let (tx, stream) = mpsc::unbounded_channel();
    let disconnected = tx.clone();
    let stream = UnboundedReceiverStream::new(stream);
    let stream = JsonStream { stream };

    let auth = state.lock().await.join(host_id, tx).await?;

    task::spawn(async move {
        disconnected.closed().await;
        let mut state = state.lock().await;
        state.disconnect(auth.user_id).await;
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
    let (phase, seat) = state.lock().await.get_seat(auth).await?;
    let mut phase = phase.lock().await;
    phase.username(seat, username).await
}

#[derive(Deserialize)]
#[serde_as]
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
    let (phase, seat) = state.lock().await.get_seat(auth).await?;
    phase.lock().await.timer(seat, millis).await?;
    Ok(())
}

#[debug_handler(state=Arc<Mutex<ApiState>>)]
async fn start(
    State(state): State<Arc<Mutex<ApiState>>>,
    auth: Auth,
) -> Result<impl IntoResponse> {
    let (phase, seat) = state.lock().await.get_seat(auth).await?;
    let mut phase = phase.lock().await;
    phase.start(seat).await
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
    let (phase, seat) = state.lock().await.get_seat(auth).await?;
    let phase = phase.lock().await;
    let phase = phase.playable(seat, cards);
    phase.await
}

#[debug_handler(state=Arc<Mutex<ApiState>>)]
async fn play(
    State(state): State<Arc<Mutex<ApiState>>>,
    auth: Auth,
    Json(PlayRequest { cards }): Json<PlayRequest>,
) -> Result<impl IntoResponse> {
    let (phase, seat) = state.lock().await.get_seat(auth).await?;
    let mut phase = phase.lock().await;
    phase.human_play(seat, cards).await
}

#[derive(Default)]
struct ApiState {
    phases: HashMap<UserId, Arc<Mutex<dyn Phase>>>,
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

    async fn join(&mut self, host_id: Option<UserId>, tx: UnboundedSender<Respond>) -> Result<Auth> {
        let phase = match host_id {
            Some(host_id) => Arc::clone(self.phases.get(&host_id).ok_or(Error::NoSession)?),
            None => Lobby::new(Weak::clone(&self.this)),
        };
        let auth = phase.lock().await.join(tx).await?;
        self.phases.insert(auth.user_id, phase);
        Ok(auth)
    }

    async fn get_seat(&self, auth: Auth) -> Result<(Arc<Mutex<dyn Phase>>, Seat)> {
        let phase = self.phases.get(&auth.user_id).ok_or(Error::NoSession)?;
        let seat = phase.lock().await;
        let seat = seat.get_seat(auth);
        let seat = seat.await?;
        Ok((Arc::clone(phase), seat))
    }

    async fn disconnect(&mut self, user_id: UserId) {
        // there should be only one thingn in these maps
        if let Some(phase) = self.phases.remove(&user_id) {
            let mut phase = phase.lock().await;
            let _ = phase.disconnect(user_id).await;
        }
    }

    fn transition(&mut self, user_ids: Vec<UserId>, new_phase: Arc<Mutex<dyn Phase>>) {
        for &user_id in user_ids.iter() {
            self.phases.insert(user_id, Arc::clone(&new_phase));
        }
    }
}

#[async_trait]
trait Phase: Send {
    async fn get_seat(&self, auth: Auth) -> Result<Seat>;
    async fn join(&mut self, tx: UnboundedSender<Respond>) -> Result<Auth>;
    async fn timer(&mut self, seat: Seat, timer: Option<Duration>) -> Result<()>;
    async fn username(&mut self, seat: Seat, username: String) -> Result<()>;
    async fn start(&mut self, seat: Seat) -> Result<()>;
    async fn playable(&self, seat: Seat, cards: Cards) -> Result<()>;
    async fn human_play(&mut self, seat: Seat, cards: Cards) -> Result<()>;
    async fn disconnect(&mut self, user_id: UserId) -> Result<()>;
}

struct Lobby {
    timer: Option<Duration>,
    usernames: HashMap<Seat, String>,
    seats: Vec<Seat>,
    seats_left: usize,
    host: Option<Seat>,
    common: Common,
}

impl Lobby {
    fn new(api_state: Weak<Mutex<ApiState>>) -> Arc<Mutex<Self>> {
        Arc::new_cyclic(|_this| {
            let mut seats = Vec::from(Seat::ALL);
            seats.shuffle(&mut thread_rng());
            let timer = None;
            let seats_left = 4;
            let host = None;
            let common = Common::new(api_state);
            let usernames = HashMap::default();
            Mutex::new(Self { timer, usernames, seats, seats_left, host, common })
        })
    }

    fn is_host(&self, seat: Seat) -> bool {
        self.host == Some(seat)
    }
}

#[async_trait]
impl Phase for Lobby {
    async fn get_seat(&self, auth: Auth) -> Result<Seat> {
        self.common.get_seat(auth)
	}

    async fn join(&mut self, tx: UnboundedSender<Respond>) -> Result<Auth> {
        self.seats_left = self.seats_left.checked_sub(1).ok_or(Error::NoSeatsLeft)?;
        let seat = self.seats[self.seats_left];
        let auth = self.common.new_user(seat, tx);
        self.host.get_or_insert(seat);

        self.common.respond(seat, Respond::Welcome { seat }).await;
        if self.is_host(seat) {
            self.common.respond(seat, Respond::Host { }).await;
        }
        for other in Seat::ALL {
            if other != seat {
                self.common.respond(other, Respond::Connected { seat }).await;
            }
            if self.common.is_human(other) && other != seat {
                self
                    .common
                    .respond(seat, Respond::Connected { seat: other })
                    .await;
            }
            if let Some(username) = self.usernames.get(&other) {
                self
                    .common
                    .respond(
                        seat,
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
        self.timer = if too_long {
            None
        } else {
            timer
        };
        Ok(())
	}

    async fn username(&mut self, seat: Seat, username: String) -> Result<()> {
        self.usernames.insert(seat, username.clone());
        for other in Seat::ALL {
            if seat == other {
                continue;
            }
            let username = username.clone();
            self
                .common
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
        let user_ids = Vec::from_iter(self.common.user_ids());
        let common = mem::take(&mut self.common);
        let active = Active::new(timer, common).await;
        api_state.lock().await.transition(user_ids, active);
        Ok(())
	}

    async fn playable(&self, _seat: Seat, _cards: Cards) -> Result<()> {
        Err(Error::BadPhase)
	}

    async fn human_play(&mut self, _seat: Seat, _cards: Cards) -> Result<()> {
        Err(Error::BadPhase)
	}

    async fn disconnect(&mut self, user_id: UserId) -> Result<()> {
        let seat = self.common.disconnect(user_id)?;
        for other in Seat::ALL {
            self.common.respond(other, Respond::Disconnected { seat }).await;
        }

        self.seats_left += 1;
        dbg!(self.seats_left, &self.seats);
        if self.is_host(seat) {
            dbg!(seat);
            self.host = self.seats.iter().rev().cloned()
                .find(|&new_host| self.common.is_human(new_host));
            dbg!(self.host);
            if let Some(new_host) = self.host {
            dbg!(new_host);
                self.common.respond(new_host, Respond::Host { }).await;
            }
        }

        Ok(())
	}
}

struct Active {
    timer: Option<Duration>,
    usernames: HashMap<Seat, String>,
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
            let usernames = HashMap::default();
            Mutex::new(Self { timer, usernames, game_state, deadline, common, this })
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
        self.play(current_player, cards).await.expect("our bots should always choose valid plays");
    }

    async fn play(&mut self, seat: Seat, cards: Cards) -> Result<()> {
        let play = self.game_state.play(cards)?;
        let pass = play.is_pass();
        let load = self.game_state.hand(seat).len();
        let win = load == 0;
        for other in Seat::ALL {
            self
                .common
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
            self
                .common
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
        let user_ids = Vec::from_iter(self.common.user_ids());
        let common = mem::take(&mut self.common);
        let phase = Win::new(common);
        api_state.lock().await.transition(user_ids, phase);
    }
}

#[async_trait]
impl Phase for Active {
    async fn get_seat(&self, auth: Auth) -> Result<Seat> {
        self.common.get_seat(auth)
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

    async fn disconnect(&mut self, user_id: UserId) -> Result<()> {
        let seat = self.common.disconnect(user_id)?;
        for other in Seat::ALL {
            self.common.respond(other, Respond::Disconnected { seat }).await;
        }
        // TODO: what if current player whose turn it is disconnects
        Ok(())
	}

}

struct Win {
    common: Common,
}

impl Win {
    fn new(common: Common) -> Arc<Mutex<Self>> {
        Arc::new_cyclic(|_weak| {
            Mutex::new(Self { common })
        })
    }
}

#[async_trait]
impl Phase for Win {
    async fn get_seat(&self, auth: Auth) -> Result<Seat> {
        self.common.get_seat(auth)
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

    async fn disconnect(&mut self, user_id: UserId) -> Result<()> {
        let seat = self.common.disconnect(user_id)?;
        for other in Seat::ALL {
            self.common.respond(other, Respond::Disconnected { seat }).await;
        }
        Ok(())
	}

}



#[derive(Default)]
struct Common {
    users: HashMap<UserId, Seat>,
    secrets: HashMap<Seat, UserSecret>,
    txs: HashMap<Seat, UnboundedSender<Respond>>,
    api_state: Weak<Mutex<ApiState>>,
}

impl Common {
    fn new(api_state: Weak<Mutex<ApiState>>) -> Self {
        Self {
            users: HashMap::default(),
            secrets: HashMap::default(),
            txs: HashMap::default(),
            api_state,
        }
    }

    fn api_state(&self) -> Arc<Mutex<ApiState>> {
        self.api_state.upgrade().expect("api_state should persist the entire lifetime of the program")
    }

    fn is_human(&self, seat: Seat) -> bool {
        self.txs.contains_key(&seat)
    }

    fn human_seats(&self) -> impl Iterator<Item=Seat> + '_ {
        self.users.values().cloned()
    }

    fn user_ids(&self) -> impl Iterator<Item=UserId> + '_ {
        self.users.keys().cloned()
    }

    fn new_user(&mut self, seat: Seat, tx: UnboundedSender<Respond>) -> Auth {
        let user_id = UserId::random();
        let user_secret = UserSecret::random();
        self.txs.insert(seat, tx);
        self.users.insert(user_id, seat);
        self.secrets.insert(seat, user_secret);
        Auth { user_id, user_secret }
    }

    fn get_seat(&self, auth: Auth) -> Result<Seat> {
        let seat = *self.users.get(&auth.user_id).ok_or(Error::Absent)?;
        if self.secrets[&seat] != auth.user_secret {
            return Err(Error::BadAuthentication);
        }
        Ok(seat)
    }

    fn disconnect(&mut self, user_id: UserId) -> Result<Seat> {
        let seat = self.users.remove(&user_id).ok_or(Error::Absent)?;
        self.txs.remove(&seat);
        self.secrets.remove(&seat);
        Ok(seat)
    }

    async fn respond(&mut self, seat: Seat, response: Respond) {
        let Some(tx) = self.txs.get(&seat) else { return };
        let _ = tx.send(response);
    }
}

#[derive(Serialize)]
#[serde(tag = "event", content = "data")]
#[serde(rename_all = "camelCase")]
#[serde_as]
enum Respond {
    Welcome {
        seat: Seat,
    },
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
    user_id: UserId,
    user_secret: UserSecret,
}

impl IntoResponseParts for Auth {
    type Error = Infallible;
    fn into_response_parts(self, mut res: ResponseParts) -> Result<ResponseParts, Self::Error> {
        let auth = serde_json::to_string(&self).unwrap();
        let auth = general_purpose::STANDARD.encode(auth);
        res.headers_mut().typed_insert(Authorization::bearer(&auth).unwrap());
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
        let TypedHeader(Authorization(token)) = parts.extract::<TypedHeader<Authorization<Bearer>>>()
            .await
            .map_err(|_err| (Error::BadAuthentication.into_response()))?;
        let token = token.token();
        let token = general_purpose::STANDARD.decode(token).map_err(|_| Error::BadAuthentication.into_response())?;
        let auth = serde_json::from_slice(&token).map_err(|_| Error::BadAuthentication.into_response())?;
        Ok(auth)
    }
}


// TODO: for some reason it is considered harmful to use Uuid's (even crypto random 122 bits) as secrets
#[derive(Copy, Clone, Serialize, Deserialize, PartialEq, Eq, Debug, Hash)]
struct UserId(Uuid);

impl UserId {
    fn random() -> Self {
        let ret = thread_rng().gen();
        let ret = uuid::Builder::from_random_bytes(ret).into_uuid();
        Self(ret)
    }
}

#[derive(Copy, Clone, Serialize, Deserialize, PartialEq, Eq, Debug, Hash)]
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
    NoSeatsLeft,
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
            Error::NoSeatsLeft => write!(f, "can only join sessions that aren't full"),
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
            Error::NoSeatsLeft => StatusCode::BAD_REQUEST,
            Error::NotCurrent => StatusCode::BAD_REQUEST,
            Error::PlayError(..) => StatusCode::BAD_REQUEST,
        };
        let body = self.to_string();
        (status, body).into_response()
    }
}

