use {
    async_trait::async_trait,
    axum::{
        extract::{FromRequestParts, Path},
        response::{
            sse::{self, KeepAlive, Sse},
            IntoResponse, Response,
        },
        routing::{delete, get, patch, post},
        Json, RequestPartsExt, Router,
    },
    http::{request::Parts, status::StatusCode},
    serde::{Deserialize, Serialize},
    std::{collections::HashMap, convert::Infallible, sync::{Arc}},
    tokio::{
        sync::mpsc::{unbounded_channel, UnboundedSender},
        sync::{Mutex, OwnedMutexGuard},
    },
    tokio_stream::{wrappers::UnboundedReceiverStream, Stream, StreamExt},
    arc_swap::{ArcSwap},
};

pub fn api<S, I: IntoIterator<Item = T>, T: Into<String>>(tables: I) -> Router<S> {
    let tables = tables.into_iter().map(|s| s.into()).map(TableName);

    Router::new()
        .route("/create/:table", post(record_create))
        .route("/update/:table/:record_id", patch(record_update))
        .route("/delete/:table/:record_id", delete(record_delete))
        .route("/read-id/:table/:record_id", get(record_read_id))
        .route("/read-query/:table", post(record_read_query))
        .route("/subscribe/:table", get(record_subscribe))
        .with_state(Tables::new(tables))
}

async fn record_create(
    TableExtract { mut table }: TableExtract,
    Json(record): Json<Arc<Record>>,
) -> impl IntoResponse {
    table.next_record_id.0 += 1;
    let id = RecordId(table.next_record_id.0);
    let update = Update { id, record: Arc::clone(&record) };
    table.notify(Notification::Create(update.clone())).await;
    table.records.insert(id, ArcSwap::new(record));
    Json(update)
}

async fn record_update(
    TableIdExtract {
        mut table,
        record_id,
    }: TableIdExtract,
    Json(record): Json<Record>,
) -> Result<impl IntoResponse> {
    let existing_place = table
        .records
        .get(&record_id)
        .ok_or(Error::RecordNotFound)?;
    let mut existing = (**existing_place.load()).clone();
    existing.fields.extend(record.fields);
    let existing = Arc::new(existing);
    existing_place.store(Arc::clone(&existing));

    let update = Update { id: record_id, record: existing };
    table.notify(Notification::Update(update.clone())).await;
    Ok(Json(update))
}

async fn record_delete(
    TableIdExtract {
        mut table,
        record_id,
    }: TableIdExtract,
) -> Result<impl IntoResponse> {
    let record = table
        .records
        .remove(&record_id)
        .ok_or(Error::RecordNotFound)?
        .load()
        .clone();
    let update = Update { id: record_id, record };
    table.notify(Notification::Delete(update.clone())).await;
    Ok(Json(update))
}

async fn record_read_id(
    TableIdExtract { table, record_id }: TableIdExtract,
) -> Result<impl IntoResponse> {
    let record = table.records.get(&record_id).ok_or(Error::RecordNotFound)?;
    Ok(Json(Update { id: record_id, record: Arc::clone(&record.load()) }))
}

async fn record_read_query(
    TableExtract { table }: TableExtract,
    Json(record): Json<Record>,
) -> Result<impl IntoResponse> {
    let existing: Vec<_> = table
        .records
        .iter()
        .map(|(i, r)| (i, r.load()))
        .filter(|(_, r)| {
            record
                .fields
                .iter()
                .all(|(key, value)| r.fields.get(key) == Some(value))
        })
        .map(|(&id, r)| Update { id, record: Arc::clone(&r) })
        .collect();
    Ok(Json(existing))
}

async fn record_subscribe(TableExtract { mut table }: TableExtract) -> impl IntoResponse {
    let stream = table.new_subscriber();
    let stream = stream.map(|json| Ok::<_, Infallible>(sse::Event::default().data(json)));
    Sse::new(stream).keep_alive(KeepAlive::default())
}

struct Tables {
    tables: HashMap<TableName, Arc<Mutex<Table>>>,
}

impl Tables {
    fn new<I: IntoIterator<Item = TableName>>(tables: I) -> Arc<Mutex<Tables>> {
        Arc::new_cyclic(move |_this| {
            let tables = tables
                .into_iter()
                .map(|table| (table, Default::default()))
                .collect();
            Mutex::new(Tables { tables })
        })
    }
}

#[derive(Default)]
struct Table {
    records: HashMap<RecordId, ArcSwap<Record>>,
    next_record_id: RecordId,
    subscribers: Vec<UnboundedSender<String>>,
}

impl Table {
    async fn notify(&mut self, notification: Notification) {
        let notification = serde_json::to_string(&notification).unwrap();
        self.subscribers.retain(|stream| stream.send(notification.clone()).is_ok());
    }

    fn new_subscriber(&mut self) -> impl Stream<Item = String> {
        let (tx, rx) = unbounded_channel();
        let rx = UnboundedReceiverStream::new(rx);
        self.subscribers.push(tx);
        rx
    }
}

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
#[serde(transparent)]
struct Record {
    fields: HashMap<String, Field>,
}

#[derive(PartialEq, Serialize, Deserialize, Debug, Clone)]
#[serde(untagged)]
enum Field {
    Null,
    String(String),
    Number(f64),
    Boolean(bool),
}

#[derive(Serialize, Default, Deserialize, Clone, Debug, PartialEq, Eq, Hash)]
struct TableName(String);

#[derive(Serialize, Default, Deserialize, Copy, Clone, Debug, PartialEq, Eq, Hash)]
#[serde(transparent)]
struct RecordId(u64);

#[derive(Serialize, Debug)]
#[serde(rename_all = "lowercase")]
#[serde(tag = "kind", content = "update")]
enum Notification {
    Create(Update),
    Update(Update),
    Delete(Update),
}

#[derive(Serialize, Debug, Clone)]
struct Update {
    id: RecordId,
    record: Arc<Record>,
}

struct TableExtract {
    table: OwnedMutexGuard<Table>,
}

#[async_trait]
impl FromRequestParts<Arc<Mutex<Tables>>> for TableExtract {
    type Rejection = Response;
    async fn from_request_parts(
        parts: &mut Parts,
        state: &Arc<Mutex<Tables>>,
    ) -> Result<Self, Self::Rejection> {
        let Path(table) = parts
            .extract::<Path<TableName>>()
            .await
            .map_err(|err| err.into_response())?;
        let tables = state.lock().await;
        let table = tables
            .tables
            .get(&table)
            .ok_or_else(|| Error::TableNotFound.into_response())?;
        let table = Arc::clone(table).lock_owned().await;
        Ok(TableExtract { table })
    }
}

struct TableIdExtract {
    table: OwnedMutexGuard<Table>,
    record_id: RecordId,
}

#[async_trait]
impl FromRequestParts<Arc<Mutex<Tables>>> for TableIdExtract {
    type Rejection = Response;
    async fn from_request_parts(
        parts: &mut Parts,
        state: &Arc<Mutex<Tables>>,
    ) -> Result<Self, Self::Rejection> {
        let Path((table, record_id)) = parts
            .extract::<Path<(TableName, RecordId)>>()
            .await
            .map_err(|err| err.into_response())?;
        let tables = state.lock().await;
        let table = tables
            .tables
            .get(&table)
            .ok_or_else(|| Error::TableNotFound.into_response())?;
        let table = Arc::clone(table).lock_owned().await;
        Ok(TableIdExtract { table, record_id })
    }
}

enum Error {
    RecordNotFound,
    TableNotFound,
}

type Result<T, E = Error> = std::result::Result<T, E>;

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        match self {
            Self::RecordNotFound => (
                StatusCode::NOT_FOUND,
                Json(HashMap::from([("reason", "record not found")])),
            ),
            Self::TableNotFound => (
                StatusCode::NOT_FOUND,
                Json(HashMap::from([("reason", "table not found")])),
            ),
        }
        .into_response()
    }
}
