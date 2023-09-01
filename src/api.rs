const INACTIVE_BEFORE_DISCONNECT: Duration = Duration::from_secs(2 * 60);

pub fn api<S>() -> Router<S> {
    Router::new()
        .route("/", get(index))
        .route("/api/join", post(index))
        .route("/api/subscribe", get(subscribe))
        .route("/api/state", get(state))
        .route("/api/keep_alive", post(keep_alive))
        .route("/api/timer", put(timer))
        .route("/api/play", post(play))
        .route("/api/playable", get(playable))
        .with_state(ApiState::new())
}

#[debug_handler(state=Arc<Mutex<ApiState>>)]
async fn index() -> Result<impl IntoResponse> {
    Ok(render_index())
}

#[debug_handler(state=Arc<Mutex<ApiState>>)]
async fn join(
    State(state): State<Arc<Mutex<ApiState>>>,
    Query(JoinQuery { session_id }): Query<JoinQuery>,
    TypedHeader(hx::CurrentUrl(url)): TypedHeader(hx::CurrentUrl),
) -> Result<impl IntoResponse> {
    let (_, session_id) = url.query_pairs().find(|(k, v)| k == "session_id").map(|(k, v)| v);
    let session_id = session_id.and_then(|session_id| SessionId::from_str(session_id).ok());
    let auth = state.lock().await.join(session_id).await?;
    let new_url = Uri::builder()
        .path_and_query(format!("/?session_id={}", session_id))
        .unwrap();
    Ok((auth, TypedHeader(hx::Push(new_url)), html))
}

#[debug_handler(state=Arc<Mutex<ApiState>>)]
async fn keep_alive(
    State(state): State<Arc<Mutex<ApiState>>>,
    unauth: Unauthenticated,
) -> Result<impl IntoResponse> {
    let phase = state.lock().await.get_phase(unauth.session_id).await?;
    let mut phase = phase.lock().await;
    let auth = phase.authenticate(unauth).await?;
    phase.keep_alive(auth).await?;
}

#[debug_handler(state=Arc<Mutex<ApiState>>)]
async fn subscribe(
    State(state): State<Arc<Mutex<ApiState>>>,
    unauth: Unauthenticated,
) -> Result<impl IntoResponse> {
    let phase = state.lock().await.get_phase(unauth.session_id).await?;
    let mut phase = phase.lock().await;
    let auth = phase.authenticate(unauth).await?;

    let (tx, rx) = mpsc::unbounded_channel();
    let rx = Sse::new(rx);

    state.lock().await.subscribe(auth, tx).await?;
    
    Ok(rx)
}

#[debug_handler(state=Arc<Mutex<ApiState>>)]
async fn state(
    State(state): State<Arc<Mutex<ApiState>>>,
    unauth: Unauthenticated,
) -> Result<impl IntoResponse> {
    let phase = state.lock().await.get_phase(unauth.session_id).await?;
    let mut phase = phase.lock().await;
    let auth = phase.authenticate(unauth).await?;

    let rendered = render_main_inner(todo!());
    
    Ok(rendered)
}

#[debug_handler(state=Arc<Mutex<ApiState>>)]
async fn timer(
    State(state): State<Arc<Mutex<ApiState>>>,
    unauth: Unauthenticated,
    RawForm(form_bytes): RawForm,
) -> Result<impl IntoResponse> {
    eprintln!("{:?}", String::from_utf8_lossy(&form_bytes));
    let phase = state.lock().await.get_phase(unauth.session_id).await?;
    let mut phase = phase.lock().await;
    let auth = phase.authenticate(unauth).await?;
    let timer = todo!();
    phase.timer(auth, timer).await
}

#[debug_handler(state=Arc<Mutex<ApiState>>)]
async fn play(
    State(state): State<Arc<Mutex<ApiState>>>,
    unauth: Unauthenticated,
    RawForm(form_bytes): RawForm,
) -> Result<impl IntoResponse> {
    eprintln!("{:?}", String::from_utf8_lossy(&form_bytes));
    let phase = state.lock().await.get_phase(unauth.session_id).await?;
    let mut phase = phase.lock().await;
    let auth = phase.authenticate(unauth).await?;
    phase.play(auth, todo!()).await
}

#[debug_handler(state=Arc<Mutex<ApiState>>)]
async fn playable(
    State(state): State<Arc<Mutex<ApiState>>>,
    unauth: Unauthenticated,
    RawForm(form_bytes): RawForm,
) -> Result<impl IntoResponse> {
    eprintln!("{:?}", String::from_utf8_lossy(&form_bytes));
    let phase = state.lock().await.get_phase(unauth.session_id).await?;
    let mut phase = phase.lock().await;
    let auth = phase.authenticate(unauth).await?;
    phase.playable(auth, todo!()).await
}

struct ApiState {
    phases: HashMap<SessionId, Arc<Mutex<Session>>>,
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
    ) -> Result<Authenticated> {
        let (session_id, phase) = match session_id {
            Some(session_id) => (session_id, self.get_phase(session_id).await?),
            None => {
                let session_id = SessionId::random();
                let phase = Lobby::new(session_id, Weak::clone(&self.this));
                self.phases.insert(session_id, Arc::clone(&phase) as _);
                (session_id, phase)
            }
        };

        let auth = phase.lock().await.join().await?;

        let this = self.this.clone();
        task::spawn(async move {
            let mut interval = interval(INACTIVE_BEFORE_DISCONNECT);
            loop {
                interval.tick().await;
                let Some(this) = this.upgrade() else { break };
                let result = this.lock().await.try_disconnect(auth).await;
                if result.is_break() { break }
            }
        });

        Ok(auth)
    }

    async fn keep_alive(&mut self, auth: Authenticated) {
        let last_active = Instant::now();
        todo!("store last active");
    }

    async fn try_disconnect(&mut self, auth: Authenticated, session_id: SessionId) -> ControlFlow<()> {
        let last_active = todo!();
        let client_active = Instant::now().elapsed(last_active) < INACTIVE_BEFORE_DISCONNECT;
        if client_active {
            return ControlFlow::Continue(());
        } 

        let Some(phase) = self.phases.get(&session_id) else { ControlFlow::Break(()) };
        let phase = Arc::clone(phase);
        let mut phase = phase.lock().await;
        let Ok(now_empty) = phase.disconnect(auth).await else { ControlFlow::Break(()) };
        if now_empty {
            self.phases.remove(&session_id);
        }
        ControlFlow::Break(())
    }

    async fn get_phase(&self, session_id: SessionId) -> Result<Arc<Mutex<dyn Phase>>> {
        let phase = self.phases.get(&session_id).ok_or(Error::NoSession)?;
        Ok(Arc::clone(phase))
    }

    fn transition(&mut self, session_id: SessionId, new_session: Session) -> Result<()> {
        let mut session = self.phases.get_mut(&session_id).ok_or(Error::NoSession)?;
        *session = new_session;
        Ok(())
    }
}

type Session = Box<dyn Phase>;

#[async_trait]
trait Phase: Send + Sync {
    async fn authenticate(&mut self, unauth: Unauthenticated) -> Result<Authenticated>;

    async fn subscribe(&mut self, auth: Authenticated, tx: Tx) -> Result<()>;

    async fn join(&mut self) -> Result<Authenticated> {
        Err(Error::BadPhase)
    }

    async fn timer(&mut self, _seat: Seat, _timer: Option<Duration>) -> Result<()> {
        Err(Error::BadPhase)
    }

    async fn start(&mut self, _auth: Authenticated) -> Result<()> {
        Err(Error::BadPhase)
    }

    async fn playable(&mut self, _auth: Authenticated, _cards: Cards) -> Result<()> {
        Err(Error::BadPhase)
    }

    async fn human_play(&mut self, _auth: Authenticated, _cards: Cards) -> Result<()> {
        Err(Error::BadPhase)
    }

    async fn disconnect(&mut self, auth: Authenticated) -> Result<bool>;
}

struct Lobby {
    timer: Option<Duration>,
    common: Common,
}

impl Lobby {
    fn new(session_id: SessionId, api_state: Weak<Mutex<ApiState>>) -> Arc<Mutex<Session>> {
        Arc::new_cyclic(|_this| {
            let timer = None;
            let common = Common::new(session_id, api_state);
            Mutex::new(Session::new(Self { timer, common }))
        })
    }
}

#[async_trait]
impl Phase for Lobby {
    async fn authenticate(&mut self, unauth: Unauthenticated) -> Result<Authenticated> {
        self.common.authenticate(unauth)
    }

    async fn subscribe(&mut self, auth: Authenticated, tx: Tx) -> Result<()> {
        self.common.subscribe(auth, tx)
    }

    async fn join(&mut self, tx: UnboundedSender<Message>) -> Result<Authenticated> {
        let unauth = self.common.new_user(tx)?;

        self.common.alert_all(&Message::Connected).await;

        Ok(unauth)
    }

    async fn timer(&mut self, auth: Authenticated, timer: Option<Duration>) -> Result<()> {
        if !self.common.is_host(auth.seat()) {
            return Err(Error::NotHost);
        }
        let too_long = timer.is_some_and(|timer| timer > MAX_ACTION_TIMER);
        self.timer = if too_long { None } else { timer };
        Ok(())
    }

    async fn start(&mut self, auth: Authenticated) -> Result<()> {
        if !self.is_host(auth.seat()) {
            return Err(Error::NotHost);
        }
        let timer = self.timer;
        self.common
            .transition(|common| async move { Active::new(timer, common).await })
            .await;
        Ok(())
    }

    async fn disconnect(&mut self, unauth: Unauthenticated) -> Result<bool> {
        let now_empty = self.common.disconnect(unauth)?;
        self.common.alert_all(&Message::Connected).await;
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
    async fn new(timer: Option<Duration>, common: Common) -> Arc<Mutex<Session>> {
        let this = Arc::new_cyclic(|this| {
            let this = Weak::clone(this);
            let game_state = GameState::default();
            let deadline = None;
            Mutex::new(Session::new(Self {
                timer,
                game_state,
                deadline,
                common,
                this,
            }))
        });
        this.lock().await.deal().await;
        this
    }

    async fn deal(&mut self) {
        self.common.alert_all(&Message::Deal).await;
        self.solicit().await;
    }

    async fn auto_play(&mut self) {
        let current_player = self.game_state.current_player();
        let cards = choose_play(&self.game_state).cards;
        self.play(current_player, cards)
            .await
            .expect("our bots should always choose valid plays");
    }

    async fn play(&mut self, auth: Authenticated, cards: Cards) -> Result<()> {
        let play = self.game_state.play(cards)?;
        let pass = play.is_pass();
        let load = self.game_state.hand(auth.seat()).len();
        let win = load == 0;
        self.common.alert_all(&Message::Play).await;

        if win {
            self.win(auth.seat()).await;
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
        self.common.alert_all(&Message::Turn).await;

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
            .transition(|common| async move { Win::new(common, winner).await })
            .await;
    }
}

#[async_trait]
impl Phase for Active {
    async fn authenticate(&mut self, unauth: Unauthenticated) -> Result<Authenticated> {
        self.common.authenticate(unauth)
    }

    async fn subscribe(&mut self, auth: Authenticated, tx: Tx) -> Result<()> {
        self.common.subscribe(auth, tx)
    }

    async fn playable(&mut self, auth: Authenticated, cards: Cards) -> Result<()> {
        let current_player = self.game_state.current_player();
        if auth.seat() != current_player {
            return Err(Error::NotCurrent);
        }
        self.game_state.playable(cards)?;
        Ok(())
    }

    async fn human_play(&mut self, auth: Authenticated, cards: Cards) -> Result<()> {
        let current_player = self.game_state.current_player();
        if auth.seat() != current_player {
            return Err(Error::NotCurrent);
        }
        self.play(auth, cards).await?;
        Ok(())
    }

    async fn disconnect(&mut self, auth: Authenticated) -> Result<bool> {
        let now_empty = self.common.disconnect(unauth)?;
        self.common.alert_all(&Message::Connected).await;
        if self.game_state.current_player() == auth.seat() {
            self.solicit().await;
        }
        Ok(now_empty)
    }
}

struct Win {
    common: Common,
}

impl Win {
    async fn new(common: Common, winner: Seat) -> Arc<Mutex<Session>> {
        let ret = Arc::new_cyclic(|_this| Mutex::new(Session::new(Self { common })));
        ret.lock().await.common.alert_all(&Message::Win).await;
        ret
    }
}

#[async_trait]
impl Phase for Win {
    async fn authenticate(&mut self, unauth: Unauthenticated) -> Result<Authenticated> {
        self.common.authenticate(unauth)
    }

    async fn subscribe(&mut self, auth: Authenticated, tx: Tx) -> Result<()> {
        self.common.subscribe(auth, tx)
    }

    async fn disconnect(&mut self, unauth: Unauthenticated) -> Result<bool> {
        let now_empty = self.common.disconnect(unauth)?;
        self.common.alert_all(&Message::Connected).await;
        Ok(now_empty)
    }
}

type Session = Arc<Mutex<Box<dyn Phase>>>;

#[derive(Default)]
struct Common {
    humans: HashMap<Seat, Human>,
    session_id: SessionId,
    session: Weak<Mutex<Box<dyn Phase>>>,
}

impl Common {
    fn new(session_id: SessionId, api_state: Weak<Mutex<ApiState>>) -> Self {
        let mut seats = Vec::from(Seat::ALL);
        seats.shuffle(&mut thread_rng());
        Self {
            humans: HashMap::default(),
            host: None,
            session_id,
            api_state,
        }
    }

    async fn subscribe(&mut self, auth: Authenticated, tx: Tx) -> Result<()> {
        self.humans.get_mut(&seat).ok_or(Error::Absent)?.tx = tx;
        Ok(())
    }

    async fn transition<F, T>(&mut self, new_phase: F)
    where
        F: FnOnce(Common) -> T,
        T: Future<Output = Session>,
    {
        let mut session = self
            .session
            .upgrade()
            .expect("method must be called when we have a mutable reference to a phase");
        let common = mem::take(self);
        let phase = new_phase(common).await;
        // What if someone's waiting on a lock for self, and acquires it
        // after this function returns and the transition is complete?
        // Since we took self, their call to authenticate will fail.
        todo!();
        api_state.lock().await.transition(session_id, phase);
    }

    fn is_host(&self, seat: Seat) -> bool {
        self.humans.get(&seat).is_some_and(|human| human.host)
    }

    fn is_human(&self, seat: Seat) -> bool {
        self.humans.contains_key(&seat)
    }

    fn authenticate(&mut self, unauth: Unauthenticated) -> Result<Authenticated> {
        let human = self.humans.get(&unauth.seat).ok_or(Error::Absent)?;
        if human.user_secret != unauth.user_secret {
            return Err(Error::BadAuthentication);
        }
        Ok(Authenticated { seat: unauth.seat, user_secret: unauth.user_secret, session_id: unauth.session_id })
    }

    fn new_user(&mut self) -> Result<Authenticated> {
        let seat = Seat::ALL.into_iter()
            .find(|seat| !self.humans.contains_key(&seat))
            .ok_or(Error::Full)?;
        let user_secret = UserSecret::random();
        self.humans.insert(seat, Human { host: self.humans.is_empty(), user_secret, last_active: Instant::now(), tx: None });
        Ok(Authenticated {
            user_secret,
            seat,
            session_id: self.session_id,
        })
    }

    fn disconnect(&mut self, auth: Authenticated) -> Result<bool> {
        let human_disconnected = self.humans.remove(&auth.seat()).ok_or(Error::Absent)?;
        if human_disconnected.host {
            let new_host = self.humans.values_mut().next();
            if let Some(new_host) {
                new_host.host = true;
            }
        }
        let now_empty = self.humans.is_empty();
        Ok(now_empty)
    }

    async fn alert(&self, seat: Seat, message: &Message) {
        let Some(tx) = self.humans.get(&seat).and_then(|human| &human.tx) else { return };
        let data = html! { <div></div> };
        let event = sse::Event::default()
            .data(data.to_string())
            .event("refresh")
            .keep_alive(KeepAlive::default());
        tx.send(Ok(event));
    }

    async fn alert_all(&mut self, message: &Message) {
        for seat in Seat::ALL {
            self.alert(seat, message).await;
        }
    }
}

struct Human {
    user_secret: UserSecret, 
    last_active: Instant,
    tx: Option<Tx>,
}

type Tx = UnboundedSender<Result<sse::Event, Infallible>>;

#[derive(Serialize)]
enum Message {
    Welcome,
    Deal,
    Host,
    Connected,
    Deal,
    Play,
    Solicit,
    Win,
}

impl Display for Message {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Message::Welcome => write!(f, "welcome"),
            Message::Host => write!(f, "host"),
            Message::Connected => write!(f, "connected"),
            Message::Deal => write!(f, "deal"),
            Message::Play => write!(f, "play"),
            Message::Solicit => write!(f, "solicit"),
            Message::Win => write!(f, "win"),
        }
    }
}

enum Relative {
    My,
    Left,
    Across,
    Right,
}

impl Relative {
    const ALL: [Relative; 4] = [Relative::My, Relative::Left, Relative::Across, Relative::Right];
    
    fn from_i8(index: i8) -> Self {
        match (index % 4 + 4) % 4  {
            Relative::My => 0,
            Relative::Left => 1,
            Relative::Across => 2,
            Relative::Right => 3,
        }
    }

    fn from_seat(my: Seat, other: Seat) -> Self {
        Relative::from_i8(other as i8 - self as i8)
    }
}

#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
struct Authenticated {
    seat: Seat,
    session_id: SessionId,
    user_secret: UserSecret,
}

impl Authenticated {
    fn seat(&self) -> Seat {
        self.seat
    }
}

#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
struct Unauthenticated {
    seat: Seat,
    session_id: SessionId,
    user_secret: UserSecret,
}

impl IntoResponseParts for Unauthenticated {
    type Error = Infallible;
    fn into_response_parts(self, mut res: ResponseParts) -> Result<ResponseParts, Self::Error> {
        let unauth = serde_json::to_string(&self).unwrap();
        let mut cookie = Cookie::new("auth", auth);
        cookie.set_secure(Some(true));
        cookie.set_http_only(Some(true));
        cookie.set_same_site(Some(SameSite::Strict));
        cookie.set_expires(Some(Expires::Session));
        let jar = CookieJar::new().add(cookie);
        Ok(jar)
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
        let unauth = jar.get("auth")
            .ok_or_else(|| (StatusCode::UNAUTHORIZED, "no unauth cookie found").into_response())?;
            .value();
        let unauth = serde_json::from_str(auth)
            .map_err(|err| (StatusCode::UNAUTHORIZED, format!("{}", err)).into_response())?;
        Ok(unauth)
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

impl FromStr for SessionId {
    type Err = uuid::Error;
    fn from_str(string: &str) -> Result<Self, Self::Err> {
        Uuid::from_str(string)
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

fn render_index() -> Node {
    html! {
        <!DOCTYPE html>
        <html lang="en-US">
        <head>
            <title>Pusoy</title>
            <meta charset="utf-8">
            <base href=".">
            <link rel="stylesheet" href="/index.css">
            <meta name="viewport" content="width=device-width, initial-scale=1" />
            <script src="/assets/htmx-1.9.5.min.js"></script>
            <script src="/assets/htmx-sse-1.9.5.js"></script>
        </head>
        <body>
            <main hx-post="/api/join" hx-trigger="load" hx-swap="outerHTML"></main>
        </body>
    }
}


fn render_main() -> Node {
    html! {
        <main hx-ext="sse" sse-connect="/api/events" hx-trigger="sse:update" hx-get="/api/state" hx-swap="innerHTML">
        </main>
    }
}

fn render_main_inner() -> Node {
    html! {
        <img class="table" alt="" />
        <section class="table">
        <div class="cards"></div>
        </section>
        { Relative::ALL
                .into_iter()
                .map(|relative| render_player(relative))
        }
    }
}

fn render_player(relative: Relative) -> Node {
    let is_hosting = todo!();
    let my_player = relative == Relative::My;

    html! {
        <section class="{} player">
            <h2 class="name">
            { if my_player && is_hosting { render_host_controls() } else { html! {} } }
            </h2>
            <div class="load">
            </div>
            <div class="bot">
            </div>
            <div class="timer">
            </div>
            <div class="turn">
            </div>
            <div class="passed">
            </div>
            <div class="control">
            </div>
            <div class="win">
            </div>
            { if my_player { render_cards_form() } else { html! {} } }
        </section>
    }
}

fn render_host_controls() -> Node {
    html! {
        <form hx-put="/api/timer">
            <label class="host-config token">
                "enable action timer"
                <input type="checkbox" name="enable-timer" hx-trigger="change" />
            </label>
            <input type="range" min="1" max="120000" value="30000" id="set-timer" name="host-config token" hx-trigger="change" />
        </form>
        <button hx-post="/api/start">start the game</button>
    }
}

fn render_cards_form(cards: Cards) -> Node {
//    self.game_state.hand(seat)
    html! {
        <form>
            { cards.into_iter()
                .map(|card| render_card(card)) 
            }
            <button type="submit" hx-post="/api/play">Play</button>
        </form>
    }
}

fn render_card(card: Card) -> Node {
    html! {
        <label class="card" data-card={format!("{}", card)} id={format!("{}", card)}>
            <input type="checkbox" class="card-check" hx-get="/api/playable" hx-trigger="input">
            <img src={format!("/assets/cards/{}.svg", card)} alt="" class="card-face">
        </label>
    }
}

//#[derive(Default)]
//struct LastRequestId(u64);
//
//impl Header for LastRequestId {
//    fn name() -> &'static HeaderName {
//        static NAME: HeaderName = HeaderName::from_static("Last-Request-ID");
//        &NAME
//    }
//
//    fn decode<'i, I>(values: &mut I) -> Result<Self, headers::Error>
//    where
//        I: Iterator<Item = &'i HeaderValue>,
//    {
//        let value = values
//            .next()
//            .ok_or_else(headers::Error::invalid)?
//            .to_str()
//            .map_err(|_| headers::Error::invalid())?
//            .parse::<u64>()
//            .map_err(|_| headers::Error::invalid())?;
//            
//        Ok(LastRequestId(value));
//    }
//    
//    fn encode<E>(&self, values: &mut E)
//    where
//        E: Extend<HeaderValue>,
//    {
//        let value = HeaderValue::try_from(self.0.to_string()).unwrap();
//        values.extend(std::iter::once(value));
//    }
//}
