const INACTIVE_BEFORE_DISCONNECT: Duration = Duration::from_secs(2 * 60);

pub fn api<S>() -> Router<S> {
    Router::new()
        .route("/", get(index))
        .route("/api/join", post(index))
        .route("/api/subscribe", get(subscribe))
        .route("/api/state", get(state))
        .route("/api/keep_alive", post(keep_alive))
        .route("/api/timer", put(timer))
        .route("/api/start", post(start))
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
    user_session: UserSession<Authenticated>,
) -> Result<impl IntoResponse> {
    user_session.keep_alive().await?;
}

#[debug_handler(state=Arc<Mutex<ApiState>>)]
async fn subscribe(
    user_session: UserSession<Authenticated>,
) -> Result<impl IntoResponse> {
    let (tx, rx) = mpsc::unbounded_channel();
    user_session.subscribe(tx).await?;
    Ok(Sse::new(rx))
}

#[debug_handler(state=Arc<Mutex<ApiState>>)]
async fn state(
    user_session: UserSession<Authenticated>,
) -> Result<impl IntoResponse> {
    user_session.state().await
}

#[serde_as]
#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
struct TimerRequest {
    #[serde_as(as = "Option<DurationMilliSeconds<u64>>")]
    timer_value: Duration,
    enable_timer: Option<String>,
}

#[debug_handler(state=Arc<Mutex<ApiState>>)]
async fn timer(
    user_session: UserSession<HostAuthenticated>,
    Form(timer_request): Form<TimerRequest>,
) -> Result<impl IntoResponse> {
    let timer = timer_request.enable_timer.map(|_| timer_value);
    host_session.timer(timer).await
}

#[debug_handler(state=Arc<Mutex<ApiState>>)]
async fn start(
    user_session: UserSession<HostAuthenticated>,
    Form(timer_request): Form<TimerRequest>,
) -> Result<impl IntoResponse> {
    host_session.start().await
}

#[debug_handler(state=Arc<Mutex<ApiState>>)]
async fn play(
    user_session: UserSession<Authenticated>,
    Form(play_request): Form<Vec<(Card, Card)>>,
) -> Result<impl IntoResponse> {
    let cards = Cards::from_iter(play_request.into_iter().map(|(card, _)| card));
    user_session.play(cards).await
}

#[debug_handler(state=Arc<Mutex<ApiState>>)]
async fn playable(
    user_session: UserSession<Authenticated>,
    Form(play_request): Form<Vec<(Card, Card)>>,
) -> Result<impl IntoResponse> {
    let cards = Cards::from_iter(play_request.into_iter().map(|(card, _)| card));
    user_session.playable(cards).await
}


struct ApiState {
    phases: HashMap<SessionId, Arc<Mutex<Session>>>,
}

impl ApiState {
    fn new() -> Arc<Mutex<Self>> {
        Arc::new_cyclic(|_this| {
            Mutex::new(Self {
                phases: HashMap::default(),
            })
        })
    }

    async fn join(
        &mut self,
        session_id: Option<SessionId>,
    ) -> Result<Authenticated> {
        let (session_id, phase) = match session_id {
            Some(session_id) => (session_id, self.get_session(session_id).await?),
            None => {
                let session_id = SessionId::random();
                let phase = Lobby::new(session_id, Weak::clone(&self.this));
                self.phases.insert(session_id, Arc::clone(&phase) as _);
                (session_id, phase)
            }
        };

        let auth = phase.lock().await.join().await?;

        let phase = phase.downgrade();
        task::spawn(async move {
            let mut interval = interval(INACTIVE_BEFORE_DISCONNECT);
            interval.set_missed_tick_behavior(MissedTickBehavior::Skip);

            loop {
                interval.tick().await;
                let Some(phase) = phase.upgrade() else { break };
                let mut phase = phase.lock().await;
                match phase.inactive_disconnect(auth).await {
                    ControlFlow::Continue(()) => {},
                    ControlFlow::Break(now_empty) => {
                        self.phases.remove(&session_id);
                        break;
                    }
                }
            }
        });

        Ok(auth)
    }

    async fn get_session(&self, session_id: SessionId) -> Result<Arc<Mutex<Session>>> {
        let phase = self.phases.get(&session_id).ok_or(Error::NoSession)?;
        Ok(Arc::clone(phase))
    }
}


struct UserSession<A> {
    session: OwnedMutexGuard<Session>,
    auth: A,
}

#[async_trait]
impl FromRequestParts<Arc<Mutex<ApiState>>> for UserSession<HostAuthenticated> {
    type Rejection = Response;
    async fn from_request_parts(
        parts: &mut Parts,
        state: &Arc<Mutex<ApiState>>,
    ) -> Result<Self, Self::Rejection> {
        let UserSession { mut session, auth } = parts
            .extract::<UserSession<Authenticated>>
            .await
            .map_err(|err| err.into_response())?;
        let auth = session.host_authenticate(auth)
            .await
            .map_err(|err| err.into_response())?;
        Ok(UserSession { session, auth })
    }
}

#[async_trait]
impl FromRequestParts<Arc<Mutex<ApiState>>> for UserSession<Authenticated> {
    type Rejection = Response;
    async fn from_request_parts(
        parts: &mut Parts,
        state: &Arc<Mutex<ApiState>>,
    ) -> Result<Self, Self::Rejection> {
        let UserSession { mut session, auth } = parts
            .extract::<UserSession<Unauthenticated>>
            .await
            .map_err(|err| err.into_response())?;
        let auth = phase.authenticate(auth).await
            .map_err(|err| err.into_response())?;
        Ok(UserSession { phase, auth })
    }
}

#[async_trait]
impl FromRequestParts<Arc<Mutex<ApiState>>> for UserSession<Unauthenticated> {
    type Rejection = Response;
    async fn from_request_parts(
        parts: &mut Parts,
        state: &Arc<Mutex<ApiState>>,
    ) -> Result<Self, Self::Rejection> {
        let jar = parts
            .extract::<CookieJar>()
            .await
            .map_err(|err| err.into_response())?;
        let auth = jar.get("auth")
            .ok_or_else(|| (StatusCode::UNAUTHORIZED, "no auth cookie found").into_response())?;
            .value();
        let auth = serde_json::from_str(auth)
            .map_err(|err| (StatusCode::UNAUTHORIZED, format!("{}", err)).into_response())?;
        let mut phase = state
            .lock()
            .await
            .get_session(auth.session_id)
            .await
            .map_err(|err| err.into_response())?
            .lock_owned()
            .await;
        Ok(UserSession { phase, auth })
    }
}

struct Session {
    timer: Option<Duration>,
    deadline: Option<Instant>,
    phase: Phase,
    game_state: GameState,
    humans: HashMap<Seat, Human>,
    session_id: SessionId,
    this: Weak<Mutex<Session>>,
}

impl Session {
    pub fn new(session_id: SessionId) -> Arc<Mutex<Self>> {
        Arc::new_cyclic(move |this| Mutex::new(Self {
            timer: None,
            deadline: None,
            game_state: GameState::new(),
            humans: HashMap::default(),
            session_id,
            this,
        })
    }

    pub async fn join(&mut self) -> Result<Authenticated> {
        if !matches!(self.phase, Phase::Lobby) {
            return Err(Error::BadPhase);
        }

        let seat = Seat::ALL.into_iter()
            .find(|seat| !self.humans.contains_key(&seat))
            .ok_or(Error::Full)?;
        let user_secret = UserSecret::random();
        let host = self.humans.is_empty();
        self.humans.insert(seat, Human { host, user_secret, last_active: Instant::now(), tx: None });

        self.alert_all(&Update::Connected).await;
        
        Ok(Authenticated {
            auth: Unauthenticated {
                user_secret,
                seat,
                session_id: self.session_id,
            }
        })
    }

    pub fn inactive_disconnect(&mut self, auth: Authenticated) -> ControlFlow<(), bool> {
        let Some(human) = self.humans.get_mut(&auth.seat) else { ControlFlow::Break(()) };
        let is_active = Instant::now().elapsed(human.last_active) < INACTIVE_BEFORE_DISCONNECT;
        if is_active {
            return ControlFlow::Continue(());
        } 
        if human.host {
            let new_host = self.humans.values_mut().next();
            if let Some(new_host) {
                new_host.host = true;
            }
        }
        let now_empty = self.humans.is_empty();

        self.alert_all(&Update::Connected).await;
        if matches!(self.phase, Phase::Active) {
            if self.game_state.current_player() == auth.seat {
                self.solicit().await;
            }
        }
        Ok(now_empty)
    }

    pub fn last_active(&mut self, auth: Authenticated) -> Result<()> {
        self.humans.get_mut(&auth.seat).ok_or(Error::Absent)?.last_active = Instant::now();
    }

    pub async fn subscribe(&mut self, auth: Authenticated, tx: Tx) -> Result<()> {
        self.humans.get_mut(&auth.seat).ok_or(Error::Absent)?.tx = tx;
        todo!("send update message");
        Ok(())
    }

    pub fn host_authenticate(&mut self, auth: Authenticated) -> Result<HostAuthenticated> {
        let is_host = self.humans.get(&auth.seat).is_some_and(|human| human.host);
        if !is_host {
            return Err(Error::NotHost);
        }
        Ok(HostAuthenticated { auth: auth.auth })
    }

    pub fn authenticate(&mut self, auth: Unauthenticated) -> Result<Authenticated> {
        let human = self.humans.get(&auth.seat).ok_or(Error::Absent)?;
        if human.user_secret != auth.user_secret {
            return Err(Error::BadAuthentication);
        }
        Ok(Authenticated { auth })
    }

    pub async fn timer(&mut self, auth: HostAuthenticated, timer: Option<Duration>) -> Result<()> {
        if !matches!(self.phase, Phase::Lobby) {
            return Err(Error::BadPhase);
        }
        // TODO: maybe send update
        self.timer = timer;
        Ok(())
    }

    pub async fn start(&mut self, auth: HostAuthenticated) -> Result<()> {
        if !matches!(self.phase, Phase::Lobby) {
            return Err(Error::BadPhase);
        }
        self.phase = Phase::Active;
        self.alert_all(&Update::Deal).await;
        self.solicit().await;

        Ok(())
    }

    pub async fn playable(&mut self, auth: Authenticated, cards: Cards) -> Result<()> {
        if !matches!(self.phase, Phase::Active) {
            return Err(Error::BadPhase);
        }
        let current_player = self.game_state.current_player();
        if auth.seat != current_player {
            return Err(Error::NotCurrent);
        }
        self.game_state.playable(cards)?;
        Ok(())
    }

    pub async fn human_play(&mut self, auth: Authenticated, cards: Cards) -> Result<()> {
        if !matches!(self.phase, Phase::Active) {
            return Err(Error::BadPhase);
        }
        let current_player = self.game_state.current_player();
        if auth.seat != current_player {
            return Err(Error::NotCurrent);
        }
        self.play(auth, cards).await?;
        Ok(())
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
        let load = self.game_state.hand(auth.seat).len();
        let win = load == 0;
        self.alert_all(&Update::Play).await;

        if win {
            self.win(auth.seat).await;
        } else {
            self.solicit().await;
        }

        Ok(())
    }

    async fn solicit(&mut self) {
        let current_player = self.game_state.current_player();
        let timer = if self.is_human(current_player) {
            self.timer
        } else {
            Some(BOT_ACTION_TIMER)
        };

        self.deadline = timer.and_then(|timer| Instant::now().checked_add(timer));

        self.alert_all(&Update::Turn).await;

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

    async fn alert(&self, seat: Seat, message: &Update) {
        let Some(tx) = self.humans.get(&seat).and_then(|human| &human.tx) else { return };
        let data = html! { <div></div> };
        let event = sse::Event::default()
            .data(data.to_string())
            .event("refresh")
            .keep_alive(KeepAlive::default());
        tx.send(Ok(event));
    }

    async fn alert_all(&mut self, message: &Update) {
        for seat in Seat::ALL {
            self.alert(seat, message).await;
        }
    }

    fn is_human(&self, seat: Seat) -> bool {
        self.humans.contains_key(&seat)
    }
}

enum Phase {
    Lobby,
    Active,
    Post,
}

struct Human {
    user_secret: UserSecret, 
    last_active: Instant,
    tx: Option<Tx>,
}

type Tx = UnboundedSender<Result<sse::Event, Infallible>>;

#[derive(Serialize)]
enum Update {
    All,
    Welcome,
    Deal,
    Host,
    Connected,
    Deal,
    Play,
    Solicit,
    Win,
}

impl Display for Update {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Update::All => write!(f, "all"),
            Update::Welcome => write!(f, "welcome"),
            Update::Host => write!(f, "host"),
            Update::Connected => write!(f, "connected"),
            Update::Deal => write!(f, "deal"),
            Update::Play => write!(f, "play"),
            Update::Solicit => write!(f, "solicit"),
            Update::Win => write!(f, "win"),
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
struct HostAuthenticated {
    auth: Authenticated,
}

impl HostAuthenticated {
    fn auth(&self) -> Authenticated {
        self.auth
    }
}

impl Deref for HostAuthenticated {
    type Target = Authenticated;
    fn deref(&self) -> &Self::Target {
        &self.auth
    }
}

#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
struct Authenticated {
    auth: Unauthenticated,
}

impl Deref for HostAuthenticated {
    type Target = Unauthenticated;
    fn deref(&self) -> &Self::Target {
        &self.auth
    }
}

#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
struct Unauthenticated {
    pub seat: Seat,
    pub session_id: SessionId,
    pub user_secret: UserSecret,
}

impl IntoResponseParts for Unauthenticated {
    type Error = Infallible;
    fn into_response_parts(self, mut res: ResponseParts) -> Result<ResponseParts, Self::Error> {
        let auth = serde_json::to_string(&self).unwrap();
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
        let auth = jar.get("auth")
            .ok_or_else(|| (StatusCode::UNAUTHORIZED, "no auth cookie found").into_response())?;
            .value();
        let auth = serde_json::from_str(auth)
            .map_err(|err| (StatusCode::UNAUTHORIZED, format!("{}", err)).into_response())?;
        Ok(auth)
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
            <label class="host-config">
                "enable action timer"
                <input type="checkbox" name="enable-timer" hx-trigger="change" />
            </label>
            <input type="range" min="1" max="120000" value="30000" id="set-timer" name="timer-value" hx-trigger="change" />
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
    let card = format!("{card}");
    html! {
        <label class="card" data-card={&card} id={&card}>
            <input type="checkbox" name={&card} hx-get="/api/playable" hx-trigger="change">
            <img src={format!("/assets/cards/{card}.svg")} alt="" class="card-face">
        </label>
    }
}
