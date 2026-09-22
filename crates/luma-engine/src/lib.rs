mod recorder;
mod settings;
mod tools;

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
    time::UNIX_EPOCH,
};

pub use recorder::{
    AudioDeviceInfo, CaptureOptions, CaptureTarget, EncoderInfo, ObsRecorder, RecordingValidation,
};
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
    media_elapsed_seconds: f64,
    wall_elapsed_seconds: f64,
    output_path: Option<String>,
    error: Option<String>,
    validation: Option<RecordingValidation>,
    encoder_requested: Option<String>,
    encoder_active: Option<String>,
    encoder_fallback: bool,
    fallback_reason: Option<String>,
    capture_mode: String,
    target_summary: String,
    system_audio: bool,
    microphone: bool,
    audio_sources_active: Vec<String>,
    warning: Option<String>,
}
impl Default for Session {
    fn default() -> Self {
        Self {
            state: "idle",
            elapsed_seconds: 0.0,
            media_elapsed_seconds: 0.0,
            wall_elapsed_seconds: 0.0,
            output_path: None,
            error: None,
            validation: None,
            encoder_requested: None,
            encoder_active: None,
            encoder_fallback: false,
            fallback_reason: None,
            capture_mode: "display".into(),
            target_summary: "主显示器".into(),
            system_audio: true,
            microphone: false,
            audio_sources_active: Vec::new(),
            warning: None,
        }
    }
}
impl Session {
    fn snapshot(&self) -> Self {
        self.clone()
    }
}

fn session_snapshot(controller: &Controller) -> Session {
    let mut session = controller.session.snapshot();
    if session.state == "recording" {
        if let Some(recorder) = controller.recorder.as_ref() {
            let (media, wall) = recorder.elapsed();
            apply_elapsed(&mut session, media, wall);
        }
    }
    session
}

fn apply_elapsed(session: &mut Session, media: f64, wall: f64) {
    session.elapsed_seconds = media;
    session.media_elapsed_seconds = media;
    session.wall_elapsed_seconds = wall;
}

struct Controller {
    recorder: Option<ObsRecorder>,
    session: Session,
    settings: SettingsStore,
}
type AppState = Arc<Mutex<Controller>>;

#[derive(Clone)]
pub struct Engine {
    state: AppState,
}

#[derive(Clone, Debug)]
pub struct TrayStatus {
    pub state: String,
    pub elapsed_seconds: f64,
    pub output_path: Option<String>,
    pub error: Option<String>,
    pub encoder_active: Option<String>,
    pub capture_mode: String,
    pub target_summary: String,
}

impl Engine {
    pub fn new(recorder: ObsRecorder) -> Self {
        Self {
            state: Arc::new(Mutex::new(controller(
                Some(recorder),
                default_settings_path(),
                default_output_directory(),
            ))),
        }
    }

    pub fn router(&self) -> Router {
        router_with_state(self.state.clone())
    }

    pub fn tray_status(&self) -> TrayStatus {
        let controller = self.state.lock().expect("controller mutex poisoned");
        let session = session_snapshot(&controller);
        TrayStatus {
            state: session.state.into(),
            elapsed_seconds: session.elapsed_seconds,
            output_path: session.output_path,
            error: session.error,
            encoder_active: session.encoder_active,
            capture_mode: session.capture_mode,
            target_summary: session.target_summary,
        }
    }

    pub fn start_default(&self) -> Result<TrayStatus, String> {
        let mut controller = self.state.lock().expect("controller mutex poisoned");
        start_locked(&mut controller, StartRequest::default())
            .map(|_| self.tray_status_from_locked(&controller))
            .map_err(|(_, error)| error)
    }

    pub fn stop(&self) -> Result<TrayStatus, String> {
        let mut controller = self.state.lock().expect("controller mutex poisoned");
        stop_locked(&mut controller)
            .map(|_| self.tray_status_from_locked(&controller))
            .map_err(|(_, error)| error)
    }

    fn tray_status_from_locked(&self, controller: &Controller) -> TrayStatus {
        let session = session_snapshot(controller);
        TrayStatus {
            state: session.state.into(),
            elapsed_seconds: session.elapsed_seconds,
            output_path: session.output_path,
            error: session.error,
            encoder_active: session.encoder_active,
            capture_mode: session.capture_mode,
            target_summary: session.target_summary,
        }
    }
}

pub fn app() -> Router {
    router(controller(
        None,
        default_settings_path(),
        default_output_directory(),
    ))
}
pub fn app_with_recorder(recorder: ObsRecorder) -> Router {
    Engine::new(recorder).router()
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
    router_with_state(Arc::new(Mutex::new(controller)))
}

fn router_with_state(state: AppState) -> Router {
    Router::new()
        .route("/", get(index))
        .route("/app.css", get(styles))
        .route("/app.js", get(script))
        .route("/dev", get(dev_index))
        .route("/api/v1", get(probe))
        .route("/api/v1/encoders", get(encoders))
        .route("/api/v1/targets", get(targets))
        .route("/api/v1/devices/audio", get(audio_devices))
        .route("/api/v1/session", get(session))
        .route("/api/v1/session/start", post(start_recording))
        .route("/api/v1/session/stop", post(stop_recording))
        .route("/api/v1/library", get(library))
        .route("/api/v1/library/open", post(open_library))
        .route("/api/v1/library/{id}", delete(delete_library_item))
        .route("/api/v1/settings", get(get_settings).put(put_settings))
        .with_state(state)
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
#[derive(Serialize)]
struct Targets {
    displays: Vec<Value>,
    windows: Vec<CaptureTarget>,
}
async fn targets(State(state): State<AppState>) -> Response {
    let controller = state.lock().expect("controller mutex poisoned");
    let Some(recorder) = controller.recorder.as_ref() else {
        return error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "libobs recorder is not initialized",
        );
    };
    success_response(Targets {
        displays: vec![
            serde_json::json!({"id":"primary","name":"主显示器","primary":true,"available":true}),
        ],
        windows: recorder.windows(),
    })
}
#[derive(Serialize)]
struct AudioDevices {
    outputs: Vec<AudioDeviceInfo>,
    inputs: Vec<AudioDeviceInfo>,
}
async fn audio_devices(State(state): State<AppState>) -> Response {
    let controller = state.lock().expect("controller mutex poisoned");
    let Some(recorder) = controller.recorder.as_ref() else {
        return error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "libobs recorder is not initialized",
        );
    };
    let devices = recorder.audio_devices();
    success_response(AudioDevices {
        outputs: devices
            .iter()
            .filter(|item| item.kind == "output")
            .cloned()
            .collect(),
        inputs: devices
            .into_iter()
            .filter(|item| item.kind == "input")
            .collect(),
    })
}
async fn session(State(state): State<AppState>) -> Json<Envelope<Session>> {
    let controller = state.lock().expect("controller mutex poisoned");
    let data = session_snapshot(&controller);
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
    window_id: Option<String>,
    mic_device_id: Option<String>,
}

async fn start_recording(State(state): State<AppState>, body: axum::body::Bytes) -> Response {
    let mut controller = state.lock().expect("controller mutex poisoned");
    let request = if body.is_empty() {
        StartRequest::default()
    } else {
        match serde_json::from_slice(&body) {
            Ok(request) => request,
            Err(error) => {
                return error_response(
                    StatusCode::BAD_REQUEST,
                    format!("invalid start request JSON: {error}"),
                );
            }
        }
    };
    match start_locked(&mut controller, request) {
        Ok(session) => success_response(session),
        Err((status, error)) => error_response(status, error),
    }
}

fn start_locked(
    controller: &mut Controller,
    request: StartRequest,
) -> Result<Session, (StatusCode, String)> {
    if controller.session.state == "recording" {
        return Err((StatusCode::CONFLICT, "recording is already active".into()));
    }
    let configured = controller.settings.current().clone();
    let mode = request.mode.as_deref().unwrap_or(&configured.capture_mode);
    let system_audio = request
        .system_audio
        .unwrap_or(configured.record_system_audio);
    let microphone = request.microphone.unwrap_or(configured.record_microphone);
    let quality = request.quality.as_deref().unwrap_or(&configured.quality);
    if !matches!(mode, "display" | "window") {
        return Err((
            StatusCode::UNPROCESSABLE_ENTITY,
            "only display and window capture are implemented".into(),
        ));
    }
    if !system_audio && !microphone {
        return Err((
            StatusCode::UNPROCESSABLE_ENTITY,
            "at least one of system audio or microphone must be enabled".into(),
        ));
    }
    if quality != "1080p30" {
        return Err((
            StatusCode::UNPROCESSABLE_ENTITY,
            "only quality 1080p30 is supported".into(),
        ));
    }
    if controller.recorder.is_none() {
        return Err((
            StatusCode::SERVICE_UNAVAILABLE,
            "libobs recorder is not initialized".into(),
        ));
    }
    let requested_encoder = request.encoder.unwrap_or(configured.encoder);
    let window_id = request.window_id.or(configured.window_id);
    let mic_device_id = request.mic_device_id.unwrap_or(configured.mic_device_id);
    let available = controller.recorder.as_ref().is_some_and(|recorder| {
        recorder
            .encoders()
            .iter()
            .any(|item| item.available && item.id == requested_encoder)
    });
    if !available {
        return Err((
            StatusCode::UNPROCESSABLE_ENTITY,
            format!("encoder {requested_encoder} is not available"),
        ));
    }
    if mode == "window" {
        let Some(id) = window_id.as_deref() else {
            return Err((
                StatusCode::UNPROCESSABLE_ENTITY,
                "window_id is required for window capture".into(),
            ));
        };
        let listed = controller
            .recorder
            .as_ref()
            .is_some_and(|recorder| recorder.windows().iter().any(|item| item.id == id));
        if !listed {
            return Err((
                StatusCode::UNPROCESSABLE_ENTITY,
                "window_id is no longer present in the current capture target list".into(),
            ));
        }
    }
    if microphone {
        let listed = controller.recorder.as_ref().is_some_and(|recorder| {
            recorder
                .audio_devices()
                .iter()
                .any(|item| item.kind == "input" && item.id == mic_device_id)
        });
        if !listed {
            return Err((
                StatusCode::UNPROCESSABLE_ENTITY,
                format!("microphone device {mic_device_id} is unavailable"),
            ));
        }
    }
    let output_directory = PathBuf::from(configured.output_directory);
    let Some(recorder) = controller.recorder.as_mut() else {
        return Err((
            StatusCode::SERVICE_UNAVAILABLE,
            "libobs recorder is not initialized".into(),
        ));
    };
    let target_summary = if mode == "window" {
        recorder
            .windows()
            .into_iter()
            .find(|item| Some(item.id.as_str()) == window_id.as_deref())
            .map_or_else(|| "窗口".into(), |item| item.title)
    } else {
        "主显示器".into()
    };
    match recorder.start(
        &output_directory,
        &requested_encoder,
        CaptureOptions {
            mode,
            window_id: window_id.as_deref(),
            system_audio,
            microphone,
            mic_device_id: Some(&mic_device_id),
        },
    ) {
        Ok(start) => {
            let fallback_reason = start.fallback_reason.clone();
            controller.session = Session {
                state: "recording",
                elapsed_seconds: 0.0,
                media_elapsed_seconds: 0.0,
                wall_elapsed_seconds: 0.0,
                output_path: Some(start.path.to_string_lossy().into_owned()),
                error: None,
                validation: None,
                encoder_requested: Some(requested_encoder),
                encoder_active: Some(start.active_encoder),
                encoder_fallback: fallback_reason.is_some(),
                fallback_reason,
                capture_mode: mode.to_string(),
                target_summary,
                system_audio,
                microphone,
                audio_sources_active: [
                    system_audio.then_some("system".into()),
                    microphone.then_some("microphone".into()),
                ]
                .into_iter()
                .flatten()
                .collect(),
                warning: (mode == "window").then_some(
                    "目标窗口最小化或关闭后画面可能变黑；请停止录制并重新选择窗口".into(),
                ),
            };
            Ok(session_snapshot(controller))
        }
        Err(error) => {
            controller.session.state = "failed";
            controller.session.error = Some(error.clone());
            Err((StatusCode::INTERNAL_SERVER_ERROR, error))
        }
    }
}

async fn stop_recording(State(state): State<AppState>) -> Response {
    let mut controller = state.lock().expect("controller mutex poisoned");
    match stop_locked(&mut controller) {
        Ok(session) => success_response(session),
        Err((status, error)) => error_response(status, error),
    }
}

fn stop_locked(controller: &mut Controller) -> Result<Session, (StatusCode, String)> {
    if controller.session.state != "recording" {
        return Err((StatusCode::CONFLICT, "no recording is active".into()));
    }
    let Some(recorder) = controller.recorder.as_mut() else {
        return Err((
            StatusCode::SERVICE_UNAVAILABLE,
            "libobs recorder is not initialized".into(),
        ));
    };
    match recorder.stop() {
        Ok((path, validation)) => {
            controller.session.state = "idle";
            controller.session.elapsed_seconds = validation.duration_seconds;
            controller.session.media_elapsed_seconds = validation.media_seconds;
            controller.session.wall_elapsed_seconds = validation.wall_seconds;
            controller.session.output_path = Some(path.to_string_lossy().into_owned());
            controller.session.validation = Some(validation);
            controller.session.error = None;
            Ok(session_snapshot(controller))
        }
        Err(error) => {
            controller.session.state = "failed";
            controller.session.error = Some(error.clone());
            Err((StatusCode::INTERNAL_SERVER_ERROR, error))
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
    if settings.capture_mode == "window" {
        let valid = settings.window_id.as_deref().is_some_and(|id| {
            controller
                .recorder
                .as_ref()
                .is_some_and(|recorder| recorder.windows().iter().any(|item| item.id == id))
        });
        if !valid {
            return error_response(
                StatusCode::UNPROCESSABLE_ENTITY,
                "window_id is not in the current target list",
            );
        }
    }
    if settings.record_microphone {
        let valid = controller.recorder.as_ref().is_some_and(|recorder| {
            recorder
                .audio_devices()
                .iter()
                .any(|item| item.kind == "input" && item.id == settings.mic_device_id)
        });
        if !valid {
            return error_response(
                StatusCode::UNPROCESSABLE_ENTITY,
                "selected microphone device is unavailable",
            );
        }
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
    let output = Command::new(tools::resolve_ffprobe())
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

    #[test]
    fn recording_session_uses_media_time_as_primary_elapsed() {
        let mut session = Session {
            state: "recording",
            ..Session::default()
        };
        apply_elapsed(&mut session, 1219.3, 1206.0);
        assert_eq!(session.elapsed_seconds, 1219.3);
        assert_eq!(session.media_elapsed_seconds, 1219.3);
        assert_eq!(session.wall_elapsed_seconds, 1206.0);
    }
}
