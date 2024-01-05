use {
    axum_core::{
        body::Body,
        response::{IntoResponse, Response},
    },
    axum_extra::typed_header::TypedHeader,
    bytes::Bytes,
    futures_util::stream::Stream,
    headers::{CacheControl, ContentType},
    mime::Mime,
    serde::Serialize,
    futures::{stream::StreamExt},
    std::{
        convert::Infallible,
        io::Write as _,
        str::FromStr,
    },
};

pub struct JsonSeq<S> {
    pub stream: S,
}

impl<S, T> IntoResponse for JsonSeq<S>
where
    S: Stream<Item = T> + Send + 'static,
    T: Serialize,
{
    fn into_response(self) -> Response {
        let mime = TypedHeader(ContentType::from(Mime::from_str("application/json-seq").unwrap()));
        let no_cache = TypedHeader(CacheControl::new().with_no_cache());
        let body = self.stream
            .map(|data| {
                let mut bytes = Vec::with_capacity(16);
                write!(bytes, "\x1e").unwrap();
                serde_json::to_writer(&mut bytes, &data)
                    .expect("json stream data should be serializable");
                writeln!(bytes).unwrap();
                Ok::<_, Infallible>(Bytes::from(bytes))
            });
        let body = Body::from_stream(body);
        (mime, no_cache, body).into_response()
    }
}
