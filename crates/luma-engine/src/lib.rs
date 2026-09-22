use axum::{Json, Router, routing::get};
use serde::Serialize;

#[derive(Serialize)]
struct Envelope<T> {
    ok: bool,
    data: T,
    error: Option<String>,
}

#[derive(Serialize)]
struct Probe {
    name: &'static str,
    control: bool,
}

#[derive(Serialize)]
struct Session {
    state: &'static str,
    output_path: Option<String>,
    error: Option<String>,
}

pub fn app() -> Router {
    Router::new()
        .route("/api/v1", get(probe))
        .route("/api/v1/session", get(session))
}

async fn probe() -> Json<Envelope<Probe>> {
    Json(Envelope {
        ok: true,
        data: Probe {
            name: "luma-next",
            control: true,
        },
        error: None,
    })
}

async fn session() -> Json<Envelope<Session>> {
    Json(Envelope {
        ok: true,
        data: Session {
            state: "idle",
            output_path: None,
            error: None,
        },
        error: None,
    })
}
