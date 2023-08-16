#![allow(dead_code)]
#![feature(async_fn_in_trait)]
#![allow(unused_imports)]

use axum::{
    debug_handler,
    extract::{FromRequestParts, Query, State},
    response::{
        sse::{self, KeepAlive, Sse},
        IntoResponse,
    },
    routing::{get, post, put},
    Json, RequestPartsExt, Router,
};
//use axum_server::tls_rustls::RustlsConfig;
use futures::future::{BoxFuture, LocalBoxFuture};
use http::{
    header::{self, HeaderValue},
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

mod game;
use crate::game::{choose_play, Cards, GameState, Seat, PlayError};

const MAX_ACTION_TIMER: Duration = Duration::from_secs(1000);
const BOT_ACTION_TIMER: Duration = Duration::from_secs(1);

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
        .layer(SetResponseHeaderLayer::overriding(
            // TODO: revert debug
            header::CACHE_CONTROL,
            HeaderValue::from_static("no-store, must-revalidate"),
        ))
}

fn api() -> Router {
    Router::new()
        .route("/test", get(|| async { "test endpoint!" }))
        .route("/lobby/join", post(lobby_join))
        .route("/lobby/subscribe", get(lobby_subscribe))
        .route("/lobby/username", put(lobby_username))
        .route("/lobby/timer", put(lobby_timer))
        .route("/lobby/start", post(lobby_start))
        .route("/active/play", post(active_play))
        .route("/active/playable", post(active_play))
        .with_state(ApiState::new())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct JoinQuery {
    host_id: Option<UserId>,
}

#[debug_handler(state=Arc<Mutex<ApiState>>)]
async fn lobby_join(
    State(state): State<Arc<Mutex<ApiState>>>,
    Query(JoinQuery { host_id }): Query<JoinQuery>,
) -> Result<impl IntoResponse, ()> {
    let auth = state.lock().await.lobby_join(host_id).await?;
    Ok(Json(auth))
}

#[debug_handler(state=Arc<Mutex<ApiState>>)]
async fn lobby_subscribe(
    State(state): State<Arc<Mutex<ApiState>>>,
    Query(auth): Query<Auth>,
) -> Result<impl IntoResponse, ()> {

    let (tx, rx) = mpsc::unbounded_channel();
    let tx_closed = tx.clone();

    let (phase, seat) = state.lock().await.get_lobby(auth).await?;
    phase.lock().await.subscribe(seat, tx).await;

    task::spawn(async move {
        tx_closed.closed().await;
        let mut state = state.lock().await;
        state.disconnect(auth.user_id).await;
    });

    let rx = UnboundedReceiverStream::new(rx);
    let sse = Sse::new(rx).keep_alive(KeepAlive::default());
    Ok(sse)
}

#[derive(Deserialize)]
struct UsernameRequest {
    username: String,
}

#[debug_handler(state=Arc<Mutex<ApiState>>)]
async fn lobby_username(
    State(state): State<Arc<Mutex<ApiState>>>,
    Query(auth): Query<Auth>,
    Json(UsernameRequest { username }): Json<UsernameRequest>,
) -> Result<impl IntoResponse, ()> {
    let (phase, seat) = state.lock().await.get_lobby(auth).await?;
    phase.lock().await.username(seat, username).await;
    Ok(())
}

#[derive(Deserialize)]
#[serde_as]
struct ActionTimerRequest {
    #[serde_as(as = "Option<DurationMilliSeconds<u64>>")]
    millis: Option<Duration>,
}

#[debug_handler(state=Arc<Mutex<ApiState>>)]
async fn lobby_timer(
    State(state): State<Arc<Mutex<ApiState>>>,
    Query(auth): Query<Auth>,
    Json(ActionTimerRequest { millis }): Json<ActionTimerRequest>,
) -> Result<impl IntoResponse, ()> {
    let (phase, seat) = state.lock().await.get_lobby(auth).await?;
    phase.lock().await.timer(seat, millis).await?;
    Ok(())
}

async fn lobby_start(
    State(state): State<Arc<Mutex<ApiState>>>,
    Query(auth): Query<Auth>,
) -> Result<impl IntoResponse, ()> {
    let (phase, seat) = state.lock().await.get_lobby(auth).await?;
    let mut phase = phase.lock().await;
    phase.start(seat).await
}

#[derive(Deserialize)]
struct PlayRequest {
    cards: Cards,
}

#[derive(Serialize)]
struct PlayableResponse {
    playable: Option<PlayError>,
}

async fn active_playable(
    State(state): State<Arc<Mutex<ApiState>>>,
    Query(auth): Query<Auth>,
    Json(PlayRequest { cards }): Json<PlayRequest>,
) -> Result<impl IntoResponse, ()> {
    let (phase, seat) = state.lock().await.get_active(auth).await?;
    let phase = phase.lock().await;
    let playable = phase.playable(seat, cards).await?;
    Ok(Json(PlayableResponse { playable }))
}

async fn active_play(
    State(state): State<Arc<Mutex<ApiState>>>,
    Query(auth): Query<Auth>,
    Json(PlayRequest { cards }): Json<PlayRequest>,
) -> Result<impl IntoResponse, ()> {
    let (phase, seat) = state.lock().await.get_active(auth).await?;
    let mut phase = phase.lock().await;
    phase.human_play(seat, cards).await
}

#[derive(Default)]
struct ApiState {
    lobbies: HashMap<UserId, Arc<Mutex<Lobby>>>,
    actives: HashMap<UserId, Arc<Mutex<Active>>>,
    wins: HashMap<UserId, Arc<Mutex<Win>>>,
    this: Weak<Mutex<ApiState>>,
}

trait Phase {
    async fn get_seat(&self, auth: Auth) -> Option<Seat>;
    async fn join(&mut self) -> Result<Auth, ()>;
    async fn subscribe(&mut self, seat: Seat, tx: UserTx);
    async fn timer(&mut self, seat: Seat, timer: Option<Duration>) -> Result<(), ()>;
    async fn username(&mut self, seat: Seat, username: String);
    async fn start(&mut self, seat: Seat) -> Result<(), ()>;
    async fn playable(&self, seat: Seat, cards: Cards) -> Result<Option<PlayError>, ()>;
    async fn play(&mut self, seat: Seat, cards: Cards) -> Result<(), ()>;
    async fn disconnect(&mut self, user_id: UserId) -> Result<(), ()>;
}

trait PhaseDyn {
    fn get_seat(&self, auth: Auth) -> LocalBoxFuture<'_, Option<Seat>>;
    fn join(&mut self) -> LocalBoxFuture<'_, Result<Auth, ()>>;
    fn subscribe(&mut self, seat: Seat, tx: UserTx) -> LocalBoxFuture<'_, ()>;
    fn timer(&mut self, seat: Seat, timer: Option<Duration>) -> LocalBoxFuture<'_, Result<(), ()>>;
    fn username(&mut self, seat: Seat, username: String) -> LocalBoxFuture<'_, ()>;
    fn start(&mut self, seat: Seat) -> LocalBoxFuture<'_, Result<(), ()>>;
    fn playable(&self, seat: Seat, cards: Cards) -> LocalBoxFuture<'_, Result<Option<PlayError>, ()>>;
    fn play(&mut self, seat: Seat, cards: Cards) -> LocalBoxFuture<'_, Result<(), ()>>;
    fn disconnect(&mut self, user_id: UserId) -> LocalBoxFuture<'_, Result<(), ()>>;
}

impl<T: Phase> PhaseDyn for T {
    fn get_seat(&self, auth: Auth) -> LocalBoxFuture<'_, Option<Seat>> {
        Box::pin(<Self as Phase>::get_seat(self, auth))
	}
    fn join(&mut self) -> LocalBoxFuture<'_, Result<Auth, ()>> {
        Box::pin(<Self as Phase>::join(self))
	}
    fn subscribe(&mut self, seat: Seat, tx: UserTx) -> LocalBoxFuture<'_, ()> {
        Box::pin(<Self as Phase>::subscribe(self, seat, tx))
	}
    fn timer(&mut self, seat: Seat, timer: Option<Duration>) -> LocalBoxFuture<'_, Result<(), ()>> {
        Box::pin(<Self as Phase>::timer(self, seat, timer))
	}
    fn username(&mut self, seat: Seat, username: String) -> LocalBoxFuture<'_, ()> {
        Box::pin(<Self as Phase>::username(self, seat, username))
	}
    fn start(&mut self, seat: Seat) -> LocalBoxFuture<'_, Result<(), ()>> {
        Box::pin(<Self as Phase>::start(self, seat))
	}
    fn playable(&self, seat: Seat, cards: Cards) -> LocalBoxFuture<'_, Result<Option<PlayError>, ()>> {
        Box::pin(<Self as Phase>::playable(self, seat, cards))
	}
    fn play(&mut self, seat: Seat, cards: Cards) -> LocalBoxFuture<'_, Result<(), ()>> {
        Box::pin(<Self as Phase>::play(self, seat, cards))
	}
    fn disconnect(&mut self, user_id: UserId) -> LocalBoxFuture<'_, Result<(), ()>> {
        Box::pin(<Self as Phase>::disconnect(self, user_id))
	}
}

fn assert() {
    let a: Arc<Mutex<dyn PhaseDyn>> = todo!();
}

// TODO: consider Box<dyn Phase>
enum Session {
    Lobby(Arc<Mutex<Lobby>>),
    Active(Arc<Mutex<Active>>),
    Win(Arc<Mutex<Win>>),
}

impl Session {
    async fn get_lobby(&self) -> Option<Arc<Mutex<Lobby>>> {
        match self {
            Session::Lobby(phase) => Some(Arc::clone(phase)),
            _ => None,
        }
    }

    async fn get_active(&self) -> Option<Arc<Mutex<Active>>> {
        match self {
            Session::Active(phase) => Some(Arc::clone(phase)),
            _ => None,
        }
    }

    async fn disconnect(&mut self, user_id: UserId) {
        match self {
            Session::Lobby(phase) => {
                let mut phase = phase.lock().await;
                let _ = phase.disconnect(user_id).await;
            },
            Session::Active(phase) => {
                let mut phase = phase.lock().await;
                let _ = phase.disconnect(user_id).await;
            },
            Session::Win(phase) => {
                let mut phase = phase.lock().await;
                let _ = phase.disconnect(user_id).await;
            },
        }
    }
}

impl ApiState {
    fn new() -> Arc<Mutex<Self>> {
        Arc::new_cyclic(|this| {
            Mutex::new(Self {
                lobbies: HashMap::default(),
                actives: HashMap::default(),
                wins: HashMap::default(),
                this: Weak::clone(this),
            })
        })
    }

    async fn lobby_join(&mut self, host_id: Option<UserId>) -> Result<Auth, ()> {
        let phase = match host_id {
            Some(host_id) => Arc::clone(self.lobbies.get(&host_id).ok_or(())?),
            None => Arc::new(Mutex::new(Lobby::new(Weak::clone(&self.this)))),
        };
        let auth = phase.lock().await.join().await?;
        self.lobbies.insert(auth.user_id, phase);
        Ok(auth)
    }

    async fn get_lobby(&self, auth: Auth) -> Result<(Arc<Mutex<Lobby>>, Seat), ()> {
        let phase = self.lobbies.get(&auth.user_id).ok_or(())?;
        let seat = phase.lock().await.get_seat(auth).ok_or(())?;
        Ok((Arc::clone(phase), seat))
    }

    async fn get_active(&self, auth: Auth) -> Result<(Arc<Mutex<Active>>, Seat), ()> {
        let phase = self.actives.get(&auth.user_id).ok_or(())?;
        let seat = phase.lock().await.get_seat(auth).ok_or(())?;
        Ok((Arc::clone(phase), seat))
    }

    async fn disconnect(&mut self, user_id: UserId) {
        // there should be only one thingn in these maps
        if let Some(phase) = self.lobbies.remove(&user_id) {
            let mut phase = phase.lock().await;
            let _ = phase.disconnect(user_id).await;
        }
        if let Some(phase) = self.actives.remove(&user_id) {
            let mut phase = phase.lock().await;
            let _ = phase.disconnect(user_id).await;
        }
        if let Some(phase) = self.wins.remove(&user_id) {
            let mut phase = phase.lock().await;
            let _ = phase.disconnect(user_id).await;
        }
    }

    fn transition_start(&mut self, user_ids: Vec<UserId>, phase: Arc<Mutex<Active>>) {
        for user_id in user_ids.iter() {
            self.lobbies.remove(user_id);
        }
        for &user_id in user_ids.iter() {
            self.actives.insert(user_id, Arc::clone(&phase));
        }
    }

    fn transition_win(&mut self, user_ids: Vec<UserId>, phase: Arc<Mutex<Win>>) {
        for user_id in user_ids.iter() {
            self.actives.remove(user_id);
        }
        for &user_id in user_ids.iter() {
            self.wins.insert(user_id, Arc::clone(&phase));
        }
    }
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
    fn new(api_state: Weak<Mutex<ApiState>>) -> Self {
        let mut seats = Vec::from(Seat::ALL);
        seats.shuffle(&mut thread_rng());
        let timer = None;
        let seats_left = 4;
        let host = None;
        let common = Common::new(api_state);
        let usernames = HashMap::default();
        Self { timer, usernames, seats, seats_left, host, common }
    }

    fn get_seat(&self, auth: Auth) -> Option<Seat> {
        self.common.get_seat(auth)
    }

    fn is_host(&self, seat: Seat) -> bool {
        self.host == Some(seat)
    }

    async fn join(&mut self) -> Result<Auth, ()> {
        self.seats_left = self.seats_left.checked_sub(1).ok_or(())?;
        let seat = self.seats[self.seats_left];

        let auth = self.common.new_user(seat);

        self.host.get_or_insert(seat);

        // TODO: ensure that user has subscribed within a couple seconds
        for other in Seat::ALL {
            let _ = self.common.respond(other, &Respond::Connected { seat }).await;
        }
        Ok(auth)
    }

    async fn subscribe(&mut self, seat: Seat, tx: UserTx) {
        self.common.subscribe(seat, tx);
        let _ = self.common.respond(seat, &Respond::Welcome { seat }).await;
        if self.is_host(seat) {
            let _ = self.common.respond(seat, &Respond::Host { }).await;
        }
        let humans = Vec::from_iter(self.common.human_seats());
        for &other in humans.iter() {
            let _ = self
                .common
                .respond(seat, &Respond::Connected { seat: other })
                .await;
        }
        for other in Seat::ALL {
            let Some(username) = self.usernames.get(&other) else { continue };
            let _ = self
                .common
                .respond(
                    seat,
                    &Respond::Username {
                        seat: other,
                        username: username.clone(),
                    },
                )
                .await;
        }

        // TODO: complete new subscription package with era details
    }

    async fn username(&mut self, seat: Seat, username: String) {
        self.usernames.insert(seat, username.clone());
        for other in Seat::ALL {
            if seat == other {
                continue;
            }
            let username = username.clone();
            let _ = self
                .common
                .respond(other, &Respond::Username { seat, username })
                .await;
        }
    }

    async fn timer(&mut self, seat: Seat, timer: Option<Duration>) -> Result<(), ()> {
        if !self.is_host(seat) {
            return Err(());
        }
        let too_long = timer.is_some_and(|timer| timer > MAX_ACTION_TIMER);
        self.timer = if too_long {
            None
        } else {
            timer
        };
        Ok(())
    }

    async fn disconnect(&mut self, user_id: UserId) -> Result<(), ()> {
        let seat = self.common.disconnect(user_id)?;
        for other in Seat::ALL {
            let _ = self.common.respond(other, &Respond::Disconnected { seat }).await;
        }

        self.seats_left += 1;
        if self.is_host(seat) {
            self.host = self.seats[self.seats_left..].last().cloned();
            if let Some(new_host) = self.host {
                let _ = self.common.respond(new_host, &Respond::Host { }).await;
            }
        }

        
        Ok(())
    }

    async fn start(&mut self, seat: Seat) -> Result<(), ()> {
        if !self.is_host(seat) {
            return Err(());
        }
        let timer = self.timer;
        let api_state = self.common.api_state();
        let user_ids = Vec::from_iter(self.common.user_ids());
        let common = mem::take(&mut self.common);
        let active = Active::new(timer, common).await;
        api_state.lock().await.transition_start(user_ids, active);
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
            let _ = self.common.respond(other, &Respond::Deal { cards }).await;
        }
        self.solicit().await;
    }

    fn get_seat(&self, auth: Auth) -> Option<Seat> {
        self.common.get_seat(auth)
    }

    async fn playable(&self, seat: Seat, cards: Cards) -> Result<Option<PlayError>, ()> {
        let current_player = self.game_state.current_player();
        if seat != current_player {
            return Err(());
        }
        Ok(self.game_state.playable(cards).err())
    }

    async fn auto_play(&mut self) {
        let current_player = self.game_state.current_player();
        let cards = choose_play(&self.game_state).cards;
        self.play(current_player, cards).await.expect("our bots should always choose valid plays");
    }

    async fn human_play(&mut self, seat: Seat, cards: Cards) -> Result<(), ()> {
        let current_player = self.game_state.current_player();
        if seat != current_player {
            return Err(());
        }
        self.play(seat, cards).await
    }

    async fn play(&mut self, seat: Seat, cards: Cards) -> Result<(), ()> {
        let play = self.game_state.play(cards).map_err(|_| ())?;
        let pass = play.is_pass();
        let load = self.game_state.hand(seat).len();
        let win = load == 0;
        for other in Seat::ALL {
            let _ = self
                .common
                .respond(
                    other,
                    &Respond::Play {
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
            let _ = self
                .common
                .respond(
                    other,
                    &Respond::Turn {
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
    fn force_play(this: Weak<Mutex<Self>>) -> BoxFuture<'static, Result<(), ()>> {
        Box::pin(async move {
            let deadline = this.upgrade().ok_or(())?.lock().await.deadline.ok_or(())?;
            sleep_until(deadline).await;

            let this = this.upgrade().ok_or(())?;
            let mut this = this.lock().await;
            if Instant::now() < this.deadline.ok_or(())? {
                return Err(());
            }
            // TODO: can choose a worse bot here if we're forcing a human player
            this.auto_play().await;
            Ok(())
        })
    }

    async fn disconnect(&mut self, user_id: UserId) -> Result<(), ()> {
        let seat = self.common.disconnect(user_id)?;
        for other in Seat::ALL {
            let _ = self.common.respond(other, &Respond::Disconnected { seat }).await;
        }
        // TODO: what if current player whose turn it is disconnects
        Ok(())
    }

    async fn win(&mut self) {
        let api_state = self.common.api_state();
        let user_ids = Vec::from_iter(self.common.user_ids());
        let common = mem::take(&mut self.common);
        let phase = Win::new(common);
        api_state.lock().await.transition_win(user_ids, phase);
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

    fn get_seat(&self, auth: Auth) -> Option<Seat> {
        self.common.get_seat(auth)
    }

    async fn disconnect(&mut self, user_id: UserId) -> Result<(), ()> {
        let seat = self.common.disconnect(user_id)?;
        for other in Seat::ALL {
            let _ = self.common.respond(other, &Respond::Disconnected { seat }).await;
        }
        Ok(())
    }
}

#[derive(Default)]
struct Common {
    users: HashMap<UserId, Seat>,
    secrets: HashMap<Seat, UserSecret>,
    txs: HashMap<Seat, UserTx>,
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

    fn new_user(&mut self, seat: Seat) -> Auth {
        let user_id = UserId::random();
        let user_secret = UserSecret::random();
        self.users.insert(user_id, seat);
        self.secrets.insert(seat, user_secret);
        Auth { user_id, user_secret }
    }

    fn subscribe(&mut self, seat: Seat, tx: UserTx) {
        self.txs.insert(seat, tx);
    }

    fn get_seat(&self, auth: Auth) -> Option<Seat> {
        let seat = *self.users.get(&auth.user_id)?;
        if self.secrets[&seat] != auth.user_secret {
            return None;
        }
        Some(seat)
    }

    fn disconnect(&mut self, user_id: UserId) -> Result<Seat, ()> {
        let seat = self.users.remove(&user_id).ok_or(())?;
        self.txs.remove(&seat);
        self.secrets.remove(&seat);
        Ok(seat)
    }

    async fn respond(&mut self, seat: Seat, response: &Respond) -> Result<(), ()> {
        let tx = self.txs.get(&seat).ok_or(())?;
        let response = serde_json::to_value(response).unwrap();
        let response = sse::Event::default()
            .json_data(&response["data"])
            .unwrap()
            .event(response["event"].as_str().unwrap());
        let response = Ok::<_, Infallible>(response);
        tx.send(response).map_err(|_| ())?;
        Ok(())
    }
}

type UserTx = UnboundedSender<Result<sse::Event, Infallible>>;

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
