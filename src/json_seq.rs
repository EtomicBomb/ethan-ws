use {
    axum_core::{
        body,
        response::{IntoResponse, Response},
    },
    bytes::Bytes,
    futures_util::stream::Stream,
    headers::{CacheControl, ContentType, HeaderMapExt},
    http::header::HeaderMap,
    http_body::Body,
    mime::Mime,
    pin_project::pin_project,
    serde::Serialize,
    std::{
        convert::Infallible,
        io::Write as _,
        pin::Pin,
        str::FromStr,
        task::{Context, Poll},
    },
};

const INIT_MESSAGE_CAPACITY: usize = 128;

pub struct JsonSeq<S> {
    pub stream: S,
}

impl<S, T> IntoResponse for JsonSeq<S>
where
    S: Stream<Item = T> + Send + 'static,
    T: Serialize,
{
    fn into_response(self) -> Response {
        let mut headers = HeaderMap::new();
        headers.typed_insert(ContentType::from(
            Mime::from_str("application/json-seq").unwrap(),
        ));
        headers.typed_insert(CacheControl::new().with_no_cache());
        let body = body::boxed(JsonSeqBody {
            stream: self.stream,
        });
        (headers, body).into_response()
    }
}

#[pin_project]
struct JsonSeqBody<S> {
    #[pin]
    stream: S,
}

impl<S, T> Body for JsonSeqBody<S>
where
    S: Stream<Item = T>,
    T: Serialize,
{
    type Data = Bytes;
    type Error = Infallible;

    fn poll_data(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Self::Data, Self::Error>>> {
        match self.project().stream.poll_next(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(Some(data)) => {
                let mut bytes = Vec::with_capacity(INIT_MESSAGE_CAPACITY);
                write!(bytes, "\x1e").unwrap();
                serde_json::to_writer(&mut bytes, &data)
                    .expect("json stream data should be serializable");
                writeln!(bytes).unwrap();
                Poll::Ready(Some(Ok(Bytes::from(bytes))))
            }
            Poll::Ready(None) => Poll::Ready(None),
        }
    }

    fn poll_trailers(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
    ) -> Poll<Result<Option<HeaderMap>, Self::Error>> {
        Poll::Ready(Ok(None))
    }
}
