use std::{
    env, fs,
    path::{Path, PathBuf},
};

fn main() {
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    let obs_source = env::var_os("LUMA_OBS_SOURCE")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\Users\Administrator\Projects\obs-studio"));
    let obs_rundir = env::var_os("LUMA_OBS_RUNDIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| obs_source.join(r"build_x64\rundir\RelWithDebInfo"));
    let obs_lib = obs_source.join(r"build_x64\libobs\RelWithDebInfo");
    let include = obs_source.join("libobs");

    for required in [&include, &obs_lib, &obs_rundir.join(r"bin\64bit")] {
        assert!(
            required.exists(),
            "OBS build artifact missing: {}",
            required.display()
        );
    }

    println!("cargo:rerun-if-env-changed=LUMA_OBS_SOURCE");
    println!("cargo:rerun-if-env-changed=LUMA_OBS_RUNDIR");
    println!("cargo:rerun-if-changed=native/obs_bridge.c");
    println!("cargo:rustc-link-search=native={}", obs_lib.display());
    println!("cargo:rustc-link-lib=obs");
    println!("cargo:rustc-link-lib=user32");
    println!("cargo:rustc-env=LUMA_OBS_RUNDIR={}", obs_rundir.display());

    cc::Build::new()
        .file("native/obs_bridge.c")
        .include(include)
        .include(obs_source.join(r"build_x64\config"))
        .warnings(true)
        .compile("luma_obs_bridge");

    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR"));
    let profile_dir = out_dir
        .ancestors()
        .nth(3)
        .expect("Cargo profile directory")
        .to_path_buf();
    copy_runtime(&obs_rundir.join(r"bin\64bit"), &profile_dir);
    copy_runtime(&obs_rundir.join(r"bin\64bit"), &profile_dir.join("deps"));
}

fn copy_runtime(source: &Path, destination: &Path) {
    fs::create_dir_all(destination).expect("create Cargo runtime directory");
    for entry in fs::read_dir(source).expect("read OBS runtime directory") {
        let entry = entry.expect("OBS runtime entry");
        let path = entry.path();
        let extension = path.extension().and_then(|value| value.to_str());
        if !matches!(extension, Some("dll" | "exe")) {
            continue;
        }
        let target = destination.join(entry.file_name());
        let should_copy = match (fs::metadata(&path), fs::metadata(&target)) {
            (Ok(source_meta), Ok(target_meta)) => source_meta.len() != target_meta.len(),
            (Ok(_), Err(_)) => true,
            _ => false,
        };
        if should_copy {
            fs::copy(&path, &target).unwrap_or_else(|error| {
                panic!("copy {} to {}: {error}", path.display(), target.display())
            });
        }
    }
}
