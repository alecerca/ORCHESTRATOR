//! `orquestador-proxy` CLI: load `proxy.json`, resolve the upstream password,
//! and serve the role-enforcing reverse proxy.
//!
//! Config path resolution: first CLI argument, else `ORQUESTADOR_PROXY_CONFIG`,
//! else `./proxy.json`.

use std::sync::Arc;

use orquestador_proxy::{config, config::Config, proxy};

#[tokio::main]
async fn main() {
    if let Err(err) = run().await {
        eprintln!("orquestador-proxy: {err}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), String> {
    let config_path = std::env::args()
        .nth(1)
        .or_else(|| std::env::var("ORQUESTADOR_PROXY_CONFIG").ok())
        .unwrap_or_else(|| "./proxy.json".to_string());

    let cfg = Config::load(&config_path).map_err(|err| format!("{config_path}: {err}"))?;

    // Env wins over the config field.
    let env_password = std::env::var("ORQUESTADOR_UPSTREAM_PASSWORD").ok();
    let upstream_password =
        config::pick_upstream_password(env_password, cfg.upstream_password.clone());
    if upstream_password.is_none() {
        eprintln!(
            "warning: no upstream password set (ORQUESTADOR_UPSTREAM_PASSWORD or `upstream_password`); \
             upstream Basic auth will use an empty password"
        );
    }

    let listen = cfg.listen.clone();
    let upstream = cfg.upstream.clone();
    let state = Arc::new(proxy::AppState::new(cfg, upstream_password));
    let app = proxy::router(state);

    let listener = tokio::net::TcpListener::bind(&listen)
        .await
        .map_err(|err| format!("could not bind {listen}: {err}"))?;
    eprintln!("orquestador-proxy listening on {listen} -> {upstream}");

    axum::serve(listener, app)
        .await
        .map_err(|err| format!("server error: {err}"))
}
