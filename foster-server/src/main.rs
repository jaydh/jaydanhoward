//! Milestone 1 (scaffold) of the Foster migration — see
//! .claude/plans/iridescent-skipping-wall.md in the Claude session that
//! authored this for the full migration plan. This binary must conform to
//! the exact same container contract the current Leptos/Bazel-built image
//! does (port 8000, `GET /health_check`, same env vars) so that cutover is
//! "ship a new image under the same tag" with zero changes to
//! homelab/service/*.yaml.

use axum::routing::get;
use axum::http::StatusCode;
use std::collections::HashMap;
use std::net::SocketAddr;

async fn health_check() -> StatusCode {
    StatusCode::OK
}

#[tokio::main]
async fn main() {
    let machines = HashMap::new();
    let app = foster_server::router(machines).route("/health_check", get(health_check));

    let addr: SocketAddr = "0.0.0.0:8000".parse().unwrap();
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    println!("jaydanhoward (Foster) → http://{addr}");
    axum::serve(listener, app).await.unwrap();
}
