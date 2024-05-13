use {
    axum::{
        extract::Host, handler::HandlerWithoutStateExt, response::IntoResponse, response::Redirect,
        Router,
    },
    axum_server::tls_rustls::RustlsConfig,
    http::{
        header::HeaderValue,
        status::StatusCode,
        uri::{PathAndQuery, Scheme},
        Uri,
    },
    std::net::{Ipv4Addr, SocketAddr},
    tower_http::{
        services::{ServeDir, ServeFile},
        set_header::SetResponseHeaderLayer,
    },
};

mod htmx;
mod pusoy;
mod records;
mod word;

#[tokio::main]
async fn main() {
    tokio::join!(serve(443), redirect_http_to_https(80, 443));
}

async fn serve(on_port: u16) {
    let serve_api = Router::new()
        .nest("/pusoy/api", pusoy::api())
        .nest("/word/api", word::api())
        .nest(
            "/applab/chess/records",
            records::api(["games", "broadcasts", "newtable"]),
        )
        .nest(
            "/applab/cool/records",
            records::api(["players", "preferences", "food"]),
        )
        .nest(
            "/applab/bounce/records",
            records::api(["singleplayerScores"]),
        );

    let serve_static = ServeDir::new("static")
        .append_index_html_on_directories(true)
        .not_found_service(ServeFile::new("static/not-found.html"));

    let serve = Router::new()
        .nest("/", serve_api)
        .fallback_service(serve_static)
        .layer(SetResponseHeaderLayer::overriding(
            http::header::CACHE_CONTROL,
            HeaderValue::from_static("no-store, must-revalidate"),
        ));

    let tls = RustlsConfig::from_pem_file("secret/cert.pem", "secret/key.pem")
        .await
        .unwrap();

    let addr = SocketAddr::from((Ipv4Addr::UNSPECIFIED, on_port));
    axum_server::bind_rustls(addr, tls)
        .serve(serve.into_make_service())
        .await
        .unwrap();
}

async fn redirect_http_to_https(from_port: u16, to_port: u16) {
    fn helper(
        host: String,
        uri: Uri,
        from_port: u16,
        to_port: u16,
    ) -> axum::response::Result<impl IntoResponse> {
        let host = host.replace(&from_port.to_string(), &to_port.to_string());
        let host = host.parse().map_err(|e| {
            (
                StatusCode::BAD_REQUEST,
                format!("could not parse host: {}", e),
            )
        })?;
        let mut parts = uri.into_parts();
        parts.scheme = Some(Scheme::HTTPS);
        parts
            .path_and_query
            .get_or_insert(PathAndQuery::from_static("/"));
        parts.authority = Some(host);
        let uri = Uri::from_parts(parts).map_err(|e| {
            (
                StatusCode::BAD_REQUEST,
                format!("could not construct url: {}", e),
            )
        })?;
        Ok(Redirect::permanent(&uri.to_string()))
    }

    let redirect = move |Host(host), uri: Uri| async move { helper(host, uri, from_port, to_port) };

    let addr = SocketAddr::from((Ipv4Addr::UNSPECIFIED, from_port));
    axum_server::bind(addr)
        .serve(redirect.into_make_service())
        .await
        .unwrap();
}
