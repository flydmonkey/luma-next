mod recorder;
mod settings;

use axum::{
    Json, Router,
    extract::{Path as AxumPath, State},
    http::{StatusCode, header},
    response::{Html, IntoResponse, Response},
    routing::{delete, get, post},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    path::{Path, PathBuf},
    process::Command,
    sync::{Arc, Mutex},
    time::{Instant, UNIX_EPOCH},
};

pub use recorder::{EncoderInfo, ObsRecorder, RecordingValidation};
use settings::{Settings, SettingsStore, default_output_directory, default_settings_path};

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
    encoder_requested: Option<String>,
    encoder_active: Option<String>,
    encoder_fallback: bool,
    fallback_reason: Option<String>,
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
            encoder_requested: None,
            encoder_active: None,
            encoder_fallback: false,
            fallback_reason: None,
            started: None,
        }
    }
}
impl Session {
    fn snapshot(&self) -> Self {
        let mut value = self.clone();
        value.elapsed_seconds = self
            .started
            .map_or(self.elapsed_seconds, |start| start.elapsed().as_secs_f64());
        value
    }
}

struct Controller {
    recorder: Option<ObsRecorder>,
    session: Session,
    settings: SettingsStore,
}
type AppState = Arc<Mutex<Controller>>;

pub fn app() -> Router {
    router(controller(
        None,
        default_settings_path(),
        default_output_directory(),
    ))
}
pub fn app_with_recorder(recorder: ObsRecorder) -> Router {
    router(controller(
        Some(recorder),
        default_settings_path(),
        default_output_directory(),
    ))
}
#[doc(hidden)]
pub fn app_with_paths(settings_path: PathBuf, output_directory: PathBuf) -> Router {
    router(controller(None, settings_path, output_directory))
}

fn controller(
    recorder: Option<ObsRecorder>,
    settings_path: PathBuf,
    output_directory: PathBuf,
) -> Controller {
    let settings = SettingsStore::load(settings_path, output_directory)
        .unwrap_or_else(|error| panic!("cannot load Luma settings: {error}"));
    Controller {
        recorder,
        session: Session::default(),
        settings,
    }
}

fn router(controller: Controller) -> Router {
    Router::new()
        .route("/", get(index))
        .route("/app.css", get(styles))
        .route("/app.js", get(script))
        .route("/dev", get(dev_index))
        .route("/api/v1", get(probe))
        .route("/api/v1/encoders", get(encoders))
        .route("/api/v1/session", get(session))
        .route("/api/v1/session/start", post(start_recording))
        .route("/api/v1/session/stop", post(stop_recording))
        .route("/api/v1/library", get(library))
        .route("/api/v1/library/open", post(open_library))
        .route("/api/v1/library/{id}", delete(delete_library_item))
        .route("/api/v1/settings", get(get_settings).put(put_settings))
        .with_state(Arc::new(Mutex::new(controller)))
}

async fn index() -> Html<&'static str> {
    Html(include_str!("../../../web/index.html"))
}
async fn dev_index() -> Html<&'static str> {
    Html(include_str!("index.html"))
}
async fn styles() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/css; charset=utf-8")],
        include_str!("../../../web/app.css"),
    )
}
async fn script() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/javascript; charset=utf-8")],
        include_str!("../../../web/app.js"),
    )
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
#[derive(Serialize)]
struct Encoders {
    encoders: Vec<EncoderInfo>,
}
async fn encoders(State(state): State<AppState>) -> Response {
    let controller = state.lock().expect("controller mutex poisoned");
    let Some(recorder) = controller.recorder.as_ref() else {
        return error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "libobs recorder is not initialized",
        );
    };
    success_response(Encoders {
        encoders: recorder.encoders(),
    })
}
async fn session(State(state): State<AppState>) -> Json<Envelope<Session>> {
    let data = state
        .lock()
        .expect("controller mutex poisoned")
        .session
        .snapshot();
    Json(Envelope {
        ok: true,
        data,
        error: None,
    })
}

#[derive(Default, Deserialize)]
struct StartRequest {
    mode: Option<String>,
    system_audio: Option<bool>,
    microphone: Option<bool>,
    quality: Option<String>,
    encoder: Option<String>,
}

async fn start_recording(
    State(state): State<AppState>,
    request: Option<Json<StartRequest>>,
) -> Response {
    let mut controller = state.lock().expect("controller mutex poisoned");
    if controller.session.state == "recording" {
        return error_response(StatusCode::CONFLICT, "recording is already active");
    }
    let request = request.map_or_else(StartRequest::default, |Json(value)| value);
    let configured = controller.settings.current().clone();
    let mode = request.mode.as_deref().unwrap_or("display");
    let system_audio = request
        .system_audio
        .unwrap_or(configured.record_system_audio);
    let microphone = request.microphone.unwrap_or(configured.record_microphone);
    let quality = request.quality.as_deref().unwrap_or(&configured.quality);
    if mode != "display" {
        return error_response(
            StatusCode::UNPROCESSABLE_ENTITY,
            "only display capture is implemented",
        );
    }
    if !system_audio {
        return error_response(
            StatusCode::UNPROCESSABLE_ENTITY,
            "system audio is required by the M3 recorder",
        );
    }
    if microphone {
        return error_response(
            StatusCode::UNPROCESSABLE_ENTITY,
            "microphone capture is not implemented yet",
        );
    }
    if quality != "1080p30" {
        return error_response(
            StatusCode::UNPROCESSABLE_ENTITY,
            "only quality 1080p30 is supported",
        );
    }
    if controller.recorder.is_none() {
        return error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "libobs recorder is not initialized",
        );
    }
    let requested_encoder = request.encoder.unwrap_or(configured.encoder);
    let available = controller.recorder.as_ref().is_some_and(|recorder| {
        recorder
            .encoders()
            .iter()
            .any(|item| item.available && item.id == requested_encoder)
    });
    if !available {
        return error_response(
            StatusCode::UNPROCESSABLE_ENTITY,
            format!("encoder {requested_encoder} is not available"),
        );
    }
    let output_directory = PathBuf::from(configured.output_directory);
    let Some(recorder) = controller.recorder.as_mut() else {
        return error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "libobs recorder is not initialized",
        );
    };
    match recorder.start(&output_directory, &requested_encoder) {
        Ok(start) => {
            let fallback_reason = start.fallback_reason.clone();
            controller.session = Session {
                state: "recording",
                elapsed_seconds: 0.0,
                output_path: Some(start.path.to_string_lossy().into_owned()),
                error: None,
                validation: None,
                encoder_requested: Some(requested_encoder),
                encoder_active: Some(start.active_encoder),
                encoder_fallback: fallback_reason.is_some(),
                fallback_reason,
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

#[derive(Serialize)]
struct LibraryItem {
    id: String,
    name: String,
    path: String,
    size_bytes: u64,
    modified_unix_seconds: u64,
    duration_seconds: Option<f64>,
}

async fn library(State(state): State<AppState>) -> Response {
    let directory = {
        PathBuf::from(
            &state
                .lock()
                .expect("controller mutex poisoned")
                .settings
                .current()
                .output_directory,
        )
    };
    match list_recordings(&directory) {
        Ok(items) => success_response(items),
        Err(error) => error_response(StatusCode::INTERNAL_SERVER_ERROR, error),
    }
}

async fn delete_library_item(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
) -> Response {
    let directory = {
        PathBuf::from(
            &state
                .lock()
                .expect("controller mutex poisoned")
                .settings
                .current()
                .output_directory,
        )
    };
    match safe_recording_path(&directory, &id).and_then(|path| {
        std::fs::remove_file(&path)
            .map(|_| path)
            .map_err(|error| format!("failed to delete recording: {error}"))
    }) {
        Ok(path) => success_response(serde_json::json!({ "deleted": path })),
        Err(error) => error_response(StatusCode::BAD_REQUEST, error),
    }
}

async fn open_library(State(state): State<AppState>) -> Response {
    let directory = {
        PathBuf::from(
            &state
                .lock()
                .expect("controller mutex poisoned")
                .settings
                .current()
                .output_directory,
        )
    };
    if let Err(error) = std::fs::create_dir_all(&directory) {
        return error_response(StatusCode::INTERNAL_SERVER_ERROR, error.to_string());
    }
    match Command::new("explorer.exe").arg(&directory).spawn() {
        Ok(_) => success_response(serde_json::json!({ "opened": directory })),
        Err(error) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to open directory: {error}"),
        ),
    }
}

async fn get_settings(State(state): State<AppState>) -> Response {
    let data = state
        .lock()
        .expect("controller mutex poisoned")
        .settings
        .current()
        .clone();
    success_response(data)
}
async fn put_settings(State(state): State<AppState>, Json(settings): Json<Settings>) -> Response {
    let mut controller = state.lock().expect("controller mutex poisoned");
    if controller.session.state == "recording" {
        return error_response(
            StatusCode::CONFLICT,
            "settings cannot change while recording",
        );
    }
    let encoder_available =
        controller
            .recorder
            .as_ref()
            .map_or(settings.encoder == "obs_x264", |recorder| {
                recorder
                    .encoders()
                    .iter()
                    .any(|item| item.available && item.id == settings.encoder)
            });
    if !encoder_available {
        return error_response(
            StatusCode::UNPROCESSABLE_ENTITY,
            format!("encoder {} is not available", settings.encoder),
        );
    }
    match controller.settings.save(settings.clone()) {
        Ok(()) => success_response(settings),
        Err(error) => error_response(StatusCode::UNPROCESSABLE_ENTITY, error),
    }
}

fn list_recordings(directory: &Path) -> Result<Vec<LibraryItem>, String> {
    if !directory.exists() {
        return Ok(Vec::new());
    }
    let mut items = Vec::new();
    for entry in std::fs::read_dir(directory)
        .map_err(|error| format!("failed to read {}: {error}", directory.display()))?
    {
        let entry = entry.map_err(|error| format!("failed to read library entry: {error}"))?;
        let path = entry.path();
        if !entry
            .file_type()
            .map_err(|error| error.to_string())?
            .is_file()
            || !is_recording(&path)
        {
            continue;
        }
        let metadata = entry.metadata().map_err(|error| error.to_string())?;
        let name = entry.file_name().to_string_lossy().into_owned();
        items.push(LibraryItem {
            id: name.clone(),
            name,
            path: path.to_string_lossy().into_owned(),
            size_bytes: metadata.len(),
            modified_unix_seconds: metadata
                .modified()
                .ok()
                .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
                .map_or(0, |time| time.as_secs()),
            duration_seconds: probe_duration(&path),
        });
    }
    items.sort_by_key(|item| std::cmp::Reverse(item.modified_unix_seconds));
    Ok(items)
}

fn safe_recording_path(directory: &Path, id: &str) -> Result<PathBuf, String> {
    let file_name = Path::new(id);
    if id.is_empty()
        || file_name.file_name().and_then(|name| name.to_str()) != Some(id)
        || !is_recording(file_name)
    {
        return Err("invalid recording id".into());
    }
    let directory = std::fs::canonicalize(directory)
        .map_err(|error| format!("output directory is unavailable: {error}"))?;
    let path = std::fs::canonicalize(directory.join(file_name))
        .map_err(|_| "recording was not found".to_string())?;
    if path.parent() != Some(directory.as_path()) || !path.is_file() {
        return Err("recording is outside the output directory".into());
    }
    Ok(path)
}
fn is_recording(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|value| value.to_str())
            .map(str::to_ascii_lowercase)
            .as_deref(),
        Some("mkv" | "mp4" | "mov")
    )
}
fn probe_duration(path: &Path) -> Option<f64> {
    let output = Command::new("ffprobe.exe")
        .args([
            "-v",
            "error",
            "-show_entries",
            "format=duration",
            "-of",
            "default=noprint_wrappers=1:nokey=1",
        ])
        .arg(path)
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().parse().ok())
        .flatten()
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
    #[test]
    fn traversal_is_rejected() {
        let directory = tempfile::tempdir().unwrap();
        assert!(safe_recording_path(directory.path(), "../outside.mkv").is_err());
        assert!(safe_recording_path(directory.path(), "notes.txt").is_err());
    }
}
