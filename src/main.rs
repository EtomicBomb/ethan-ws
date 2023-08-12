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
use tokio::sync::mpsc::{self, UnboundedSender};
use axum_extra::extract::cookie::CookieJar;
use tokio::time::sleep;
use std::sync::{Weak};
use axum::debug_handler;
use uuid::Uuid;
use tokio::sync::MutexGuard;
use tokio::time::sleep_until;
use axum::extract::Query;
use futures_util::future::BoxFuture;
use http::HeaderMap;


mod game;
use crate::game::{GameState, choose_play, all_plays, Seat, Cards, Play};

#[tokio::main]
async fn main() {
//    let config = RustlsConfig::from_pem_file( // TODO: revert on deploy
//        "secret/cert.pem",
//        "secret/key.pem",
//    )
//    .await
//    .unwrap();

    let addr = SocketAddr::from(([127, 0, 0, 1], 8000));
    eprintln!("listening on {}", addr);

    let app = app();

//    axum_server::bind_rustls(addr, config) // TODO: revert on deploy
    axum_server::bind(addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

fn app() -> Router {
    let serve_dir = ServeDir::new("www")
        .not_found_service(ServeFile::new("www/not_found.html"))
        .append_index_html_on_directories(true);

    Router::new()
        .nest("/api", api())
        .nest_service("/", serve_dir)
        .layer(SetResponseHeaderLayer::overriding( // TODO: revert debug
            header::CACHE_CONTROL, 
            HeaderValue::from_static("no-store, must-revalidate")
        ))
}

fn api() -> Router {
    let state: Arc<Mutex<ApiState>> = Default::default();

    Router::new()
        .route("/join", post(join))
        .route("/subscribe", get(subscribe))
        .route("/username", put(username))
        .route("/action-timer", put(action_timer))
        .route("/start-game", post(start_game))
        .route("/play", post(play))
        .route("/test", get(|| async { "test endpoint" }))
        .with_state(state)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JoinResponse {
    user_id: UserId, 
}

#[debug_handler(state=Arc<Mutex<ApiState>>)]
async fn join(
    ExtractGameNoSeat { game }: ExtractGameNoSeat,
) -> Result<impl IntoResponse, ()> {
    let mut game = game.lock().await;
    let user_id = game.join().await?;
    // TODO: I want to use Authorization header authentication
    // that would mean setting headers here!
    Ok(Json(JoinResponse { user_id }))
}

#[debug_handler(state=Arc<Mutex<ApiState>>)]
async fn subscribe(
    ExtractGame { game, seat, user_id }: ExtractGame,
    headers: HeaderMap,
) -> impl IntoResponse {

    dbg!(user_id, seat, &headers);

    let game2 = Arc::clone(&game);
    let mut game = game.lock().await;
    let (tx, rx) = mpsc::unbounded_channel();
    let tx2 = tx.clone();
    let rx = UnboundedReceiverStream::new(rx);
    let sse = Sse::new(rx).keep_alive(KeepAlive::default());
    game.subscribe(seat, tx).await;

    task::spawn(async move {
        tx2.closed().await;
        let mut game2 = game2.lock().await;
        game2.disconnect(user_id).await;
    });

    sse
}

#[derive(Deserialize)]
struct UsernameRequest {
    username: String,
}

#[debug_handler(state=Arc<Mutex<ApiState>>)]
async fn username(
    ExtractGame { game, seat, .. }: ExtractGame,
    Json(UsernameRequest { username }): Json<UsernameRequest>,
) -> impl IntoResponse {
    let mut game = game.lock().await;
    game.username(seat, username).await;
    StatusCode::OK
}

#[derive(Deserialize)]
struct ActionTimerRequest {
    millis: Option<DurationMillis>,
}

#[debug_handler(state=Arc<Mutex<ApiState>>)]
async fn action_timer(
    ExtractGame { game, seat, .. }: ExtractGame,
    Json(ActionTimerRequest { millis }): Json<ActionTimerRequest>,
) -> impl IntoResponse {
    let mut game = game.lock().await;
    game.action_timer(seat, millis).await
}

async fn start_game(
    ExtractGame { game, seat, .. }: ExtractGame,
) -> Result<impl IntoResponse, ()> {
    let mut game = game.lock().await;
    game.start_game(seat).await
}

#[derive(Deserialize)]
struct PlayRequest {
    play: Play,
}

async fn play(
    ExtractGame { game, seat, .. }: ExtractGame,
    Json(PlayRequest { play }): Json<PlayRequest>,
) -> Result<impl IntoResponse, ()> {
    let mut game = game.lock().await;
    game.human_play(seat, play).await
}


#[derive(Default)]
struct ApiState {
    games: HashMap<GameId, Arc<Mutex<Game>>>,
}

impl ApiState {
    fn get_game(&mut self, game_id: GameId) -> Arc<Mutex<Game>> {
        // TODO: timeout and delete game if nobody subscribes?
        let game = self.games.entry(game_id)
            .or_insert_with(|| Arc::new_cyclic(|weak| {
                Mutex::new(Game::new(Weak::clone(weak)))
            }));
        Arc::clone(game)
    }
}

struct Game {
    users: HashMap<UserId, Seat>,
    era: Era,
    txs: HashMap<Seat, UserTx>,
    host: Option<Seat>,
    action_timer: Option<Duration>,
    usernames: HashMap<Seat, String>,
    this: Weak<Mutex<Game>>,
}

impl Game {
    fn new(this: Weak<Mutex<Self>>) -> Self {
        let mut seats_left = Vec::from(Seat::ALL);
        seats_left.shuffle(&mut thread_rng());
        let usernames = Seat::ALL.into_iter()
            .map(|seat| (seat, format!("{:?}", seat)))
            .collect();
        Self {
            users: HashMap::default(),
            era: Era::Lobby { seats_left },
            txs: HashMap::default(),
            host: None,
            action_timer: None,
            usernames,
            this,
        }
    }

    fn get_seat(&self, user_id: UserId) -> Option<Seat> {
        self.users.get(&user_id).cloned()
    }

    async fn join(&mut self) -> Result<UserId, ()> {
        let Era::Lobby { ref mut seats_left } = self.era else { return Err(()) };
        let Some(seat) = seats_left.pop() else { return Err(()) };
        let user_id = thread_rng().gen();
        let user_id = uuid::Builder::from_random_bytes(user_id).into_uuid();
        let user_id = UserId(user_id);
        self.users.insert(user_id, seat);
        let is_host = seats_left.len() == 3;
        if is_host { 
            self.host = Some(seat);
        }
        // TODO: ensure that user has subscribed within a couple seconds
        let humans: Vec<_> = self.users.values().cloned().collect();
        for &other in humans.iter() {
            let _ = self.respond(other, &Respond::Connected { seat }).await;
        }
        Ok(user_id)
    }

    async fn subscribe(&mut self, seat: Seat, tx: UserTx) {
        self.txs.insert(seat, tx);
        let _ = self.respond(seat, &Respond::Welcome { seat }).await;
        let humans: Vec<_> = self.users.values().cloned().collect();
        for &other in humans.iter() {
            let _ = self.respond(seat, &Respond::Connected { seat: other }).await;
        }
        for other in Seat::ALL {
            let username = self.usernames[&other].clone();
            let _ = self.respond(seat, &Respond::Username { seat: other, username }).await;
        }

        // TODO: complete new subscription package with era details
    }

    async fn username(&mut self, seat: Seat, username: String) {
        self.usernames.insert(seat, username.clone());
        for other in Seat::ALL {
            if seat == other { continue }
            let username = username.clone();
            let _ = self.respond(other, &Respond::Username { seat, username }).await;
        }
    }

    async fn action_timer(&mut self, seat: Seat, millis: Option<DurationMillis>) -> Result<(), ()> {
        if self.host != Some(seat) { return Err(()) }
        self.action_timer = millis.map(Duration::from);
        Ok(())
    }

    async fn start_game(&mut self, seat: Seat) -> Result<(), ()> {
        // TODO: probably don't want to start a game when there already is one active
        if self.host != Some(seat) { return Err(()) }
        
        self.era = Era::Active { 
            game_state: GameState::default(), 
            deadline: None
        };

        let Era::Active { game_state, .. } = &mut self.era else { unreachable!() };
        let hands = Seat::ALL.map(|other| (other, game_state.hand(other)));
        for (other, cards) in hands {
            let _ = self.respond(other, &Respond::Deal { cards }).await;
        }

        let _ = self.solicit().await;
        Ok(())
    }

    async fn auto_play(&mut self) -> Result<(), ()> {
        let Era::Active { game_state, .. } = &mut self.era else { return Err(()) };
        let current_player = game_state.current_player();
        let play = choose_play(game_state);
        self.play(current_player, play).await
    }

    async fn human_play(&mut self, seat: Seat, play: Play) -> Result<(), ()> {
        let Era::Active { game_state, .. } = &mut self.era else { return Err(()) };
        let current_player = game_state.current_player();
        if seat != current_player { return Err(()) }
        self.play(seat, play).await
    }

    async fn play(&mut self, seat: Seat, play: Play) -> Result<(), ()> {
        let Era::Active { game_state, .. } = &mut self.era else { return Err(()) };

        game_state.play(play).map_err(|_| ())?;

        let load = game_state.hand(seat).len();

        for other in Seat::ALL {
            let _ = self.respond(other, &Respond::Play { seat, load, play }).await;
        }

        let win = load == 0;
        if win {
            self.era = Era::Win { };
        } else {
            let _ = self.solicit().await;
        }

        Ok(())
    }

    async fn disconnect(&mut self, user_id: UserId) {
        let Some(seat) = self.users.remove(&user_id) else { return };
        self.txs.remove(&seat);
        let humans: Vec<_> = self.users.values().cloned().collect();
        for &other in humans.iter() {
            let _ = self.respond(other, &Respond::Disconnected { seat }).await;
        }
        // TODO: what if current player whose turn it is disconnects
        // TODO: handle host disconnect
        // TODO: special case all players disconnect
    }

    async fn solicit(&mut self) -> Result<(), ()> {
        let bot_timer = Duration::from_millis(1000);
        
        let Era::Active { game_state, deadline: old_deadline } = &mut self.era else { return Err(()) };
        let current_player = game_state.current_player();
        let timer = match self.txs.get(&current_player) {
            Some(_) => self.action_timer,
            None => Some(bot_timer),
        };
        let deadline = timer.map(|timer| Instant::now() + timer);
        *old_deadline = deadline;

        let options = game_state.valid_plays();
        let control = game_state.has_control();
        let _ = self.respond(current_player, &Respond::Prompt { 
            options, 
            control,
            timer: timer.map(DurationMillis::from), 
        }).await;

        for other in Seat::ALL {
            if current_player == other { continue }
            let _ = self.respond(other, &Respond::Turn { 
                seat: current_player, 
                control,
                timer: timer.map(DurationMillis::from), 
            }).await;
        }

        let this = Weak::clone(&self.this);
        task::spawn(async move {
            let _ = Self::force_play(this).await;
        });
        Ok(())
    }

    // BoxFuture to break recursion
    fn force_play(this: Weak<Mutex<Self>>) -> BoxFuture<'static, Result<(), ()>> {
        Box::pin(async move {
            {
                let Some(this) = this.upgrade() else { return Err(()) };
                let this = this.lock().await;
                let Era::Active { deadline: Some(deadline), .. } = this.era else { return Err(()) };
                sleep_until(deadline).await;
            }

            let Some(this) = this.upgrade() else { return Err(()) };
            let mut this = this.lock().await;
            let Era::Active { deadline: Some(deadline), .. } = this.era else { return Err(()) };
            if Instant::now() < deadline { return Err(()) }

            // TODO: can choose a worse bot here if we're forcing a human player
            let _ = this.auto_play().await;
            Ok(())
        })
    }

    async fn respond(&mut self, seat: Seat, response: &Respond) -> Result<(), ()> {
        let response = serde_json::to_value(response).unwrap();
        let response = sse::Event::default()
            .json_data(&response["data"]).unwrap()
            .event(response["event"].as_str().unwrap());
//            .retry(Duration::from_millis(1000));
        let response = Ok::<_, Infallible>(response);
        let tx = self.txs.get(&seat).ok_or(())?;
        tx.send(response).map_err(|_| ())?;
        Ok(())
    }
}


enum Era {
    Lobby {
        seats_left: Vec<Seat>,
    },
    Active {
        game_state: GameState,
        deadline: Option<Instant>,
    },
    Win { },
}

type UserTx = UnboundedSender<Result<sse::Event, Infallible>>;

struct ExtractGameNoSeat {
    game: Arc<Mutex<Game>>,
}

#[async_trait]
impl FromRequestParts<Arc<Mutex<ApiState>>> for ExtractGameNoSeat {
    type Rejection = StatusCode;
    async fn from_request_parts(parts: &mut Parts, state: &Arc<Mutex<ApiState>>) -> Result<Self, Self::Rejection> {
        #[derive(Deserialize)]
        struct AuthGameId {
            game_id: GameId,
        }

        let Query(AuthGameId { game_id }) = parts.extract::<Query<AuthGameId>>().await
            .map_err(|_| StatusCode::FORBIDDEN)?;
        let game = state.lock().await.get_game(game_id);
        Ok(Self { game })
    }
}

struct ExtractGame {
    game: Arc<Mutex<Game>>,
    seat: Seat,
    user_id: UserId,
}

#[async_trait]
impl FromRequestParts<Arc<Mutex<ApiState>>> for ExtractGame {
    type Rejection = (StatusCode, &'static str);
    async fn from_request_parts(parts: &mut Parts, state: &Arc<Mutex<ApiState>>) -> Result<Self, Self::Rejection> {
        let Query(Authentication { game_id, user_id }) = parts.extract::<Query<Authentication>>().await
            .map_err(|_| (StatusCode::FORBIDDEN, "expected user and game id: {}"))?;
        let game = state.lock().await.get_game(game_id);
        let seat = game.lock().await.get_seat(user_id)
            .ok_or_else(|| (StatusCode::FORBIDDEN, "could not access game with credentials"))?;
        Ok(Self { game, seat, user_id })
    }
}
        
#[derive(Deserialize)]
struct Authentication {
    game_id: GameId,
    user_id: UserId,
}

#[derive(Serialize)]
#[serde(tag = "event", content = "data")]
#[serde(rename_all = "camelCase")]
enum Respond {
    Welcome {
        seat: Seat,
    },
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
        play: Play,
    },
    Turn {
        seat: Seat,
        control: bool,
        timer: Option<DurationMillis>,
    },
    Prompt {
        options: Vec<Play>,
        control: bool,
        timer: Option<DurationMillis>,
    },
    Disconnected {
        seat: Seat,
    },
}

#[derive(Serialize, Deserialize, Debug, Copy, Clone)]
struct DurationMillis(u64);

impl From<Duration> for DurationMillis {
    fn from(duration: Duration) -> Self {
        DurationMillis(duration.as_millis() as u64)
    }
}

impl From<DurationMillis> for Duration {
    fn from(duration_millis: DurationMillis) -> Self {
        Duration::from_millis(duration_millis.0)
    }
}

#[derive(Copy, Clone, Serialize, Deserialize, PartialEq, Eq, Debug, Hash)]
struct UserId(Uuid);

#[derive(Copy, Clone, Serialize, Deserialize, PartialEq, Eq, Debug, Hash)]
struct GameId(Uuid);

//#[derive(Default)]
//struct ApiState {
//    lobbies: HashMap<GameId, Arc<Mutex<Lobby>>>,
//    actives: HashMap<GameId, Arc<Mutex<Active>>>,
//    sessions: HashMap<SessionId, Session>,
//}
//
//struct Session {
//    game_id: GameId,
//    seat: Seat,
//}
//
//struct Lobby {
//    sinks: HashMap<Seat, UserTx>,
//    host: Seat,
//    seats_left: Vec<Seat>,
//    action_timer: Option<Duration>,
//    usernames: HashMap<Seat, String>,
//}
//
//impl Lobby {
//    async fn new(session_id: SessionId, host_tx: UserTx) -> Self {
//        let mut seats_left = Vec::from(Seat::ALL);
//        seats_left.shuffle(&mut thread_rng());
//        let host_seat = seats_left.pop().unwrap();
//        let sinks = HashMap::default();
//        let action_timer = None;
//        let usernames = HashMap::default();
//        let mut ret = Self { sinks, host: host_seat, seats_left, action_timer, usernames };
//        ret.join_announce(host_seat, host_tx).await; 
//        ret
//    }
//
//    async fn join(
//        &mut self, 
//        tx: UserTx,
//    ) -> Result<(), ()> {
//        let Some(seat) = self.seats_left.pop() else { return Err(()) };
//        self.join_announce(seat, tx).await;
//        Ok(())
//    }
//
//    async fn join_announce(&mut self, seat: Seat, tx: UserTx) {
//        self.sinks.insert(seat, tx);
//
//        todo!("announce")
//
//    }
//    
//    async fn set_username(&mut self, seat: Seat, new_username: String) {
//        self.usernames.insert(seat, new_username);
//        todo!("announce")
//    }
//}
//
//struct Active {
//    sinks: HashMap<Seat, UserTx>,
//    action_timer: Option<Duration>,
//    start_timestamp: Instant,
//    last_play: Instant,
//    game_state: GameState,
//}
//
//struct ExtractLobby {
//    lobby: Arc<Mutex<Lobby>>,
//}
//
//#[async_trait]
//impl FromRequestParts<Arc<Mutex<ApiState>>> for ExtractLobby {
//    type Rejection = StatusCode;
//    async fn from_request_parts(parts: &mut Parts, state: &Arc<Mutex<ApiState>>) -> Result<Self, Self::Rejection> {
//        let game_id = parts.extract::<Query<Authentication>>().await.map_err(|_| StatusCode::IM_A_TEAPOT)?;
//        let state = state.lock().await;
//        let lobby = state.lobbies.get(&game_id).ok_or(StatusCode::IM_A_TEAPOT)?;
//
//        Ok(Self { lobby: Arc::clone(lobby) })
//    }
//}
//
//struct ExtractLobbySeat {
//    seat: Seat,
//    lobby: Arc<Mutex<Lobby>>,
//}
//
//#[async_trait]
//impl FromRequestParts<Arc<Mutex<ApiState>>> for ExtractLobbySeat {
//    type Rejection = StatusCode;
//    async fn from_request_parts(parts: &mut Parts, state: &Arc<Mutex<ApiState>>) -> Result<Self, Self::Rejection> {
//        let jar = parts.extract::<CookieJar>().await.expect("infallible");
//        let session_id = jar.get("session-id").ok_or(StatusCode::IM_A_TEAPOT)?;
//        let session_id: SessionId = session_id.value().parse().map_err(|_| StatusCode::IM_A_TEAPOT)?;
//        
//        let state = state.lock().await;
//        let Session { game_id, seat } = state.sessions.get(&session_id).ok_or(StatusCode::IM_A_TEAPOT)?;
//        let lobby = state.lobbies.get(&game_id).ok_or(StatusCode::IM_A_TEAPOT)?;
//
//        Ok(Self { seat: seat.clone(), lobby: Arc::clone(lobby) })
//    }
//}
//
//struct ExtractActiveSeat {
//    seat: Seat,
//    active: Arc<Mutex<Active>>,
//}
//
//#[async_trait]
//impl FromRequestParts<Arc<Mutex<ApiState>>> for ExtractActiveSeat {
//    type Rejection = StatusCode;
//    async fn from_request_parts(parts: &mut Parts, state: &Arc<Mutex<ApiState>>) -> Result<Self, Self::Rejection> {
//        let jar = parts.extract::<CookieJar>().await.expect("infallible");
//        let session_id = jar.get("session-id").ok_or(StatusCode::IM_A_TEAPOT)?;
//        let session_id: SessionId = session_id.value().parse().map_err(|_| StatusCode::IM_A_TEAPOT)?;
//        
//        let state = state.lock().await;
//        let Session { game_id, seat } = state.sessions.get(&session_id).ok_or(StatusCode::IM_A_TEAPOT)?;
//        let active = state.actives.get(&game_id).ok_or(StatusCode::IM_A_TEAPOT)?;
//
//        Ok(Self { seat: seat.clone(), active: Arc::clone(active) })
//    }
//}


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
