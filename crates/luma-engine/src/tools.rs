use std::path::{Path, PathBuf};

pub(crate) fn resolve_ffprobe() -> PathBuf {
    resolve_tool("LUMA_FFPROBE", "ffprobe.exe")
}

#[allow(dead_code)]
pub(crate) fn resolve_ffmpeg() -> PathBuf {
    resolve_tool("LUMA_FFMPEG", "ffmpeg.exe")
}

fn resolve_tool(environment: &str, file_name: &str) -> PathBuf {
    if let Some(path) = std::env::var_os(environment).filter(|value| !value.is_empty()) {
        return PathBuf::from(path);
    }
    let executable = std::env::current_exe().ok();
    resolve_tool_from(executable.as_deref(), file_name)
}

fn resolve_tool_from(executable: Option<&Path>, file_name: &str) -> PathBuf {
    if let Some(directory) = executable.and_then(Path::parent) {
        let adjacent = directory.join(file_name);
        if adjacent.is_file() {
            return adjacent;
        }
        if let Some(root) = directory.parent() {
            let nested = root.join("ffmpeg").join("bin").join(file_name);
            if nested.is_file() {
                return nested;
            }
        }
    }
    PathBuf::from(file_name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adjacent_tool_wins_over_nested_bundle() {
        let directory = tempfile::tempdir().unwrap();
        let bin = directory.path().join("bin");
        std::fs::create_dir_all(&bin).unwrap();
        let engine = bin.join("luma-engine.exe");
        let adjacent = bin.join("ffprobe.exe");
        std::fs::write(&adjacent, []).unwrap();
        assert_eq!(resolve_tool_from(Some(&engine), "ffprobe.exe"), adjacent);
    }

    #[test]
    fn unresolved_tool_falls_back_to_path_name() {
        assert_eq!(
            resolve_tool_from(None, "ffprobe.exe"),
            PathBuf::from("ffprobe.exe")
        );
    }
}
