use {
    crate::{
        html::{HtmlBuf},
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
        Form, RequestPartsExt, Router, 
    },
    axum_core::response::{IntoResponseParts, ResponseParts},
    axum_extra::{
        TypedHeader,
        extract::CookieJar,
    },
    cookie::{Cookie, Expiration, SameSite},
    futures::future::BoxFuture,
    http::{request::Parts, status::StatusCode},
    once_cell::{sync::Lazy};
    rand::{seq::SliceRandom, thread_rng, Rng, distributions::weighted::WeightedIndex},
    serde::{Deserialize, Serialize},
    serde_with::{serde_as, DurationMilliSeconds},
    std::{
        collections::HashMap,
        convert::Infallible,
        fmt::{self},
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
        time::{sleep_until, Instant},
    },
    tokio_stream::{wrappers::UnboundedReceiverStream, StreamExt},
    time::{OffsetDateTime},
    uuid::Uuid,
};

const BOT_ACTION_TIMER: Duration = Duration::from_secs(1);
const INACTIVE_BEFORE_DISCONNECT: Duration = Duration::from_secs(60);

pub fn api<S>() -> Router<S> {
    Router::new()
        .route("/connect", post(connect))
        .route("/wait-lobby", get(wait_lobby))
        .route("/start", post(start))
        .route("/score", get(score))
        .route("/spell", put(spell))
        .route("/clear", post(clear))
        .with_state(ApiState::new())
}

#[debug_handler(state=Arc<Mutex<ApiState>>)]
async fn connect(
    State(state): State<Arc<Mutex<ApiState>>>,
    user_session: Option<UserSession<Authenticated>>,
    TypedHeader(hx::request::CurrentUrl(mut url)): TypedHeader<hx::request::CurrentUrl>,
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
        (_, Some(url_session_id)) => match state.add_player_existing_session(url_session_id).await {
            Ok(auth) => (auth, None),
            Err(reconnect_reason) => {
                let auth = state.add_player_new_session().await?;
                (auth, Some(reconnect_reason))
            }
        },
        (_, None) => {
            let auth = state.add_player_new_session().await?;
            (auth, None)
        }
    };

    url.query_pairs_mut()
        .clear()
        .append_pair("session_id", &format!("{}", auth.session_id));

    let h = HtmlBuf::default();
    match phase {
        Phase::NeedsOpponent => {
            let h = h
                .node("main", |h| h
                    .class("wait-lobby")
                    .hx_ext("sse")
                    .sse_connect("api/wait-lobby")
                    .hx_trigger("sse:message, every 10s, session-cleared from:body")
                    .hx_post("api/connect")
                    .hx_swap("outerHTML")
                    .node("h1", |h| h
                        .text("Word search game")
                    )
                    .node("p", |h| h
                        .text("Share the link in the address bar with your opponent")
                    )
                    .node("p", |h| h
                        .text("Waiting for your them to click the link.")
                    )
                    .node("button", |h| h
                        .hx_post("api/clear")
                        .text("Forget about this game and make a new one.")
                    )
                    .node("button", |h| h
                        .text("Play singleplayer (coming soon)")
                    )
                );
        Phase::Start => {
            let h = h
                .node("main", |h| h
                    .class("page")
                    .hx_trigger("start-game from:body")
                    .hx_post("api/connect")
                    .hx_swap("outerHTML")
                    .node("p", |h| h
                        .text("Found your opponent!")
                    )
                    .node("button", |h| h
                        .hx_post("api/start")
                        .hx_swap("none")
                        .text("Play now")
                    )
                );
        }
        Phase::Main { turn_remaining, board } => {
            let duration = turn_remaining.as_millis();
            let h = h
                .node("main", |h| h
                    .hx_trigger(format!("load delay:{}ms(duration)")
                    .hx_post("api/connect")
                    .hx_swap("outerHTML")
                    .hx_on("htmx:load", "setupBoard(this)")
                    .node("section", |h| h
                        .class("banner")
                        .node("div", |h| h
                            .class("spelled")
                        )
                        .node("div", |h| h
                            .class("clock")
                            .node("div", |h| h
                                .style(format!("animation-duration: {}ms;", duration))
                            )
                        )
                        .node("div", |h| h
                            .class("scoreboard")
                            .hx_trigger("load, score-refresh from:body")
                            .hx_get("api/score")
                            .hx_swap("innerHTML")
                        )
                    )
                    .chain(|h| board.render(h))
                );
        }
        Phase::Post { you_spelled, others_spelled } => {
            let render_spelled = |spelled, html| {
                spelled.iter().fold(h, |h, word| {
                    render_word_score(word, h)
                })
            };
            let h = h
                .node("main", |h| h
                    .hx_trigger("session-cleared from:body")
                    .hx_post("api/connect")
                    .hx_swap("outerHTML")
                    .node("h1", |h| h
                        .text("Thank you for playing")
                    )
                    .node("section", |h| h
                        .class("you spelled")
                        .chain(|h| render_spelled(you_spelled, h))
                    )
                    .chain(|h| {
                        others_spelled.iter().fold(h, |h, others| h
                            .node("section", |h| h
                                .class("others spelled")
                                .chain(|h| render_spelled(others, h))
                            )
                        )
                    })
                    .node("button", |h| h
                        .hx_post("api/clear")
                        .text("Forget about this session and make a new one.")
                    )
                    .node("button", |h| h
                        .text("Play another round")
                    )
                );
        }
    }

    Ok((*auth, TypedHeader(hx::response::ReplaceUrl(url)), html))
}

fn render_word_score(word: &str, html: HtmlBuf) -> HtmlBuf {
    let points = score_word(word);
    html
        .node("p", |h| h
            .class("word-score")
            .node("span", |h| h
                .class("word")
                .text(word)
            )
            .node("span", |h| h
                .class("score")
                .text(format!("{}", points))
            )
        )
}

type Update = ();

#[debug_handler(state=Arc<Mutex<ApiState>>)]
async fn wait_lobby(mut user_session: UserSession<Authenticated>) -> Result<impl IntoResponse> {
    let (tx, rx) = mpsc::unbounded_channel();
    user_session
        .session
        .wait_lobby(user_session.auth, tx)
        .await?;
    let rx = UnboundedReceiverStream::new(rx)
        .map(|data| format!("{}", data))
        .map(|data| sse::Event::default().data(data).event("message"))
        .map(Ok::<_, Infallible>);
    Ok(Sse::new(rx).keep_alive(KeepAlive::default()))
}

#[debug_handler(state=Arc<Mutex<ApiState>>)]
async fn start(mut user_session: UserSession<Authenticated>) -> Result<impl IntoResponse> {
    user_session.session.start(user_session.auth).await?;
    let start_game = hx::response::Trigger(HeaderValue::from_static("start-game"));
    Ok((start_game, StatusCode::OK))
}

#[debug_handler(state=Arc<Mutex<ApiState>>)]
async fn spell(
    mut user_session: UserSession<Authenticated>,
    Form(word): Form<Vec<BoardPosition, String>>,
) -> Result<impl IntoResponse> {
    let mut word: Vec<(u32, BoardPosition)> = word
        .into_iter()
        .filter_map(|(board_position, order)| Some((order.parse::<u32>().ok()?, board_position)));
    word.sort_by_key(|(order, board_position)| order);
    let word = word.into_iter().map(|(order, board_position)| board_position).collect::<Vec<_>>();
    let word = user_session
        .session
        .spell(user_session.auth, word)
        .await?;
    let html = HtmlBuf::default();
    let (status_code, html) = match word {
        Some(word) => {
            let html = html
                .node("p", |h| h
                    .class("spelled")
                    .node("span", |h| h
                        .class("word")
                        .text(&word)
                    )
                    .node("span", |h| h
                        .class("points")
                        .text(format!("+{}", score_word(&word)))
                    )
                );
            (StatusCode::CREATED, html)
        }
        None => {
            (StatusCode::NO_CONTENT, html)
        }
    };
    let score_refresh = hx::response::Trigger(HeaderValue::from_static("score-refresh"));
    Ok((score_refresh, status_code, html))
}

#[debug_handler(state=Arc<Mutex<ApiState>>)]
async fn score(
    mut user_session: UserSession<Authenticated>,
) -> Result<impl IntoResponse> {
    let score = user_session
        .session
        .score(user_session.auth)
        .await?;
    let html = HtmlBuf::default()
        .node("span", |h| h
            .text(format!("{}", score))
        );
    Ok((html))
}

#[debug_handler(state=Arc<Mutex<ApiState>>)]
async fn clear(
    mut user_session: UserSession<Authenticated>,
) -> Result<impl IntoResponse> {
    let cookie = Cookie::build(("auth", ""))
        .secure(true)
        .http_only(true)
        .max_age(time::Duration::ZERO)
        .same_site(SameSite::Strict)
        .build();
    let session_cleared = hx::response::Trigger(HeaderValue::from_static("session-cleared"));
    Ok((session_cleared, cookie))
}
struct ApiState {
    sessions: HashMap<SessionId, Arc<Mutex<Session>>>,
    expiry: VecDeque<SessionId>,
}

impl ApiState {
    fn new() -> Self {
        Self {
            sessions: HashMap::default(),
            expiry: VecDeque::default(),
        }
    }

    async fn add_player_new_session(&mut self) -> Result<Authenticated> {
        if self.expiry.len() > 1000 {
            let removed = self.expiry_queue.pop_front().unwrap();
            self.sessions.remove(&removed);
        }
        let session_id = SessionId::generate(&mut thread_rng());
        let session = Session::new(session_id);
        self.sessions.insert(session_id, Arc::clone(&session));
        self.expiry.push_back(session_id);
        let auth = session.lock().await.add_player(session_id).await?;
        Ok(auth)
    }

    async fn add_player_existing_session(&mut self, session_id: SessionId) -> Result<Authenticated> {
        let session = self.get_session(session_id).await?;
        let auth = session.lock().await.add_player(session_id).await?;
        Ok(auth)
    }

    async fn get_session(&self, session_id: SessionId) -> Result<Arc<Mutex<Session>>> {
        let session = self.sessions.get(&session_id).ok_or(Error::NoSession)?;
        Ok(Arc::clone(session))
    }
}

fn score_word(word: &str) -> usize {
    let points = word.chars().count() as f64 - 2.0;
    let points = points * points * 0.9;
    let points = 100.0 * points.round();
    points as usize
}

#[derive(Clone, Copy, Hash, Eq, PartialEq, PartialOrd, Ord, Debug, Serialize, Deserialize)]
enum Phase<'a> {
    NeedsOpponent,
    Start,
    Main {
        turn_remaining: Duration,
        board: &'a Board,
    },
    Post {
        you_spelled: &'a HashSet<String>,
        others_spelled: Vec<&'a HashSet<String>>,
    },
}

#[derive(Debug)]
struct Player {
    tx: Option<UnboundedSender<Update>>,
    spelled: HashSet<String>,
    turn_expires: Option<Instant>,
}

#[derive(Debug)]
struct Session {
    players: HashMap<UserSecret, Player>,
    board: Board,
    session_id: SessionId,
}

impl Session {
    pub fn new(session_id: SessionId) -> Self
        Self {
            players: HashMap::default(),
            phase: Phase::Lobby,
            session_id,
        }
    }

    pub async fn phase(auth: Authenticated) -> Result<Phase<'_>> {
        if self.players.len() < 2 {
            return Ok(Phase::NeedsOpponent);
        }
        let Some(turn_expires) = self.players[&auth.user_secret].turn_expires else {
            return Ok(Phase::Start);
        };
        let turn_remaining = turn_expires.checked_duration_since(Instant::now()) else {
            let you_spelled = &self.players[&auth.user_secret].spelled;
            let others_spelled = self.players.iter()
                .filter(|(k, v)| k != auth.user_secret)
                .map(|(k, v)| &v.spelled)
                .collect::<Vec<_>>();
            return Ok(Phase::Post { you_spelled, others_spelled });
        };
        Ok(Phase::Main { turn_remaining, board: &self.board, })
    }

    pub async fn add_player(&mut self) -> Result<Authenticated> {
        if self.players.len() > 1 {
            return Err(Error::TooManyPlayers);
        }
        for player in self.players.values() {
            let Some(tx) = player.tx.as_ref() else { continue }
            tx.send(Update);
        }
        let user_secret = UserSecret::generate(&mut thread_rng());
        self.players[seat] = Some(Player {
            tx: None,
            spelled: HashSet::default(),
            turn_expires: None,
        });
        Ok(Authenticated {
            auth: Unauthenticated {
                user_secret,
                session_id,
            },
        })
    }

    pub async fn start(&mut self, auth: Authenticated) -> Result<()> {
        if self.players.len() < 2 {
            return Err(Error::NotEnoughPlayers);
        }
        if self.turn_expires.is_some() {
            return Err(Error::AlreadyStarted);
        }
        let turn_expires = Instant::now() + Duration::from_secs(60);
        self.players[&auth.user_secret].turn_expires = Some(turn_expires);
        Ok(())
    }

    pub async fn spell(&mut self, auth: Authenticated, word: &[BoardPosition]) -> Result<Option<String>> {
        let player = &self.players[&auth.user_secret];
        if player.turn_expires.ok_or(Error::NotStarted)? < Instant::now() {
            return Err(Error::NotStarted);
        }
        let word = self.board.spell(word)?;
        let already_present = self.players[&auth.user_secret].spelled.insert(word.clone()).is_some();
        if already_present {
            Ok(None)
        } else {
            Ok(Some(word))
        }
    }

    pub async fn score(&mut self, auth: Authenticated) -> Result<usize> {
        Ok(self.players[&auth.user_secret].spelled.iter().map(|word| score_word(word)).sum())
    }

    pub async fn wait_lobby(
        &mut self,
        auth: Authenticated,
        tx: UnboundedSender<Update>,
    ) -> Result<()> {
        self.players[auth.user_secret].as_mut().ok_or(Error::Absent)?.tx = Some(tx);
        Ok(())
    }

    pub fn authenticate(&mut self, auth: Unauthenticated) -> Result<Authenticated> {
        let human = self.players[auth.seat].as_ref().ok_or(Error::Absent)?;
        if human.user_secret != auth.user_secret {
            return Err(Error::BadAuthentication);
        }
        Ok(Authenticated { auth })
    }
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
struct BoardPosition(usize);

impl BoardPosition {
    fn row_col(&self, columns: usize) -> (usize, usize) {
        (self / columns, self % colunms)
    }

    fn adjacent(&self, other: &BoardPosition, columns: usize) -> bool {
        let (sr, sc) = self.row_col(columns);
        let (or, oc) = self.row_col(columns);
        self != other && cmp::max(sr.abs_diff(or), sc.abs_diff(oc)) <= 1
    }
}

struct Board {
    rows: usize,
    cols: usize,
    tiles: Vec<char>,
}

impl Board {
    fn generate<R: Rng>(mut rng: &mut R, rows: usize, cols: usize) -> Self {
        // https://en.wikipedia.org/wiki/Letter_frequency
        let weights = [8.2,1.5,2.8,4.3,12.7,2.2,2.0,6.1,7.0,0.15,0.77,4.0,2.4,6.7,7.5,1.9,0.095,6.0,6.3,9.1,2.8,0.98,2.4,0.15,2.0,0.074];
        let dist = WeightedIndex::new(&weights);
        iter::repeat_with(|| dist.sample(&mut rng))
            .map(|i| u32::try_from(i).unwrap())
            .map(|i| i + u32::from('A'))
            .map(|i| char::try_from(i))
            .take(rows * cols)
            .collect::<Vec<_>>();lines
        Self { rows, cols, tiles }
    }

    fn spell(&self, positions: &[BoardPosition]) -> Result<String> {
        static DICTIONARY: Lazy<HashSet<String>> = Lazy::new(|| {
            let dictionary = fs::read_to_string("static/word/words.txt")
                .expect("should find dictionary file");
            dictionary.lines().map(ToString::to_string).collect()
        });
        let mut seen = HashSet::default();
        for pair in positions.iter().windows(2) {
            let &[a, b]: &[BoardPosition; 2] = pair.try_into().unwrap();
            let never_seen = seen.insert(a);
            if !never_seen || !a.adjacent(b) {
                return Err(Error::BadSpelling);
            }
        }
        let word = positions
            .iter()
            .map(|&position| self.tiles.get(position.0).ok_or(Error::BadSpelling))
            .collect::<Result<String, _>>()?;
        if !DICTIONARY.contains(word) {
            return Err(Error::BadSpelling);
        }
        Ok(word)
    }

    fn render(&self, html: HtmlBuf) -> HtmlBuf {
        html
            .node("form", |h| {
                let h = h 
                    .class("tiles")
                    .hx_trigger("spell")
                    .hx_put("api/spell")
                    .hx_target("previous .spelled")
                    .hx_swap("afterbegin")
                self.tiles.iter().enumerate().fold(h, |h, (i, letter)| h
                    .node("label", |h| h
                        .node("input", |h| h
                            .r#type("hidden")
                            .name(format!("{}", i))
                        )
                        .text(letter)
                    )
                )
            )
    }
}

struct UserSession<A> {
    session: OwnedMutexGuard<Session>,
    auth: A,
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
    pub session_id: SessionId,
    pub user_secret: UserSecret,
}

impl IntoResponseParts for Unauthenticated {
    type Error = Infallible;
    fn into_response_parts(self, res: ResponseParts) -> Result<ResponseParts, Self::Error> {
        let auth = serde_urlencoded::to_string(self).unwrap();
        let cookie = Cookie::build(("auth", auth))
            .secure(true)
            .http_only(true)
            .expires(Expiration::Session)
            .same_site(SameSite::Strict)
            .build();
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
    fn generate<R: Rng>(rng: R) -> Self {
        let ret = rng.gen();
        let ret = uuid::Builder::from_random_bytes(ret).into_uuid();
        Self(ret)
    }
}

impl fmt::Display for SessionId {
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
    fn generate<R: Rng>(rng: R) -> Self {
        let ret = rng.gen();
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
    BadSpelling,
    AlreadyStarted,
    NotStarted,
    NotEnoughPlayers
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
            Self::BadSpelling => write!(f, "bad spelling"),
            Self::AlreadyStarted => write!(f, "already started"),
            Self::NotStarted => write!(f, "not started"),
            Self::NotEnoughPlayers => write!(f, "not enough players"),
            Self::TooManyPlayers => write!(f, "too many players"),
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
            Self::BadSpelling => StatusCode::BAD_REQUEST,
            Self::AlreadyStarted => StatusCode::BAD_REQUEST,
            Self::NotStarted => StatusCode::BAD_REQUEST,
            Self::NotEnoughPlayers => StatusCode::BAD_REQUEST,
            Self::TooManyPlayers => StatusCode::BAD_REQUEST,
        };
        let body = self.to_string();
        (status, body).into_response()
    }
}