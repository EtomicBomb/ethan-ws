//#![allow(dead_code)]
//#![allow(unused_imports)]

use axum::{
    routing::{get, post, put}, 
    extract::{Query, FromRequestParts},
    Router,
    Json,
    RequestPartsExt, 
    async_trait,
    debug_handler,
    response::{
        sse::{self, KeepAlive, Sse}, 
        IntoResponse
    },
};
//use axum_server::tls_rustls::RustlsConfig;
use std::{
    net::SocketAddr,
    collections::{HashMap},
    time::{Duration},
    sync::{Arc},
    convert::Infallible,
    sync::{Weak},
};
use tower_http::{
    services::{ServeDir, ServeFile},
    set_header::SetResponseHeaderLayer,
};
use http::{
    header::{self, HeaderValue},
    status::StatusCode,
    request::Parts,
    HeaderMap,
};
use tokio::{
    time::{Instant, sleep_until},
    sync::{Mutex},
    task,
    sync::mpsc::{self, UnboundedSender},
};
use tokio_stream::{wrappers::UnboundedReceiverStream};
use futures::future::BoxFuture;
use serde::{Serialize, Deserialize};
use rand::{thread_rng, Rng, seq::SliceRandom};
use uuid::Uuid;


mod game;
use crate::game::{GameState, choose_play, Seat, Cards, Play};

#[tokio::main]
async fn main() {
//    let config = RustlsConfig::from_pem_file( // TODO: revert on deploy
//        "secret/cert.pem",
//        "secret/key.pem",
//    )
//    .await
//    .unwrap();

    let app = app();

    let addr = SocketAddr::from(([127, 0, 0, 1], 8000));
    eprintln!("listening on {}", addr);

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
    cards: Cards,
}

async fn play(
    ExtractGame { game, seat, .. }: ExtractGame,
    Json(PlayRequest { cards }): Json<PlayRequest>,
) -> Result<impl IntoResponse, ()> {
    let mut game = game.lock().await;
    game.human_play(seat, cards).await
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
        let cards = choose_play(game_state).cards;
        self.play(current_player, cards).await
    }

    async fn human_play(&mut self, seat: Seat, cards: Cards) -> Result<(), ()> {
        let Era::Active { game_state, .. } = &mut self.era else { return Err(()) };
        let current_player = game_state.current_player();
        if seat != current_player { return Err(()) }
        self.play(seat, cards).await
    }

    async fn play(&mut self, seat: Seat, cards: Cards) -> Result<(), ()> {
        let Era::Active { game_state, .. } = &mut self.era else { return Err(()) };

        let play = game_state.play(cards).map_err(|_| ())?;
        let pass = play.is_pass();
        let load = game_state.hand(seat).len();
        for other in Seat::ALL {
            let _ = self.respond(other, &Respond::Play { seat, load, pass, cards }).await;
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

        let control = game_state.has_control();
        for other in Seat::ALL {
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
            .ok_or((StatusCode::FORBIDDEN, "could not access game with credentials"))?;
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
        pass: bool,
        cards: Cards,
    },
    Turn {
        seat: Seat,
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
