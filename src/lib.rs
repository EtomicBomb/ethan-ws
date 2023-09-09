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
    serde_json::json,
    std::{collections::HashMap, convert::Infallible, sync::Arc},
    tokio::{
        sync::mpsc::{unbounded_channel, UnboundedSender},
        sync::{Mutex, OwnedMutexGuard},
    },
    tokio_stream::{wrappers::UnboundedReceiverStream, Stream, StreamExt},
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
    Json(record): Json<Record>,
) -> impl IntoResponse {
    table.next_record_id.0 += 1;
    let id = RecordId(table.next_record_id.0);
    let json = json!({ "id": id, "record": record });
    table.notify(RecordEventKind::Create, &json).await;
    table.records.insert(id, record);
    Json(json)
}

async fn record_update(
    TableIdExtract {
        mut table,
        record_id,
    }: TableIdExtract,
    Json(record): Json<Record>,
) -> Result<impl IntoResponse> {
    let existing = table
        .records
        .get_mut(&record_id)
        .ok_or(Error::RecordNotFound)?;
    existing.fields.extend(record.fields);
    let json = json!({ "id": record_id, "record": existing });
    table.notify(RecordEventKind::Update, &json).await;
    Ok(Json(json))
}

async fn record_delete(
    TableIdExtract {
        mut table,
        record_id,
    }: TableIdExtract,
) -> Result<impl IntoResponse> {
    let existing = table
        .records
        .remove(&record_id)
        .ok_or(Error::RecordNotFound)?;
    let json = json!({ "id": record_id, "record": existing });
    table.notify(RecordEventKind::Delete, &json).await;
    Ok(Json(json))
}

async fn record_read_id(
    TableIdExtract { table, record_id }: TableIdExtract,
) -> Result<impl IntoResponse> {
    let existing = table.records.get(&record_id).ok_or(Error::RecordNotFound)?;
    Ok(Json(json!({ "id": record_id, "record": existing })))
}

async fn record_read_query(
    TableExtract { table }: TableExtract,
    Json(record): Json<Record>,
) -> Result<impl IntoResponse> {
    let existing: serde_json::Value = table
        .records
        .iter()
        .filter(|(_, r)| {
            record
                .fields
                .iter()
                .all(|(key, value)| r.fields.get(key) == Some(value))
        })
        .map(|(id, r)| json!({ "id": id, "record": r }))
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
    subscribers: Vec<UnboundedSender<String>>,
    records: HashMap<RecordId, Record>,
    next_record_id: RecordId,
}

impl Table {
    async fn notify<'a>(&mut self, kind: RecordEventKind, update: &serde_json::Value) {
        let message = json!({
            "kind": kind,
            "update": update,
        })
        .to_string();
        self.subscribers
            .retain(|stream| stream.send(message.clone()).is_ok());
    }

    fn new_subscriber(&mut self) -> impl Stream<Item = String> {
        let (tx, rx) = unbounded_channel();
        let rx = UnboundedReceiverStream::new(rx);
        self.subscribers.push(tx);
        rx
    }
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "lowercase")]
enum RecordEventKind {
    Create,
    Update,
    Delete,
}

#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(transparent)]
struct Record {
    fields: HashMap<String, Field>,
}

#[derive(PartialEq, Serialize, Deserialize, Debug)]
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
                Json(json!({ "reason": "table not found" })),
            ),
            Self::TableNotFound => (
                StatusCode::NOT_FOUND,
                Json(json!({ "reason": "record not found" })),
            ),
        }
        .into_response()
    }
}
