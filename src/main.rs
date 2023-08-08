#![allow(dead_code)]
#![allow(unused_imports)]

use axum::{
    routing::{get, post, put}, 
    Router,
    extract::{Path, State},
    extract::ws::{WebSocket, Message},
    response::Response,
    Json,
};
use axum_server::tls_rustls::RustlsConfig;
use std::{
    net::SocketAddr,
    collections::{HashMap, HashSet},
    time::{Duration},
    sync::{Arc},
    ops::ControlFlow,
};
use tower_http::{
    services::{ServeDir, ServeFile},
    set_header::SetResponseHeaderLayer,
};
use tower::{
    ServiceExt,
};
use http::{
    header,
    header::HeaderValue,
};
use tokio::{
    time::Instant,
    sync::{Mutex},
    task,
};
use serde::{Serialize, Deserialize};
use futures_util::{
    sink::SinkExt, 
    stream::{SplitSink, SplitStream},
};
use rand::{thread_rng, Rng, seq::SliceRandom};
use axum::{RequestPartsExt, async_trait};
use std::num::ParseIntError;
use http::status::StatusCode;
use http::request::Parts;
use std::fmt::{self, Display};
use axum::extract::FromRequestParts;
use std::str::FromStr;
use http::header::{AUTHORIZATION, SET_COOKIE};
use axum::response::{AppendHeaders, IntoResponse};
use cookie::{Cookie, SameSite};
use axum::response::sse::{self, KeepAlive, Sse};
use tokio_stream::{StreamExt, wrappers::UnboundedReceiverStream};
use futures_util::stream::{self, Stream};
use std::convert::Infallible;
use tokio::sync::mpsc::UnboundedSender;
use axum_extra::extract::cookie::CookieJar;
use tokio::time::sleep;
use std::sync::{Weak};

mod game;
use crate::game::{GameState, choose_play, all_plays, Seat};

#[tokio::main]
async fn main() {
//    let config = RustlsConfig::from_pem_file(
//        "secret/cert.pem",
//        "secret/key.pem",
//    )
//    .await
//    .unwrap();

    let addr = SocketAddr::from(([127, 0, 0, 1], 8000));
    eprintln!("listening on {}", addr);

    let app = app();

//    axum_server::bind_rustls(addr, config)
    axum_server::bind(addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

// ideas for refactoring:
// tokens (session_id, game_id) created by the client
// no separate endpoints for creating and joining, just a single "subscribe" endpoint

fn app() -> Router {
    Router::new()
        .nest("/api", api())
        .nest_service("/", ServeDir::new("www").not_found_service(ServeFile::new("www/not_found.html")))
        .layer(SetResponseHeaderLayer::overriding(
            header::CACHE_CONTROL, 
            HeaderValue::from_static("no-store, must-revalidate")
        ))
        .layer(SetResponseHeaderLayer::overriding(
            header::EXPIRES, 
            HeaderValue::from_static("0")
        ))
}

fn api() -> Router {
    let state: Arc<Mutex<ApiState>> = Default::default();

    Router::new()
        .route("/games", post(create_game))
        .route("/lobby/:game_id", post(join_lobby))
        .route("/subscribe", get(subscribe))
        .route("/username", put(set_username))
        .route("/action-timer", post(start))
        .route("/play", post(play))
        .route("/can-play", post(can_play))
        .with_state(state)
}


async fn create_game(
    jar: CookieJar,
    State(api_state): State<Arc<Mutex<ApiState>>>,
) -> impl IntoResponse {
    let session_id = SessionId(thread_rng().gen());
    let cookie = Cookie::build("session-id", session_id.to_string())
        .secure(true)
        .same_site(SameSite::Strict)
        .permanent()
        .finish();
    let jar = jar.add(cookie);
    
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    let rx = UnboundedReceiverStream::new(rx);

    let mut lobby = lobby.lock().await;
    lobby.join(tx).await?;
    let sse = Sse::new(rx).keep_alive(KeepAlive::default());

    let game_id = GameId(thread_rng().gen());
    let lobby = Arc::new(Mutex::new(Lobby::new(tx)));
    api_state.lock().await.lobbies.insert(game_id, lobby);
    api_state.lock().await.sessions.insert(session_id, Session {
        game_id,  
    });


    Ok((jar, sse))
}

use axum::debug_handler;

#[debug_handler(state=Arc<Mutex<ApiState>>)]
async fn join_lobby(
    jar: CookieJar,
    ExtractLobby { lobby }: ExtractLobby,
) -> Result<impl IntoResponse, ()> {
    let id = SessionId(thread_rng().gen());
    let cookie = Cookie::build("session-id", id.to_string())
        .secure(true)
        .same_site(SameSite::Strict)
        .permanent()
        .finish();
    let jar = jar.add(cookie);
    
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    let rx = UnboundedReceiverStream::new(rx);
    let sse = Sse::new(rx).keep_alive(KeepAlive::default());

    let mut lobby = lobby.lock().await;
    lobby.new(tx).await?;

    Ok((jar, sse))
}

#[derive(Deserialize)]
struct UsernameRequest {
    new_username: String,
}

#[debug_handler(state=Arc<Mutex<ApiState>>)]
async fn set_username(
    ExtractLobbySeat { seat, lobby }: ExtractLobbySeat,
    Json(UsernameRequest { new_username }): Json<UsernameRequest>,
) {
    let mut lobby = lobby.lock().await;
    lobby.set_username(seat, new_username).await
}


async fn start() -> &'static str {
    "create game"
}

async fn play() -> &'static str {
    "create game"
}

async fn can_play() -> &'static str {
    "can play"
}

#[derive(Default)]
struct ApiState {
    lobbies: HashMap<GameId, Arc<Mutex<Lobby>>>,
    actives: HashMap<GameId, Arc<Mutex<Active>>>,
    sessions: HashMap<SessionId, Session>,
}

struct Session {
    game_id: GameId,
    seat: Seat,
}

type UserTx = UnboundedSender<Result<sse::Event, Infallible>>;

struct Lobby {
    sinks: HashMap<Seat, UserTx>,
    host: Seat,
    seats_left: Vec<Seat>,
    action_timer: Option<Duration>,
    usernames: HashMap<Seat, String>,
}

impl Lobby {
    async fn new(session_id: SessionId, host_tx: UserTx) -> Self {
        let mut seats_left = Vec::from(Seat::ALL);
        seats_left.shuffle(&mut thread_rng());
        let host_seat = seats_left.pop().unwrap();
        let sinks = HashMap::default();
        let action_timer = None;
        let usernames = HashMap::default();
        let mut ret = Self { sinks, host: host_seat, seats_left, action_timer, usernames };
        ret.join_announce(host_seat, host_tx).await; 
        ret
    }

    async fn join(
        &mut self, 
        tx: UserTx,
    ) -> Result<(), ()> {
        let Some(seat) = self.seats_left.pop() else { return Err(()) };
        self.join_announce(seat, tx).await;
        Ok(())
    }

    async fn join_announce(&mut self, seat: Seat, tx: UserTx) {
        self.sinks.insert(seat, tx);

        todo!("announce")

    }
    
    async fn set_username(&mut self, seat: Seat, new_username: String) {
        self.usernames.insert(seat, new_username);
        todo!("announce")
    }
}

struct Active {
    sinks: HashMap<Seat, UserTx>,
    action_timer: Option<Duration>,
    start_timestamp: Instant,
    last_play: Instant,
    game_state: GameState,
}

struct ExtractLobby {
    lobby: Arc<Mutex<Lobby>>,
}

#[async_trait]
impl FromRequestParts<Arc<Mutex<ApiState>>> for ExtractLobby {
    type Rejection = StatusCode;
    async fn from_request_parts(parts: &mut Parts, state: &Arc<Mutex<ApiState>>) -> Result<Self, Self::Rejection> {
        let game_id = parts.extract::<Path<GameId>>().await.map_err(|_| StatusCode::IM_A_TEAPOT)?;
        let state = state.lock().await;
        let lobby = state.lobbies.get(&game_id).ok_or(StatusCode::IM_A_TEAPOT)?;

        Ok(Self { lobby: Arc::clone(lobby) })
    }
}

struct ExtractLobbySeat {
    seat: Seat,
    lobby: Arc<Mutex<Lobby>>,
}

#[async_trait]
impl FromRequestParts<Arc<Mutex<ApiState>>> for ExtractLobbySeat {
    type Rejection = StatusCode;
    async fn from_request_parts(parts: &mut Parts, state: &Arc<Mutex<ApiState>>) -> Result<Self, Self::Rejection> {
        let jar = parts.extract::<CookieJar>().await.expect("infallible");
        let session_id = jar.get("session-id").ok_or(StatusCode::IM_A_TEAPOT)?;
        let session_id: SessionId = session_id.value().parse().map_err(|_| StatusCode::IM_A_TEAPOT)?;
        
        let state = state.lock().await;
        let Session { game_id, seat } = state.sessions.get(&session_id).ok_or(StatusCode::IM_A_TEAPOT)?;
        let lobby = state.lobbies.get(&game_id).ok_or(StatusCode::IM_A_TEAPOT)?;

        Ok(Self { seat: seat.clone(), lobby: Arc::clone(lobby) })
    }
}

struct ExtractActiveSeat {
    seat: Seat,
    active: Arc<Mutex<Active>>,
}

#[async_trait]
impl FromRequestParts<Arc<Mutex<ApiState>>> for ExtractActiveSeat {
    type Rejection = StatusCode;
    async fn from_request_parts(parts: &mut Parts, state: &Arc<Mutex<ApiState>>) -> Result<Self, Self::Rejection> {
        let jar = parts.extract::<CookieJar>().await.expect("infallible");
        let session_id = jar.get("session-id").ok_or(StatusCode::IM_A_TEAPOT)?;
        let session_id: SessionId = session_id.value().parse().map_err(|_| StatusCode::IM_A_TEAPOT)?;
        
        let state = state.lock().await;
        let Session { game_id, seat } = state.sessions.get(&session_id).ok_or(StatusCode::IM_A_TEAPOT)?;
        let active = state.actives.get(&game_id).ok_or(StatusCode::IM_A_TEAPOT)?;

        Ok(Self { seat: seat.clone(), active: Arc::clone(active) })
    }
}

#[derive(Clone, Copy, Hash, Eq, PartialEq, PartialOrd, Ord, Serialize, Deserialize)]
struct SessionId(u64);

impl FromStr for SessionId {
    type Err = ParseIntError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(SessionId(s.parse()?))
    }
}

impl Display for SessionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Clone, Copy, Hash, Eq, PartialEq, PartialOrd, Ord, Serialize, Deserialize)]
struct GameId(u64);

//#[derive(Default)]
//struct AppState {
//    // TODO: how is a game going to be removed from this map? should we store a weak as the value?
//    games: HashMap<GameId, Arc<Mutex<GameInfo>>>,
//}
//
//impl AppState {
//    fn new_game(host_tx: SplitSink<WebSocket, Message>) -> (GameId, Arc<Mutex<GameInfo>>) {
//        let game_id = GameId(thread_rng().gen());
//        let game = Arc::new(Mutex::new(GameInfo::new(host_tx)));
//        self.games.insert(game_id, Arc::clone(&game));
//        (game_id, game)
//    }
//
//    fn get_game(&mut self, game_id: GameId) -> Result<Arc<Mutex<GameInfo>>, ()> {
//    async fn current_player() -> Seat {
//        todo!()
//    }
//
//        let Some(game) = self.games.get(&game_id) else { return Err(()) }
//        Ok(Arc::clone(game))
//    }
//}


// https://github.com/tokio-rs/axum/blob/main/examples/jwt/src/main.rs

//tower_http::set_header::SetResponseHeaderLayer
//
//let service = ServiceBuilder::new()
//    .layer(AsyncRequireAuthorizationLayer::new(|request: Request<Body>| async move {
//        if let Some(user_id) = check_auth(&request).await {
//            Ok(request)
//        } else {
//            let unauthorized_response = Response::builder()
//                .status(StatusCode::UNAUTHORIZED)
//                .body(Body::empty())
//                .unwrap();
//
//            Err(unauthorized_response)
//        }
//    }))
//    .service_fn(handle);
//
//#[async_trait]
//impl<S> FromRequestParts<S> for AuthenticatedUser
//where
//    S: Send + Sync,
//{
//    type Rejection = Response;
//
//    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
//        // You can either call them directly...
//        let TypedHeader(Authorization(token)) =
//            TypedHeader::<Authorization<Bearer>>::from_request_parts(parts, state)
//                .await
//                .map_err(|err| err.into_response())?;
//
//        // ... or use `extract` / `extract_with_state` from `RequestExt` / `RequestPartsExt`
//        use axum::RequestPartsExt;
//        let Extension(state) = parts.extract::<Extension<State>>()
//            .await
//            .map_err(|err| err.into_response())?;
//
//        unimplemented!("actually perform the authorization")
//    }
//}
//
//async fn request_game(
//    ws: WebSocketUpgrade,
//    State(state): State<Arc<Mutex<AppState>>>,
//) -> Response {
//    ws.on_upgrade(|socket| async move {
//        let (mut tx, mut rx) = socket.split();
//        
//        let (game_id, game) = state.lock().unwrap().new_game(tx);
//
//        spin_message_listener(game_id, user_id, game, rx).await
//    })
//}
//
//async fn request_join(
//    ws: WebSocketUpgrade,
//    Path(game_id): Path<GameId>,
//    State(state): State<Arc<Mutex<AppState>>>,
//) -> Response {
//    ws.on_upgrade(|socket| async move {
//        let (mut tx, mut rx) = socket.split();
//
//        let game = state.lock().unwrap().get_game(game_id);
//        let Ok(user_id) = game.lock().unwrap().new_user(tx, false) else { return };
//
//        spin_message_listener(game_id, user_id, game).await
//    })
//}
//
//async fn spin_message_listener(
//    game_id: GameId, 
//    seat: Seat, 
//    state: Arc<Mutex<GameInfo>>,
//    rx: SplitStream<WebSocket>,
//) {
//    while let Some(Ok(message)) = rx.recv().await {
//        let Message::Text(message) = message else { break };
//
//        let Ok(message) = serde_json::from_str(&message) else { break };
//
//        let result = state.lock().unwrap().handle_message(user_id, message);
//        if result.is_break() { break }
//    }
//}
//
//
//#[derive(Serialize, Deserialize)]
//#[serde(tag = "kind")]
//enum ClientToServer {
//    StartGame {},
//    SetActionTimer {
//        timer_millis: Option<u64>,
//    },
//    SetUsername {
//        username: String,
//    },
//    Play {
//        cards: Cards, 
//    },
//}
//
//#[derive(Serialize, Deserialize)]
//#[serde(tag = "kind")]
//#[serde_as]
//enum ServerToClient {
//    PlayerJoined {
//        seat: Seat,
//    },
//    SetUsername {
//        seat: Seat,
//        username: String,
//    },
//    Welcome {
//        usernames: HashMap<Seat, String>,
//    },
//    Deal {
//        hand: Cards,
//        plays: Vec<Play>,
//        timestamp: u64,
//        starting_player: Seat,
//    },
//    Played {
//        default: bool,
//        seat: Seat,
//        play: Play, 
//        timestamp: u64,
//    },
//    InvalidPlay {
//        play: Play,
//    },
//    Disconnected {
//        seat: Seat,
//    },
//}
//

//#[derive(Default)]
//struct GameInfo {
//    Lobby {
//        weak: Weak<Mutex<GameInfo>>,
//        sinks: HashMap<Seat, SplitSink<WebSocket, Message>>,
//        action_timer: Option<Duration>,
//        usernames: HashMap<Seat, String>,
//        seats_left: Vec<Seat>,
//        host: Seat,
//    },
//    Active {
//        weak: Weak<Mutex<GameInfo>>,
//        sinks: HashMap<Seat, SplitSink<WebSocket, Message>>,
//        action_timer: Option<Duration>,
//        start_timestamp: Instant,
//        last_play: Instant,
//        game_state: GameState,
//    },
//}
//
//use futures::SinkExt;
//impl GameInfo {
//    fn new(host_tx: SplitSink<WebSocket, Message>, weak: Weak<Mutex<GameInfo>>) -> GameInfo {
//        let mut seats_left = Vec::from(Seat::ALL);
//        seats_left.shuffle(&mut thread_rng());
//        let host = seats_left.pop().unwrap();
//        let sinks = HashMap::from([(host, host_tx)]);
//        
//        // TODO: announce
//    
//        let action_timer = None;
//        GameInfo::Lobby { sinks, seats_left, action_timer, host, weak }
//    }
//
//    fn is_human(&self, seat: Seat) -> bool {
//        match self {
//            GameInfo::Lobby { sinks, .. } => sinks.contains_key(&seat),
//            GameInfo::Active { sinks, .. } => sinks.contains_key(&seat),
//        }
//    }
//
//    /// destination must be human (not a bot)
//    async fn to_client(&mut self, destination: Seat, message: &ServerToClient) {
//        let destination = match self {
//            GameInfo::Lobby { sinks, .. } => sinks[&seat],
//            GameInfo::Active { sinks, .. } => sinks[&seat],
//        };
//        let message = serde_json::to_string(message).unwrap();
//        let message = Message::Text(message);
//
//        let _ = destination.send(message).await; // TODO: error from sending
//    }
//
//    async fn new_user(
//        &mut self, 
//        tx: SplitSink<WebSocket, Message>, 
//    ) -> Result<Seat, ()> {
//        match self {
//            GameInfo::Lobby { seating, sinks, seats_left, .. } if !seats_left.is_empty() => {
//                let seat = seats_left.pop().unwrap();
//                sinks.insert(user_id, tx);
//
//                // TODO: announce
//                // TODO: send welcome
//
//                Ok(user_id)
//            },
//            GameInfo::Lobby { .. } => Err(()), // full lobby
//            GameInfo::Game { .. } => Err(()), // cannot join while in progress
//        }
//    }
//
//    async fn handle_message(&mut self, seat: Seat, message: ClientToServer) -> ControlFlow<()> {
//        match message {
//            ClientToServer::StartGame { } => self.start_game(seat),
//            ClientToServer::SetActionTimer { timer_millis } => 
//                self.set_action_timer_millis(seat, timer_millis), 
//            ClientToServer::SetUsername { username } => 
//                self.set_username(seat, username),
//            ClientToServer::Play { cards } => self.play(seat, cards),
//            ClientToServer::ValidPlays => self.valid_plays(seat, cards),
//        }
//    }
//
//    async fn start_game(&mut self, seat: Seat) -> ControlFlow<()> {
//        let GameInfo::Lobby { sinks, seats_left, host, action_timer, weak } = self 
//            else { return ControlFlow::Break<()> };
//        if host != seat { return ControlFlow::Break<()> } 
//
//        // TODO: announce
//        // TODO: ai player might be first
//
//        let weak = mem::take(weak);
//        let sinks = mem::take(sinks);
//        let last_play = SystemTime::now();
//        let game_state = GameState::default();
//        *self = GameInfo::Active { sinks, last_play, game_state  };
//
//        for other in sinks.keys() {
//            
//
//        }
//    }
//
//    async fn set_action_timer(&mut self, seat: Seat, timer_millis: Option<u64>) -> ControlFlow<()> {
//        let GameInfo::Lobby { action_timer, .. } = self 
//            else { return ControlFlow::Break<()> };
//        action_timer = timer_millis.map(Duration::from_millis);
//    }
//
//    async fn set_username(&mut self, seat: Seat, username: String) -> ControlFlow<()> {
//        let message = ServerToClient::SetUsername { seat, username: username.clone() }; 
//        match self {
//            GameInfo::Lobby { usernames, sinks, .. } => { 
//                usernames.insert(seat, username); 
//                for &other in self.sinks.keys() {
//                    self.to_client(other, &message);
//                }
//            },
//            GameInfo::Active { sinks, .. } => {
//                for &other in self.sinks.keys() {
//                    self.to_client(other, &message);
//                }
//            },
//        }
//
//        ControlFlow::Continue(())
//    }
//
//    const BOT_TIMER: Duration = Duration::from_millis(500);
//
//    /// Handle the action timer
//    async fn alarm(&mut self) {
//        let GameInfo::Active { last_play, game_state, action_timer, .. } = self 
//            else { panic!() };
//
//        let now = Instant::now();
//        
//        let current_player = self.game_state.current_player();
//    
//        if self.is_human(current_player) {
//            if let Some(action_timer) = *action_timer {
//                if *last_play + action_timer <= now { // ring ring ring
//                    self.play(self.game_state.current_player(), None).await;
//                }
//            }
//        } else {
//            if *last_play + BOT_TIMER <= now {
//                self.play(self.current_player(), None).await;
//            }
//        }
//
//        // TODO: play default action
//
//        last_play + action_timer
//    }
//
//    async fn play(&mut self, seat: Seat, play: Option<Play>) -> ControlFlow<()> {
//        let GameInfo::Active { start_timestamp, sinks, last_play, game_state, action_timer, weak, .. } = self 
//            else { return ControlFlow::Break(()) };
//
//        let weak = Weak::clone(weak);
//
//        if self.current_player() != seat { return ControlFlow::Continue(()) }
//
//        let play = play.unwrap_or_else(|| choose_play(game_state));
//        
//        match game_state.play(play) {
//            Ok(()) => {},
//            Err(()) => {
//                // TODO: invalid play
//            },
//        }
//
//        // TODO: announce
//        
//        *last_play = Instant::now();
//
//        if self.is_human(self.current_player()) {
//            if let Some(action_timer) = *action_timer {
//                task::spawn(async move {
//                    sleep(action_timer).await;
//                    if let Some(game) = weak.upgrade() {
//                        let game = game.lock().unwrap();
//                        game.alarm().await;
//                    })
//                });
//            }
//
//        } else {
//            task::spawn(async move {
//                sleep(BOT_TIMER).await;
//                if let Some(game) = weak.upgrade() {
//                    let game = game.lock().unwrap();
//                    game.alarm().await;
//                })
//            });
//        }
//
//    }
//
//    async fn valid_plays(&mut self, seat: Seat, cards: Cards) -> ControlFlow<()> {
//
//    }
//}
//
//
