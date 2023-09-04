mod api;
mod game;
mod hx;
mod json_seq;

use {
    axum::{
        Router,
    },
    http::{header::HeaderValue},
    std::{
        net::{Ipv4Addr, SocketAddr},
        time::Duration,
    },
    tower_http::{
        LatencyUnit,
        services::{ServeDir, ServeFile},
        set_header::SetResponseHeaderLayer,
        trace::{TraceLayer, DefaultOnResponse, DefaultMakeSpan, DefaultOnRequest, DefaultOnBodyChunk, DefaultOnEos},
    },
    tracing_subscriber::layer::SubscriberExt,
    tracing_subscriber::util::SubscriberInitExt,
};

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "pusoy=debug,tower_http=trace,axum::rejection=trace".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let serve_api = crate::api::api();

    let serve_static = ServeDir::new("www")
        .not_found_service(ServeFile::new("www/not_found.html"))
        .append_index_html_on_directories(true);

    let serve = Router::new()
        .nest("/", serve_api)
        .fallback_service(serve_static)
        .layer(SetResponseHeaderLayer::overriding(
            http::header::CACHE_CONTROL,
            HeaderValue::from_static("no-store, must-revalidate"),
        ))
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(DefaultMakeSpan::new().include_headers(true))
                .on_request(DefaultOnRequest::new())
                .on_response(DefaultOnResponse::new().latency_unit(LatencyUnit::Micros))
                .on_body_chunk(DefaultOnBodyChunk::new())
                .on_eos(DefaultOnEos::new()),
        );

    let addr = SocketAddr::from((Ipv4Addr::UNSPECIFIED, 8000));
    eprintln!("listening on {}", addr);

    axum_server::bind(addr)
        .serve(serve.into_make_service())
        .await
        .unwrap();
}
