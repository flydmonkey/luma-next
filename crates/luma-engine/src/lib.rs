mod recorder;

use std::{
    sync::{Arc, Mutex},
    time::Instant,
};

use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    response::{Html, IntoResponse, Response},
    routing::{get, post},
};
use serde::Serialize;
use serde_json::Value;

pub use recorder::{ObsRecorder, RecordingValidation};

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

#[derive(Clone, Serialize)]
struct Session {
    state: &'static str,
    elapsed_seconds: f64,
    output_path: Option<String>,
    error: Option<String>,
    validation: Option<RecordingValidation>,
    #[serde(skip)]
    started: Option<Instant>,
}

impl Default for Session {
    fn default() -> Self {
        Self {
            state: "idle",
            elapsed_seconds: 0.0,
            output_path: None,
            error: None,
            validation: None,
            started: None,
        }
    }
}

impl Session {
    fn snapshot(&self) -> Self {
        let mut snapshot = self.clone();
        snapshot.elapsed_seconds = self
            .started
            .map_or(self.elapsed_seconds, |start| start.elapsed().as_secs_f64());
        snapshot
    }
}

struct Controller {
    recorder: Option<ObsRecorder>,
    session: Session,
}

type AppState = Arc<Mutex<Controller>>;

pub fn app() -> Router {
    router(Controller {
        recorder: None,
        session: Session::default(),
    })
}

pub fn app_with_recorder(recorder: ObsRecorder) -> Router {
    router(Controller {
        recorder: Some(recorder),
        session: Session::default(),
    })
}

fn router(controller: Controller) -> Router {
    Router::new()
        .route("/", get(index))
        .route("/api/v1", get(probe))
        .route("/api/v1/session", get(session))
        .route("/api/v1/session/start", post(start_recording))
        .route("/api/v1/session/stop", post(stop_recording))
        .with_state(Arc::new(Mutex::new(controller)))
}

async fn index() -> Html<&'static str> {
    Html(include_str!("index.html"))
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

async fn session(State(state): State<AppState>) -> Json<Envelope<Session>> {
    let session = state
        .lock()
        .expect("controller mutex poisoned")
        .session
        .snapshot();
    Json(Envelope {
        ok: true,
        data: session,
        error: None,
    })
}

async fn start_recording(State(state): State<AppState>) -> Response {
    let mut controller = state.lock().expect("controller mutex poisoned");
    if controller.session.state == "recording" {
        return error_response(StatusCode::CONFLICT, "recording is already active");
    }
    let Some(recorder) = controller.recorder.as_mut() else {
        return error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "libobs recorder is not initialized",
        );
    };
    match recorder.start() {
        Ok(path) => {
            controller.session = Session {
                state: "recording",
                elapsed_seconds: 0.0,
                output_path: Some(path.to_string_lossy().into_owned()),
                error: None,
                validation: None,
                started: Some(Instant::now()),
            };
            success_response(controller.session.snapshot())
        }
        Err(error) => {
            controller.session.state = "failed";
            controller.session.error = Some(error.clone());
            error_response(StatusCode::INTERNAL_SERVER_ERROR, error)
        }
    }
}

async fn stop_recording(State(state): State<AppState>) -> Response {
    let mut controller = state.lock().expect("controller mutex poisoned");
    if controller.session.state != "recording" {
        return error_response(StatusCode::CONFLICT, "no recording is active");
    }
    let Some(recorder) = controller.recorder.as_mut() else {
        return error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "libobs recorder is not initialized",
        );
    };
    match recorder.stop() {
        Ok((path, validation)) => {
            controller.session.state = "idle";
            controller.session.elapsed_seconds = validation.wall_seconds;
            controller.session.output_path = Some(path.to_string_lossy().into_owned());
            controller.session.validation = Some(validation);
            controller.session.started = None;
            controller.session.error = None;
            success_response(controller.session.snapshot())
        }
        Err(error) => {
            controller.session.state = "failed";
            controller.session.started = None;
            controller.session.error = Some(error.clone());
            error_response(StatusCode::INTERNAL_SERVER_ERROR, error)
        }
    }
}

fn success_response<T: Serialize>(data: T) -> Response {
    (
        StatusCode::OK,
        Json(Envelope {
            ok: true,
            data,
            error: None,
        }),
    )
        .into_response()
}

fn error_response(status: StatusCode, error: impl Into<String>) -> Response {
    (
        status,
        Json(Envelope {
            ok: false,
            data: Value::Null,
            error: Some(error.into()),
        }),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idle_session_has_no_fake_elapsed_time() {
        let session = Session::default().snapshot();
        assert_eq!(session.state, "idle");
        assert_eq!(session.elapsed_seconds, 0.0);
        assert!(session.output_path.is_none());
    }
}
