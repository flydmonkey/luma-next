#[cfg(target_os = "windows")]
unsafe extern "C" {
    fn luma_game_probe_run() -> i32;
}

fn main() {
    #[cfg(target_os = "windows")]
    std::process::exit(unsafe { luma_game_probe_run() });
    #[cfg(not(target_os = "windows"))]
    eprintln!("The Luma game-capture probe is Windows-only.");
}
