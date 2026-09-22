#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(not(target_os = "windows"))]
compile_error!("luma-shell is Windows-first and currently requires Windows/WebView2.");

use std::{process::Command, thread, time::Duration};

use tao::{
    dpi::LogicalSize,
    event::{Event, StartCause, WindowEvent},
    event_loop::{ControlFlow, EventLoopBuilder},
    platform::windows::{WindowBuilderExtWindows, WindowExtWindows},
    window::WindowBuilder,
};
use windows_sys::Win32::Graphics::Dwm::DwmSetWindowAttribute;
use wry::{WebView, WebViewBuilder, http::Request};

const ENGINE_ORIGIN: &str = "http://127.0.0.1:18765";
const SHELL_HTML: &str = include_str!("shell.html");

#[derive(Debug)]
enum ShellEvent {
    Minimize,
    ToggleMaximize,
    Close,
    Drag,
    OpenRecordings,
    Retry,
    EngineProbe(bool),
}

fn main() -> wry::Result<()> {
    let event_loop = EventLoopBuilder::<ShellEvent>::with_user_event().build();
    let proxy = event_loop.create_proxy();
    let window = WindowBuilder::new()
        .with_title("Luma Next")
        .with_inner_size(LogicalSize::new(980.0, 700.0))
        .with_min_inner_size(LogicalSize::new(720.0, 520.0))
        .with_decorations(false)
        .with_undecorated_shadow(true)
        .build(&event_loop)
        .expect("failed to create the Luma window");

    request_rounded_corners(window.hwnd());

    let ipc_proxy = proxy.clone();
    let webview = WebViewBuilder::new()
        .with_html(SHELL_HTML)
        .with_devtools(cfg!(debug_assertions))
        .with_hotkeys_zoom(false)
        .with_navigation_handler(is_trusted_navigation)
        .with_ipc_handler(move |request: Request<String>| {
            if let Some(event) = parse_command(request.body()) {
                let _ = ipc_proxy.send_event(event);
            }
        })
        .build(&window)?;

    probe_engine(proxy.clone());

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;
        match event {
            Event::NewEvents(StartCause::Init) => window.set_visible(true),
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => *control_flow = ControlFlow::Exit,
            Event::UserEvent(command) => match command {
                ShellEvent::Minimize => window.set_minimized(true),
                ShellEvent::ToggleMaximize => {
                    window.set_maximized(!window.is_maximized());
                    sync_maximize_icon(&webview, window.is_maximized());
                }
                ShellEvent::Close => *control_flow = ControlFlow::Exit,
                ShellEvent::Drag => {
                    let _ = window.drag_window();
                }
                ShellEvent::OpenRecordings => open_recordings_directory(),
                ShellEvent::Retry => probe_engine(proxy.clone()),
                ShellEvent::EngineProbe(ready) => sync_engine_state(&webview, ready),
            },
            _ => {}
        }
    });
}

fn parse_command(command: &str) -> Option<ShellEvent> {
    match command {
        "window:minimize" => Some(ShellEvent::Minimize),
        "window:toggle-maximize" => Some(ShellEvent::ToggleMaximize),
        "window:close" => Some(ShellEvent::Close),
        "window:drag" => Some(ShellEvent::Drag),
        "shell:open-recordings" => Some(ShellEvent::OpenRecordings),
        "shell:retry" => Some(ShellEvent::Retry),
        _ => None,
    }
}

fn is_trusted_navigation(url: String) -> bool {
    url == "about:blank"
        || url.starts_with("data:text/html")
        || url == ENGINE_ORIGIN
        || url.starts_with(&format!("{ENGINE_ORIGIN}/"))
}

fn probe_engine(proxy: tao::event_loop::EventLoopProxy<ShellEvent>) {
    thread::spawn(move || {
        let ready = std::net::TcpStream::connect_timeout(
            &"127.0.0.1:18765".parse().expect("static socket address"),
            Duration::from_millis(650),
        )
        .is_ok();
        let _ = proxy.send_event(ShellEvent::EngineProbe(ready));
    });
}

fn sync_engine_state(webview: &WebView, ready: bool) {
    let script = format!("window.lumaShell.setEngineReady({ready}, {ENGINE_ORIGIN:?})");
    let _ = webview.evaluate_script(&script);
}

fn sync_maximize_icon(webview: &WebView, maximized: bool) {
    let script = format!("window.lumaShell.setMaximized({maximized})");
    let _ = webview.evaluate_script(&script);
}

fn open_recordings_directory() {
    let path = std::env::var_os("USERPROFILE")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("Videos")
        .join("Luma");
    if std::fs::create_dir_all(&path).is_ok() {
        let _ = Command::new("explorer.exe").arg(path).spawn();
    }
}

fn request_rounded_corners(hwnd: isize) {
    // Windows 11 honors this as a preference. Windows 10 safely ignores it.
    const DWMWA_WINDOW_CORNER_PREFERENCE: u32 = 33;
    const DWMWCP_ROUND: u32 = 2;
    unsafe {
        let _ = DwmSetWindowAttribute(
            hwnd as *mut _,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            &DWMWCP_ROUND as *const u32 as *const _,
            size_of::<u32>() as u32,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_local_engine_navigation_is_allowed() {
        assert!(is_trusted_navigation(format!("{ENGINE_ORIGIN}/settings")));
        assert!(!is_trusted_navigation("https://example.com/".into()));
        assert!(!is_trusted_navigation("http://127.0.0.1:18766/".into()));
    }

    #[test]
    fn unknown_ipc_is_ignored() {
        assert!(parse_command("window:close").is_some());
        assert!(parse_command("window:destroy-everything").is_none());
    }
}
