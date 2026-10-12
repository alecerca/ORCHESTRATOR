//! `orquestador-proxy` CLI.
//!
//! * Serve mode (default): `orquestador-proxy [config-path]` loads `proxy.json`,
//!   resolves the upstream password, and serves the role-enforcing reverse proxy.
//! * Owner tooling: `orquestador-proxy adduser <name> --role <editor|viewer>
//!   [--config <path>]` reads a password from stdin and appends an
//!   Argon2id-hashed user to the config file.
//!
//! Config path resolution for serve: first CLI argument, else
//! `ORQUESTADOR_PROXY_CONFIG`, else `./proxy.json`. For `adduser`: `--config`,
//! else `ORQUESTADOR_PROXY_CONFIG`, else `./proxy.json`.

use std::sync::Arc;

use orquestador_proxy::{
    adduser,
    config::{self, Config, Role},
    proxy,
};

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();

    let result = match args.first().map(String::as_str) {
        Some("adduser") => run_adduser(&args),
        _ => run_serve(args.first().cloned()).await,
    };

    if let Err(err) = result {
        eprintln!("orquestador-proxy: {err}");
        std::process::exit(1);
    }
}

/// Default serve mode: load the config and run the reverse proxy.
async fn run_serve(config_arg: Option<String>) -> Result<(), String> {
    let config_path = config_arg
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

/// `adduser` subcommand: append a hashed user to the config file.
fn run_adduser(args: &[String]) -> Result<(), String> {
    let mut name: Option<String> = None;
    let mut role: Option<Role> = None;
    let mut config_path: Option<String> = None;

    // `args[0]` is the `adduser` subcommand itself.
    let mut index = 1;
    while index < args.len() {
        match args[index].as_str() {
            "--role" => {
                let value = args
                    .get(index + 1)
                    .ok_or("adduser: --role requires a value")?;
                role = Some(parse_role(value)?);
                index += 2;
            }
            "--config" => {
                let value = args
                    .get(index + 1)
                    .ok_or("adduser: --config requires a path")?;
                config_path = Some(value.clone());
                index += 2;
            }
            other if !other.starts_with("--") && name.is_none() => {
                name = Some(other.to_string());
                index += 1;
            }
            other => return Err(format!("adduser: unexpected argument `{other}`")),
        }
    }

    let name = name.ok_or("adduser: missing <name>")?;
    let role = role.ok_or("adduser: missing --role <editor|viewer>")?;

    let path = config_path
        .or_else(|| std::env::var("ORQUESTADOR_PROXY_CONFIG").ok())
        .unwrap_or_else(|| "./proxy.json".to_string());

    let password = read_password_from_stdin()?;
    let raw = std::fs::read_to_string(&path).map_err(|err| format!("{path}: {err}"))?;
    let updated = adduser::add_user_to_json(&raw, &name, role, &password)
        .map_err(|err| format!("{path}: {err}"))?;
    std::fs::write(&path, updated).map_err(|err| format!("{path}: {err}"))?;

    println!("added user `{name}` ({}) to {path}", role.as_str());
    Ok(())
}

/// Parse a role name from the CLI.
fn parse_role(value: &str) -> Result<Role, String> {
    match value {
        "editor" => Ok(Role::Editor),
        "viewer" => Ok(Role::Viewer),
        other => Err(format!(
            "adduser: invalid role `{other}` (expected editor|viewer)"
        )),
    }
}

/// Read a single password line from stdin. The value is never echoed by us.
fn read_password_from_stdin() -> Result<String, String> {
    let mut buffer = String::new();
    std::io::stdin()
        .read_line(&mut buffer)
        .map_err(|err| format!("could not read password: {err}"))?;
    Ok(buffer.trim_end_matches(['\n', '\r']).to_string())
}
