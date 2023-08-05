#![allow(dead_code)]
#![allow(unused_imports)]

use axum::{
    routing::{get, post}, 
    Router,
    extract::State,
    extract::ws::{WebSocket, Message},
    Response,
    Json,
};
use axum_server::tls_rustls::RustlsConfig;
use std::{
    net::SocketAddr,
    collections::{HashMap, HashSet},
    time::{Instant, Duration},
    sync::{Arc, Mutex},
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
    task,
};
use serde::{Serialize, Deserialize};
use futures_util::{
    sink::SinkExt, 
    stream::{StreamExt, SplitSink, SplitStream},
};
use rand::{thread_rng, Rng, SliceRandom};

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

#[derive(Clone, Copy, Hash, Eq, PartialEq, PartialOrd, Ord, Serialize, Deserialize)]
struct GameId(u64);

#[derive(Default)]
struct AppState {
    // TODO: how is a game going to be removed from this map? should we store a weak as the value?
    games: HashMap<GameId, Arc<Mutex<GameInfo>>>,
}

impl AppState {
    fn new_game(host_tx: SplitSink<WebSocket, Message>) -> (GameId, Arc<Mutex<GameInfo>>) {
        let game_id = GameId(thread_rng().gen());
        let game = Arc::new(Mutex::new(GameInfo::new(host_tx)));
        self.games.insert(game_id, Arc::clone(&game));
        (game_id, game)
    }

    fn get_game(&mut self, game_id: GameId) -> Result<Arc<Mutex<GameInfo>>, ()> {
    async fn current_player() -> Seat {
        todo!()
    }

        let Some(game) = self.games.get(&game_id) else { return Err(()) }
        Ok(Arc::clone(game))
    }
}

fn app() -> Router {
    let static_server = ServeDir::new("www")
        .not_found_service(ServeFile::new("www/not_found.html"));

    let app_state: Arc<Mutex<AppState>> = Default::default();

    Router::new()
        .route("/v1/test", get(|| async { "Test Successful!" }))
        .route("/v1/request_game", post(request_game))
        .route("/v1/request_join/:game_id", post(request_seat))
        // CREATE /pusoy/lobby -> game_id
        // CREATE /pusoy/lobby/:game_id/ -> (user id token, seat)
        // GET /pusoy/lobby/:game_id -> sse stream
        // pusoy/lobby/:game_id/
        .nest_service("/", static_server.clone())
        .with_state(app_state)
        .layer(SetResponseHeaderLayer::overriding(
            header::CACHE_CONTROL, 
            HeaderValue::from_static("no-store, must-revalidate")
        ))
        .layer(SetResponseHeaderLayer::overriding(
            header::EXPIRES, 
            HeaderValue::from_static("0")
        ))
}

async fn request_game(
    ws: WebSocketUpgrade,
    State(state): State<Arc<Mutex<AppState>>>,
) -> Response {
    ws.on_upgrade(|socket| async move {
        let (mut tx, mut rx) = socket.split();
        
        let (game_id, game) = state.lock().unwrap().new_game(tx);

        spin_message_listener(game_id, user_id, game, rx).await
    })
}

async fn request_join(
    ws: WebSocketUpgrade,
    Path(game_id): Path<GameId>,
    State(state): State<Arc<Mutex<AppState>>>,
) -> Response {
    ws.on_upgrade(|socket| async move {
        let (mut tx, mut rx) = socket.split();

        let game = state.lock().unwrap().get_game(game_id);
        let Ok(user_id) = game.lock().unwrap().new_user(tx, false) else { return };

        spin_message_listener(game_id, user_id, game).await
    })
}

async fn spin_message_listener(
    game_id: GameId, 
    seat: Seat, 
    state: Arc<Mutex<GameInfo>>,
    rx: SplitStream<WebSocket>,
) {
    while let Some(Ok(message)) = rx.recv().await {
        let Message::Text(message) = message else { break };

        let Ok(message) = serde_json::from_str(&message) else { break };

        let result = state.lock().unwrap().handle_message(user_id, message);
        if result.is_break() { break }
    }
}


#[derive(Serialize, Deserialize)]
#[serde(tag = "kind")]
enum ClientToServer {
    StartGame {},
    SetActionTimer {
        timer_millis: Option<u64>,
    },
    SetUsername {
        username: String,
    },
    Play {
        cards: Cards, 
    },
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "kind")]
#[serde_as]
enum ServerToClient {
    PlayerJoined {
        seat: Seat,
    },
    SetUsername {
        seat: Seat,
        username: String,
    },
    Welcome {
        usernames: HashMap<Seat, String>,
    },
    Deal {
        hand: Cards,
        plays: Vec<Play>,
        timestamp: u64,
        starting_player: Seat,
    },
    Played {
        default: bool,
        seat: Seat,
        play: Play, 
        timestamp: u64,
    },
    InvalidPlay {
        play: Play,
    },
    Disconnected {
        seat: Seat,
    },
}

#[derive(Default)]
struct GameInfo {
    Lobby {
        weak: Weak<Mutex<GameInfo>>,
        sinks: HashMap<Seat, SplitSink<WebSocket, Message>>,
        action_timer: Option<Duration>,
        usernames: HashMap<Seat, String>,
        seats_left: Vec<Seat>,
        host: Seat,
    },
    Active {
        weak: Weak<Mutex<GameInfo>>,
        sinks: HashMap<Seat, SplitSink<WebSocket, Message>>,
        action_timer: Option<Duration>,
        start_timestamp: Instant,
        last_play: Instant,
        game_state: GameState,
    },
}

use futures::SinkExt;
impl GameInfo {
    fn new(host_tx: SplitSink<WebSocket, Message>, weak: Weak<Mutex<GameInfo>>) -> GameInfo {
        let mut seats_left = Vec::from(Seat::ALL);
        seats_left.shuffle(&mut thread_rng());
        let host = seats_left.pop().unwrap();
        let sinks = HashMap::from([(host, host_tx)]);
        
        // TODO: announce
    
        let action_timer = None;
        GameInfo::Lobby { sinks, seats_left, action_timer, host, weak }
    }

    fn is_human(&self, seat: Seat) -> bool {
        match self {
            GameInfo::Lobby { sinks, .. } => sinks.contains_key(&seat),
            GameInfo::Active { sinks, .. } => sinks.contains_key(&seat),
        }
    }

    /// destination must be human (not a bot)
    async fn to_client(&mut self, destination: Seat, message: &ServerToClient) {
        let destination = match self {
            GameInfo::Lobby { sinks, .. } => sinks[&seat],
            GameInfo::Active { sinks, .. } => sinks[&seat],
        };
        let message = serde_json::to_string(message).unwrap();
        let message = Message::Text(message);

        let _ = destination.send(message).await; // TODO: error from sending
    }

    async fn new_user(
        &mut self, 
        tx: SplitSink<WebSocket, Message>, 
    ) -> Result<Seat, ()> {
        match self {
            GameInfo::Lobby { seating, sinks, seats_left, .. } if !seats_left.is_empty() => {
                let seat = seats_left.pop().unwrap();
                sinks.insert(user_id, tx);

                // TODO: announce
                // TODO: send welcome

                Ok(user_id)
            },
            GameInfo::Lobby { .. } => Err(()), // full lobby
            GameInfo::Game { .. } => Err(()), // cannot join while in progress
        }
    }

    async fn handle_message(&mut self, seat: Seat, message: ClientToServer) -> ControlFlow<()> {
        match message {
            ClientToServer::StartGame { } => self.start_game(seat),
            ClientToServer::SetActionTimer { timer_millis } => 
                self.set_action_timer_millis(seat, timer_millis), 
            ClientToServer::SetUsername { username } => 
                self.set_username(seat, username),
            ClientToServer::Play { cards } => self.play(seat, cards),
            ClientToServer::ValidPlays => self.valid_plays(seat, cards),
        }
    }

    async fn start_game(&mut self, seat: Seat) -> ControlFlow<()> {
        let GameInfo::Lobby { sinks, seats_left, host, action_timer, weak } = self 
            else { return ControlFlow::Break<()> };
        if host != seat { return ControlFlow::Break<()> } 

        // TODO: announce
        // TODO: ai player might be first

        let weak = mem::take(weak);
        let sinks = mem::take(sinks);
        let last_play = SystemTime::now();
        let game_state = GameState::default();
        *self = GameInfo::Active { sinks, last_play, game_state  };

        for other in sinks.keys() {
            

        }
    }

    async fn set_action_timer(&mut self, seat: Seat, timer_millis: Option<u64>) -> ControlFlow<()> {
        let GameInfo::Lobby { action_timer, .. } = self 
            else { return ControlFlow::Break<()> };
        action_timer = timer_millis.map(Duration::from_millis);
    }

    async fn set_username(&mut self, seat: Seat, username: String) -> ControlFlow<()> {
        let message = ServerToClient::SetUsername { seat, username: username.clone() }; 
        match self {
            GameInfo::Lobby { usernames, sinks, .. } => { 
                usernames.insert(seat, username); 
                for &other in self.sinks.keys() {
                    self.to_client(other, &message);
                }
            },
            GameInfo::Active { sinks, .. } => {
                for &other in self.sinks.keys() {
                    self.to_client(other, &message);
                }
            },
        }

        ControlFlow::Continue(())
    }

    const BOT_TIMER: Duration = Duration::from_millis(500);

    /// Handle the action timer
    async fn alarm(&mut self) {
        let GameInfo::Active { last_play, game_state, action_timer, .. } = self 
            else { panic!() };

        let now = Instant::now();
        
        let current_player = self.game_state.current_player();
    
        if self.is_human(current_player) {
            if let Some(action_timer) = *action_timer {
                if *last_play + action_timer <= now { // ring ring ring
                    self.play(self.game_state.current_player(), None).await;
                }
            }
        } else {
            if *last_play + BOT_TIMER <= now {
                self.play(self.current_player(), None).await;
            }
        }

        // TODO: play default action

        last_play + action_timer
    }

    async fn play(&mut self, seat: Seat, play: Option<Play>) -> ControlFlow<()> {
        let GameInfo::Active { start_timestamp, sinks, last_play, game_state, action_timer, weak, .. } = self 
            else { return ControlFlow::Break(()) };

        let weak = Weak::clone(weak);

        if self.current_player() != seat { return ControlFlow::Continue(()) }

        let play = play.unwrap_or_else(|| choose_play(game_state));
        
        match game_state.play(play) {
            Ok(()) => {},
            Err(()) => {
                // TODO: invalid play
            },
        }

        // TODO: announce
        
        *last_play = Instant::now();

        if self.is_human(self.current_player()) {
            if let Some(action_timer) = *action_timer {
                task::spawn(async move {
                    sleep(action_timer).await;
                    if let Some(game) = weak.upgrade() {
                        let game = game.lock().unwrap();
                        game.alarm().await;
                    })
                });
            }

        } else {
            task::spawn(async move {
                sleep(BOT_TIMER).await;
                if let Some(game) = weak.upgrade() {
                    let game = game.lock().unwrap();
                    game.alarm().await;
                })
            });
        }

    }

    async fn valid_plays(&mut self, seat: Seat, cards: Cards) -> ControlFlow<()> {

    }
}

