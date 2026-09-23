use std::{
    ffi::{CStr, CString, c_char, c_void},
    path::{Path, PathBuf},
    process::Command,
    ptr::NonNull,
    time::{SystemTime, UNIX_EPOCH},
};

use serde::Serialize;
use serde_json::Value;

use crate::tools;

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
        requested: *const c_char,
        mode: *const c_char,
        target: *const c_char,
        quality: *const c_char,
        display_width: u32,
        display_height: u32,
        region_x: i32,
        region_y: i32,
        region_width: u32,
        region_height: u32,
        system_audio: bool,
        microphone: bool,
        mic_device: *const c_char,
        active: *mut c_char,
        active_size: usize,
        fallback: *mut c_char,
        fallback_size: usize,
        error: *mut c_char,
        error_size: usize,
    ) -> bool;
    fn luma_obs_encoder_available(context: *mut c_void, id: *const c_char) -> bool;
    fn luma_obs_encoder_unavailable_reason(
        context: *mut c_void,
        id: *const c_char,
        buffer: *mut c_char,
        buffer_size: usize,
    ) -> usize;
    fn luma_obs_list_property(
        context: *mut c_void,
        source_id: *const c_char,
        property_name: *const c_char,
        buffer: *mut c_char,
        buffer_size: usize,
    ) -> usize;
    fn luma_obs_list_displays(
        context: *mut c_void,
        buffer: *mut c_char,
        buffer_size: usize,
    ) -> usize;
    fn luma_obs_pause(
        context: *mut c_void,
        pause: bool,
        error: *mut c_char,
        error_size: usize,
    ) -> bool;
    fn luma_obs_stop(
        context: *mut c_void,
        encoded_frames: *mut u32,
        total_bytes: *mut u64,
        media_seconds: *mut f64,
        wall_seconds: *mut f64,
        error: *mut c_char,
        error_size: usize,
    ) -> bool;
    fn luma_obs_shutdown(context: *mut c_void);
    fn luma_obs_media_seconds(context: *mut c_void) -> f64;
    fn luma_obs_wall_seconds(context: *mut c_void) -> f64;
}

#[derive(Debug, Clone, Serialize)]
pub struct RecordingValidation {
    pub video_stream: bool,
    pub audio_stream: bool,
    pub video_width: Option<u64>,
    pub video_height: Option<u64>,
    pub duration_seconds: f64,
    pub wall_seconds: f64,
    pub media_seconds: f64,
    pub media_ffprobe_delta_seconds: f64,
    pub wall_media_delta_seconds: f64,
    pub encoded_frames: u32,
    pub bytes: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct EncoderInfo {
    pub id: String,
    pub name: String,
    pub available: bool,
    pub hardware: bool,
    pub vendor: Option<String>,
    pub unavailable_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CaptureTarget {
    pub id: String,
    pub title: String,
    pub executable: String,
    pub minimized: bool,
    pub available: bool,
    pub unavailable_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AudioDeviceInfo {
    pub id: String,
    pub name: String,
    pub kind: &'static str,
    pub is_default: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct DisplayTarget {
    pub id: String,
    pub obs_id: String,
    pub name: String,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub primary: bool,
    pub available: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct CaptureRegion {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

pub struct CaptureOptions<'a> {
    pub mode: &'a str,
    pub window_id: Option<&'a str>,
    pub game_id: Option<&'a str>,
    pub display: Option<&'a DisplayTarget>,
    pub region: Option<CaptureRegion>,
    pub system_audio: bool,
    pub microphone: bool,
    pub mic_device_id: Option<&'a str>,
    pub quality: &'a str,
}

#[derive(Debug)]
pub struct RecordingStart {
    pub path: PathBuf,
    pub active_encoder: String,
    pub fallback_reason: Option<String>,
}

pub struct ObsRecorder {
    context: NonNull<c_void>,
    active: Option<ActiveRecording>,
}

struct ActiveRecording {
    path: PathBuf,
    audio_only: bool,
}

// libobs owns its worker threads; access to this handle is serialized by the engine mutex.
unsafe impl Send for ObsRecorder {}

impl ObsRecorder {
    pub fn initialize() -> Result<Self, String> {
        let root = resolve_obs_runtime()?;
        eprintln!("Using OBS runtime from {}", root.display());
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
        let recorder = NonNull::new(context)
            .map(|context| Self {
                context,
                active: None,
            })
            .ok_or_else(|| read_error(&error))?;
        for encoder in recorder.encoders().into_iter().filter(|item| item.hardware) {
            if encoder.available {
                eprintln!("Hardware encoder {} available", encoder.id);
            } else {
                eprintln!(
                    "Hardware encoder {} unavailable: {}",
                    encoder.id,
                    encoder
                        .unavailable_reason
                        .as_deref()
                        .unwrap_or("unknown reason")
                );
            }
        }
        Ok(recorder)
    }

    pub fn encoders(&self) -> Vec<EncoderInfo> {
        encoder_candidates()
            .into_iter()
            .map(|(id, name, hardware, vendor)| {
                let id_string = CString::new(id).expect("static encoder id");
                let available = unsafe {
                    luma_obs_encoder_available(self.context.as_ptr(), id_string.as_ptr())
                };
                let unavailable_reason = if available {
                    None
                } else {
                    let mut buffer = error_buffer();
                    unsafe {
                        luma_obs_encoder_unavailable_reason(
                            self.context.as_ptr(),
                            id_string.as_ptr(),
                            buffer.as_mut_ptr(),
                            buffer.len(),
                        )
                    };
                    Some(read_error(&buffer))
                };
                EncoderInfo {
                    id: id.into(),
                    name: name.into(),
                    available,
                    hardware,
                    vendor: vendor.map(str::to_string),
                    unavailable_reason,
                }
            })
            .collect()
    }

    pub fn windows(&self) -> Vec<CaptureTarget> {
        self.property_items("window_capture", "window")
            .into_iter()
            .map(|(id, label)| {
                let (executable, title) = label
                    .strip_prefix('[')
                    .and_then(|value| value.split_once("]: "))
                    .unwrap_or(("unknown", label.as_str()));
                CaptureTarget {
                    id,
                    title: title.to_string(),
                    executable: executable.to_string(),
                    minimized: false,
                    available: true,
                    unavailable_reason: None,
                }
            })
            .collect()
    }

    pub fn games(&self) -> Vec<CaptureTarget> {
        self.property_items("game_capture", "window")
            .into_iter()
            .map(|(id, label)| {
                let (executable, title) = label
                    .strip_prefix('[')
                    .and_then(|value| value.split_once("]: "))
                    .unwrap_or(("unknown", label.as_str()));
                CaptureTarget {
                    id,
                    title: title.to_string(),
                    executable: executable.to_string(),
                    minimized: false,
                    available: true,
                    unavailable_reason: None,
                }
            })
            .collect()
    }

    pub fn displays(&self) -> Vec<DisplayTarget> {
        let mut buffer = vec![0_i8; 64 * 1024];
        let written = unsafe {
            luma_obs_list_displays(self.context.as_ptr(), buffer.as_mut_ptr(), buffer.len())
        };
        let bytes = unsafe { std::slice::from_raw_parts(buffer.as_ptr().cast::<u8>(), written) };
        parse_displays(&String::from_utf8_lossy(bytes))
    }

    pub fn audio_devices(&self) -> Vec<AudioDeviceInfo> {
        [
            ("wasapi_output_capture", "output"),
            ("wasapi_input_capture", "input"),
        ]
        .into_iter()
        .flat_map(|(source, kind)| {
            self.property_items(source, "device_id")
                .into_iter()
                .map(move |(id, name)| AudioDeviceInfo {
                    is_default: id == "default",
                    id,
                    name,
                    kind,
                })
        })
        .collect()
    }

    fn property_items(&self, source: &str, property: &str) -> Vec<(String, String)> {
        let source = CString::new(source).expect("static source id");
        let property = CString::new(property).expect("static property id");
        let mut buffer = vec![0_i8; 128 * 1024];
        let written = unsafe {
            luma_obs_list_property(
                self.context.as_ptr(),
                source.as_ptr(),
                property.as_ptr(),
                buffer.as_mut_ptr(),
                buffer.len(),
            )
        };
        let bytes = unsafe { std::slice::from_raw_parts(buffer.as_ptr().cast::<u8>(), written) };
        String::from_utf8_lossy(bytes)
            .lines()
            .filter_map(|line| line.split_once('\t'))
            .map(|(id, name)| (id.to_string(), name.to_string()))
            .collect()
    }

    pub fn start(
        &mut self,
        directory: &Path,
        encoder: &str,
        capture: CaptureOptions<'_>,
    ) -> Result<RecordingStart, String> {
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
        let c_encoder = CString::new(encoder).map_err(|_| "invalid encoder id".to_string())?;
        let c_mode = CString::new(capture.mode).map_err(|_| "invalid capture mode".to_string())?;
        let target = if capture.mode == "window" {
            capture.window_id.unwrap_or_default()
        } else if capture.mode == "game" {
            capture.game_id.unwrap_or_default()
        } else {
            capture
                .display
                .map_or("", |display| display.obs_id.as_str())
        };
        let c_target = CString::new(target).map_err(|_| "invalid window id".to_string())?;
        let c_quality = CString::new(capture.quality).map_err(|_| "invalid quality".to_string())?;
        let c_mic_device = CString::new(capture.mic_device_id.unwrap_or("default"))
            .map_err(|_| "invalid microphone device id".to_string())?;
        let mut active = error_buffer();
        let mut fallback = error_buffer();
        let mut error = error_buffer();
        let started = unsafe {
            luma_obs_start(
                self.context.as_ptr(),
                c_path.as_ptr(),
                c_encoder.as_ptr(),
                c_mode.as_ptr(),
                c_target.as_ptr(),
                c_quality.as_ptr(),
                capture.display.map_or(0, |display| display.width),
                capture.display.map_or(0, |display| display.height),
                capture.region.map_or(0, |region| region.x),
                capture.region.map_or(0, |region| region.y),
                capture.region.map_or(0, |region| region.width),
                capture.region.map_or(0, |region| region.height),
                capture.system_audio,
                capture.microphone,
                c_mic_device.as_ptr(),
                active.as_mut_ptr(),
                active.len(),
                fallback.as_mut_ptr(),
                fallback.len(),
                error.as_mut_ptr(),
                error.len(),
            )
        };
        if !started {
            return Err(read_error(&error));
        }
        self.active = Some(ActiveRecording {
            path: path.clone(),
            audio_only: capture.mode == "audio_only",
        });
        let fallback_reason = read_error(&fallback);
        Ok(RecordingStart {
            path,
            active_encoder: read_error(&active),
            fallback_reason: (!fallback_reason.is_empty()).then_some(fallback_reason),
        })
    }

    pub fn pause(&mut self, pause: bool) -> Result<(), String> {
        if self.active.is_none() {
            return Err("no recording is active".into());
        }
        let mut error = error_buffer();
        if unsafe {
            luma_obs_pause(
                self.context.as_ptr(),
                pause,
                error.as_mut_ptr(),
                error.len(),
            )
        } {
            Ok(())
        } else {
            Err(read_error(&error))
        }
    }

    pub fn stop(&mut self) -> Result<(PathBuf, RecordingValidation), String> {
        let active = self
            .active
            .take()
            .ok_or_else(|| "no recording is active".to_string())?;
        let mut encoded_frames = 0;
        let mut reported_bytes = 0;
        let mut media_seconds = 0.0;
        let mut wall_seconds = 0.0;
        let mut error = error_buffer();
        let stopped = unsafe {
            luma_obs_stop(
                self.context.as_ptr(),
                &mut encoded_frames,
                &mut reported_bytes,
                &mut media_seconds,
                &mut wall_seconds,
                error.as_mut_ptr(),
                error.len(),
            )
        };
        if !stopped {
            return Err(read_error(&error));
        }
        let validation = validate_recording(
            &active.path,
            media_seconds,
            wall_seconds,
            encoded_frames,
            reported_bytes,
            active.audio_only,
        )?;
        Ok((active.path, validation))
    }

    pub fn elapsed(&self) -> (f64, f64) {
        if self.active.is_none() {
            return (0.0, 0.0);
        }
        unsafe {
            (
                luma_obs_media_seconds(self.context.as_ptr()),
                luma_obs_wall_seconds(self.context.as_ptr()),
            )
        }
    }
}

fn parse_displays(value: &str) -> Vec<DisplayTarget> {
    value
        .lines()
        .enumerate()
        .filter_map(|(index, line)| {
            let fields: Vec<_> = line.split('\t').collect();
            if fields.len() != 7 {
                return None;
            }
            Some(DisplayTarget {
                id: format!("display-{index}"),
                obs_id: fields[0].to_string(),
                name: fields[1].to_string(),
                x: fields[2].parse().ok()?,
                y: fields[3].parse().ok()?,
                width: fields[4].parse().ok()?,
                height: fields[5].parse().ok()?,
                primary: fields[6] == "1",
                available: true,
            })
        })
        .collect()
}

fn resolve_obs_runtime() -> Result<PathBuf, String> {
    let root = if let Some(override_path) = std::env::var_os("LUMA_OBS_RUNDIR") {
        PathBuf::from(override_path)
    } else {
        let executable = std::env::current_exe()
            .map_err(|error| format!("failed to locate luma-engine executable: {error}"))?;
        let bundled = executable
            .parent()
            .and_then(Path::parent)
            .map(|install_root| install_root.join("obs"));
        match bundled.filter(|path| is_obs_runtime(path)) {
            Some(path) => path,
            None => PathBuf::from(env!("LUMA_OBS_RUNDIR")),
        }
    };
    if !is_obs_runtime(&root) {
        return Err(format!(
            "OBS runtime is incomplete at {} (expected data/libobs and obs-plugins/64bit)",
            root.display()
        ));
    }
    Ok(root)
}

fn is_obs_runtime(root: &Path) -> bool {
    root.join("data").join("libobs").is_dir() && root.join("obs-plugins").join("64bit").is_dir()
}

fn encoder_candidates() -> [(&'static str, &'static str, bool, Option<&'static str>); 6] {
    [
        ("obs_x264", "Software (x264)", false, None),
        ("h264_texture_amf", "AMD HW H.264 (AMF)", true, Some("AMD")),
        ("jim_nvenc", "NVIDIA NVENC H.264", true, Some("NVIDIA")),
        (
            "ffmpeg_nvenc",
            "NVIDIA NVENC H.264 (FFmpeg)",
            true,
            Some("NVIDIA"),
        ),
        (
            "obs_qsv11_v2",
            "Intel Quick Sync H.264",
            true,
            Some("Intel"),
        ),
        (
            "obs_qsv11",
            "Intel Quick Sync H.264 (deprecated id)",
            true,
            Some("Intel"),
        ),
    ]
}

impl Drop for ObsRecorder {
    fn drop(&mut self) {
        unsafe { luma_obs_shutdown(self.context.as_ptr()) };
    }
}

fn validate_recording(
    path: &Path,
    media_seconds: f64,
    wall_seconds: f64,
    encoded_frames: u32,
    reported_bytes: u64,
    audio_only: bool,
) -> Result<RecordingValidation, String> {
    let bytes = std::fs::metadata(path)
        .map_err(|error| {
            format!(
                "OBS stopped but {} is not readable: {error}",
                path.display()
            )
        })?
        .len();
    if bytes == 0 || (!audio_only && encoded_frames == 0) {
        return Err(format!(
            "OBS produced no usable video frames (frames={encoded_frames}, bytes={bytes}, reported_bytes={reported_bytes})"
        ));
    }

    let ffprobe = tools::resolve_ffprobe();
    let output = Command::new(&ffprobe)
        .args([
            "-v",
            "error",
            "-show_entries",
            "stream=codec_type,width,height:format=duration",
            "-of",
            "json",
        ])
        .arg(path)
        .output()
        .map_err(|error| {
            format!(
                "ffprobe is required to validate recordings (tried {}): {error}. Use the official Luma Next package or install FFmpeg",
                ffprobe.display()
            )
        })?;
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
    let video = streams
        .iter()
        .find(|stream| stream["codec_type"] == "video");
    let video_width = video.and_then(|stream| stream["width"].as_u64());
    let video_height = video.and_then(|stream| stream["height"].as_u64());
    let duration_seconds = probe["format"]["duration"]
        .as_str()
        .and_then(|value| value.parse::<f64>().ok())
        .ok_or_else(|| "ffprobe returned no valid container duration".to_string())?;
    let ffprobe_tolerance = 1.5;
    let clock_tolerance = (wall_seconds * 0.005).max(1.5);
    if !audio_stream || (!audio_only && !video_stream) {
        return Err(format!(
            "recording is missing required streams (video={video_stream}, audio={audio_stream})"
        ));
    }
    let media_ffprobe_delta_seconds = (duration_seconds - media_seconds).abs();
    let wall_media_delta_seconds = (media_seconds - wall_seconds).abs();
    if duration_seconds < 1.0 || media_ffprobe_delta_seconds > ffprobe_tolerance {
        return Err(format!(
            "recording duration disagrees with OBS output frames (ffprobe={duration_seconds:.3}s, OBS media={media_seconds:.3}s, tolerance={ffprobe_tolerance:.3}s)"
        ));
    }
    if wall_media_delta_seconds > clock_tolerance {
        return Err(format!(
            "OBS media time diverged from output wall clock (media={media_seconds:.3}s, wall={wall_seconds:.3}s, tolerance={clock_tolerance:.3}s)"
        ));
    }
    Ok(RecordingValidation {
        video_stream,
        audio_stream,
        video_width,
        video_height,
        duration_seconds,
        wall_seconds,
        media_seconds,
        media_ffprobe_delta_seconds,
        wall_media_delta_seconds,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_runtime_requires_data_and_plugin_directories() {
        let directory = tempfile::tempdir().unwrap();
        assert!(!is_obs_runtime(directory.path()));
        std::fs::create_dir_all(directory.path().join("data/libobs")).unwrap();
        std::fs::create_dir_all(directory.path().join("obs-plugins/64bit")).unwrap();
        assert!(is_obs_runtime(directory.path()));
    }

    #[test]
    fn display_paths_with_url_metacharacters_get_stable_ui_ids() {
        let displays = parse_displays(
            "\\\\?\\DISPLAY#DEL4241#5&b18619&0&UID264#{guid}\tDell P2722H\t0\t0\t1920\t1080\t1\n",
        );
        assert_eq!(displays.len(), 1);
        assert_eq!(displays[0].id, "display-0");
        assert_eq!(
            displays[0].obs_id,
            "\\\\?\\DISPLAY#DEL4241#5&b18619&0&UID264#{guid}"
        );
        assert!(displays[0].primary);
    }
}
