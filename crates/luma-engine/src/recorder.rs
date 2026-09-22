use std::{
    ffi::{CStr, CString, c_char, c_void},
    path::{Path, PathBuf},
    process::Command,
    ptr::NonNull,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use serde::Serialize;
use serde_json::Value;

const ERROR_CAPACITY: usize = 2048;

unsafe extern "C" {
    fn luma_obs_initialize(
        root: *const c_char,
        config_path: *const c_char,
        error: *mut c_char,
        error_size: usize,
    ) -> *mut c_void;
    fn luma_obs_start(
        context: *mut c_void,
        path: *const c_char,
        error: *mut c_char,
        error_size: usize,
    ) -> bool;
    fn luma_obs_stop(
        context: *mut c_void,
        encoded_frames: *mut u32,
        total_bytes: *mut u64,
        error: *mut c_char,
        error_size: usize,
    ) -> bool;
    fn luma_obs_shutdown(context: *mut c_void);
}

#[derive(Debug, Clone, Serialize)]
pub struct RecordingValidation {
    pub video_stream: bool,
    pub audio_stream: bool,
    pub duration_seconds: f64,
    pub wall_seconds: f64,
    pub encoded_frames: u32,
    pub bytes: u64,
}

pub struct ObsRecorder {
    context: NonNull<c_void>,
    active: Option<ActiveRecording>,
}

struct ActiveRecording {
    path: PathBuf,
    started: Instant,
}

// libobs owns its worker threads; access to this handle is serialized by the engine mutex.
unsafe impl Send for ObsRecorder {}

impl ObsRecorder {
    pub fn initialize() -> Result<Self, String> {
        let root = PathBuf::from(env!("LUMA_OBS_RUNDIR"));
        let config = std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir)
            .join("LumaNext")
            .join("obs-plugin-config");
        std::fs::create_dir_all(&config).map_err(|error| {
            format!(
                "failed to create OBS config directory {}: {error}",
                config.display()
            )
        })?;
        let root = path_to_cstring(&root)?;
        let config = path_to_cstring(&config)?;
        let mut error = error_buffer();
        let context = unsafe {
            luma_obs_initialize(
                root.as_ptr(),
                config.as_ptr(),
                error.as_mut_ptr(),
                error.len(),
            )
        };
        NonNull::new(context)
            .map(|context| Self {
                context,
                active: None,
            })
            .ok_or_else(|| read_error(&error))
    }

    pub fn start(&mut self, directory: &Path) -> Result<PathBuf, String> {
        if self.active.is_some() {
            return Err("recording is already active".into());
        }
        std::fs::create_dir_all(directory).map_err(|error| {
            format!(
                "failed to create recording directory {}: {error}",
                directory.display()
            )
        })?;
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| format!("system clock is before Unix epoch: {error}"))?
            .as_secs();
        let path = directory.join(format!("luma-{timestamp}.mkv"));
        let c_path = path_to_cstring(&path)?;
        let mut error = error_buffer();
        let started = unsafe {
            luma_obs_start(
                self.context.as_ptr(),
                c_path.as_ptr(),
                error.as_mut_ptr(),
                error.len(),
            )
        };
        if !started {
            return Err(read_error(&error));
        }
        self.active = Some(ActiveRecording {
            path: path.clone(),
            started: Instant::now(),
        });
        Ok(path)
    }

    pub fn stop(&mut self) -> Result<(PathBuf, RecordingValidation), String> {
        let active = self
            .active
            .take()
            .ok_or_else(|| "no recording is active".to_string())?;
        let mut encoded_frames = 0;
        let mut reported_bytes = 0;
        let mut error = error_buffer();
        let stopped = unsafe {
            luma_obs_stop(
                self.context.as_ptr(),
                &mut encoded_frames,
                &mut reported_bytes,
                error.as_mut_ptr(),
                error.len(),
            )
        };
        if !stopped {
            return Err(read_error(&error));
        }
        let wall = active.started.elapsed();
        let validation = validate_recording(&active.path, wall, encoded_frames, reported_bytes)?;
        Ok((active.path, validation))
    }
}

impl Drop for ObsRecorder {
    fn drop(&mut self) {
        unsafe { luma_obs_shutdown(self.context.as_ptr()) };
    }
}

fn validate_recording(
    path: &Path,
    wall: Duration,
    encoded_frames: u32,
    reported_bytes: u64,
) -> Result<RecordingValidation, String> {
    let bytes = std::fs::metadata(path)
        .map_err(|error| {
            format!(
                "OBS stopped but {} is not readable: {error}",
                path.display()
            )
        })?
        .len();
    if bytes == 0 || encoded_frames == 0 {
        return Err(format!(
            "OBS produced no usable video frames (frames={encoded_frames}, bytes={bytes}, reported_bytes={reported_bytes})"
        ));
    }

    let output = Command::new("ffprobe.exe")
        .args([
            "-v",
            "error",
            "-show_entries",
            "stream=codec_type:format=duration",
            "-of",
            "json",
        ])
        .arg(path)
        .output()
        .map_err(|error| format!("ffprobe is required to validate recordings: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "ffprobe rejected {}: {}",
            path.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let probe: Value = serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("invalid ffprobe JSON: {error}"))?;
    let streams = probe["streams"]
        .as_array()
        .ok_or_else(|| "ffprobe returned no stream list".to_string())?;
    let video_stream = streams.iter().any(|stream| stream["codec_type"] == "video");
    let audio_stream = streams.iter().any(|stream| stream["codec_type"] == "audio");
    let duration_seconds = probe["format"]["duration"]
        .as_str()
        .and_then(|value| value.parse::<f64>().ok())
        .ok_or_else(|| "ffprobe returned no valid container duration".to_string())?;
    let wall_seconds = wall.as_secs_f64();
    let tolerance = (wall_seconds * 0.35).max(3.0);
    if !video_stream || !audio_stream {
        return Err(format!(
            "recording is missing required streams (video={video_stream}, audio={audio_stream})"
        ));
    }
    if duration_seconds < 1.0 || (duration_seconds - wall_seconds).abs() > tolerance {
        return Err(format!(
            "recording duration is not credible (media={duration_seconds:.3}s, wall={wall_seconds:.3}s, tolerance={tolerance:.3}s)"
        ));
    }
    Ok(RecordingValidation {
        video_stream,
        audio_stream,
        duration_seconds,
        wall_seconds,
        encoded_frames,
        bytes,
    })
}

fn path_to_cstring(path: &Path) -> Result<CString, String> {
    CString::new(path.to_string_lossy().as_bytes())
        .map_err(|_| format!("path contains an embedded null: {}", path.display()))
}

fn error_buffer() -> Vec<c_char> {
    vec![0; ERROR_CAPACITY]
}

fn read_error(buffer: &[c_char]) -> String {
    unsafe { CStr::from_ptr(buffer.as_ptr()) }
        .to_string_lossy()
        .trim()
        .to_string()
}
