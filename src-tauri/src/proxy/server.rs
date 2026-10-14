use crate::proxy::{handler, ProxyRegistry};
use axum::{extract::DefaultBodyLimit, routing::post, Router};
use std::time::Duration;

pub(crate) fn router(registry: ProxyRegistry) -> Result<Router, reqwest::Error> {
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(120))
        .no_proxy()
        .build()?;
    Ok(Router::new()
        .route(
            "/proxy/:provider_id/v1/chat/completions",
            post(handler::proxy_handler),
        )
        .layer(DefaultBodyLimit::max(handler::MAX_BODY))
        .with_state(handler::AppState { registry, client }))
}

pub fn start_proxy_server(registry: ProxyRegistry) {
    start_on_port(registry, 18318);
}

pub(super) fn start_on_port(registry: ProxyRegistry, port: u16) {
    // Bind before returning so an occupied port never masquerades as our service.
    // No process is killed, no alternate port or existing listener is trusted.
    let listener = match std::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, port)) {
        Ok(listener) => listener,
        Err(_) => {
            eprintln!("Gemini proxy unavailable: cannot bind 127.0.0.1:18318; app remains usable");
            return;
        }
    };
    if listener.set_nonblocking(true).is_err() {
        return;
    }
    let app = match router(registry.clone()) {
        Ok(app) => app,
        Err(_) => return,
    };
    registry.set_running(true);
    tauri::async_runtime::spawn(async move {
        if let Ok(listener) = tokio::net::TcpListener::from_std(listener) {
            let _ = axum::serve(listener, app).await;
        }
        registry.set_running(false);
        eprintln!("Gemini proxy stopped; app remains usable");
    });
}
