use std::{
    net::{IpAddr, SocketAddr},
    thread,
    time::Duration,
};

use clap::Parser;
use luma_engine::Engine;
use tokio::sync::watch;
use tray_icon::{
    Icon, TrayIconBuilder,
    menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem},
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, MSG, PM_REMOVE, PeekMessageW, TranslateMessage, WM_QUIT,
};

mod windows_host;

#[derive(Debug, Parser)]
#[command(name = "luma-engine", about = "Luma Next local HTTP engine")]
struct Args {
    #[arg(long, default_value = "127.0.0.1")]
    bind: IpAddr,
    #[arg(long, default_value_t = 18765)]
    port: u16,
    #[arg(long, help = "Disable the Windows tray icon for CI or service use")]
    no_tray: bool,
    #[arg(
        long,
        help = "Open the control page in the default browser after startup"
    )]
    open_ui: bool,
    #[arg(
        long,
        help = "Allow another engine instance for isolated development ports"
    )]
    allow_second_instance: bool,
    #[arg(
        long,
        conflicts_with = "uninstall_autostart",
        help = "Install per-user Windows logon autostart for this executable"
    )]
    install_autostart: bool,
    #[arg(
        long,
        conflicts_with = "install_autostart",
        help = "Remove the per-user Windows logon autostart entry"
    )]
    uninstall_autostart: bool,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    if args.install_autostart {
        let executable = std::env::current_exe()?;
        let command = windows_host::install_autostart(&executable)
            .map_err(|error| format!("安装开机启动失败：{error}"))?;
        println!(
            "已安装当前用户开机启动：HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Run\\LumaNext"
        );
        println!("命令：{command}");
        return Ok(());
    }
    if args.uninstall_autostart {
        let removed = windows_host::uninstall_autostart()
            .map_err(|error| format!("卸载开机启动失败：{error}"))?;
        println!(
            "{}",
            if removed {
                "已移除当前用户开机启动。"
            } else {
                "开机启动项原本就不存在。"
            }
        );
        return Ok(());
    }

    let _instance_guard = if args.allow_second_instance {
        None
    } else {
        match windows_host::acquire_single_instance()? {
            windows_host::InstanceGuard::Acquired(guard) => Some(guard),
            windows_host::InstanceGuard::AlreadyRunning => {
                eprintln!("Luma Next 已在当前 Windows 会话中运行；本次启动已取消。");
                std::process::exit(2);
            }
        }
    };
    eprintln!(
        "Initializing embedded libobs from {}",
        env!("LUMA_OBS_RUNDIR")
    );
    let recorder = luma_engine::ObsRecorder::initialize()
        .map_err(|error| format!("failed to initialize embedded libobs: {error}"))?;
    let engine = Engine::new(recorder);
    let address = SocketAddr::new(args.bind, args.port);
    let listener = tokio::net::TcpListener::bind(address)
        .await
        .map_err(|error| format!("failed to bind http://{address}: {error}"))?;
    let control_url = format!("http://127.0.0.1:{}/", args.port);
    eprintln!("Luma Next engine listening on http://{address}");

    if args.open_ui {
        open::that(&control_url)
            .map_err(|error| format!("failed to open control page: {error}"))?;
    }

    let (exit_tx, mut exit_rx) = watch::channel(false);
    let keep_exit_channel_alive = exit_tx.clone();
    let _tray_thread = (!args.no_tray)
        .then(|| {
            let engine = engine.clone();
            let control_url = control_url.clone();
            thread::Builder::new()
                .name("luma-tray".into())
                .spawn(move || run_tray(engine, control_url, exit_tx))
        })
        .transpose()?;

    axum::serve(listener, engine.router())
        .with_graceful_shutdown(async move {
            let _keep_exit_channel_alive = keep_exit_channel_alive;
            tokio::select! {
                _ = shutdown_signal() => {},
                _ = exit_rx.changed() => {},
            }
        })
        .await?;
    Ok(())
}

fn run_tray(engine: Engine, control_url: String, exit_tx: watch::Sender<bool>) {
    let menu = Menu::new();
    let open_item = MenuItem::new("打开控制页", true, None);
    let action_item = MenuItem::new("开始录制", true, None);
    let status_item = MenuItem::new("状态：空闲", false, None);
    let quit_item = MenuItem::new("退出", true, None);
    let separator = PredefinedMenuItem::separator();
    if let Err(error) = menu.append_items(&[
        &open_item,
        &action_item,
        &status_item,
        &separator,
        &quit_item,
    ]) {
        eprintln!("failed to build tray menu: {error}");
        return;
    }
    let icon = make_icon(false);
    let tray = match TrayIconBuilder::new()
        .with_tooltip("Luma Next · 空闲")
        .with_icon(icon)
        .with_menu(Box::new(menu))
        .build()
    {
        Ok(tray) => tray,
        Err(error) => {
            eprintln!("failed to create system tray: {error}");
            return;
        }
    };
    let mut last_state = String::new();
    let mut quitting = false;
    loop {
        unsafe {
            let mut message: MSG = std::mem::zeroed();
            while PeekMessageW(&mut message, std::ptr::null_mut(), 0, 0, PM_REMOVE) != 0 {
                if message.message == WM_QUIT {
                    quitting = true;
                }
                TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
        while let Ok(event) = MenuEvent::receiver().try_recv() {
            if event.id == *open_item.id() {
                if let Err(error) = open::that(&control_url) {
                    eprintln!("failed to open control page: {error}");
                }
            } else if event.id == *action_item.id() {
                let engine = engine.clone();
                thread::spawn(move || {
                    let status = engine.tray_status();
                    let result = if status.state == "recording" {
                        engine.stop()
                    } else {
                        engine.start_default()
                    };
                    match result {
                        Ok(status) => eprintln!("tray action completed: {}", status.state),
                        Err(error) => eprintln!("tray recording action failed: {error}"),
                    }
                });
            } else if event.id == *quit_item.id() {
                quitting = true;
            }
        }
        let status = engine.tray_status();
        let state_key = format!(
            "{}:{:.0}:{}:{:?}",
            status.state,
            status.elapsed_seconds,
            status.output_path.as_deref().unwrap_or(""),
            status.error
        );
        if state_key != last_state {
            let recording = status.state == "recording";
            let action = if recording {
                "停止录制"
            } else {
                "开始录制"
            };
            let summary = status_summary(&status);
            action_item.set_text(action);
            status_item.set_text(format!("状态：{summary}"));
            let _ = tray.set_tooltip(Some(format!("Luma Next · {summary}")));
            let _ = tray.set_icon(Some(make_icon(recording)));
            eprintln!("tray menu refreshed: action={action}, {summary}");
            last_state = state_key;
        }
        if quitting {
            if engine.tray_status().state == "recording" {
                match engine.stop() {
                    Ok(status) => eprintln!(
                        "recording stopped before exit: {}",
                        status.output_path.unwrap_or_default()
                    ),
                    Err(error) => eprintln!(
                        "recording stop failed during exit; exiting with error preserved: {error}"
                    ),
                }
            }
            let _ = exit_tx.send(true);
            break;
        }
        thread::sleep(Duration::from_millis(200));
    }
}

fn status_summary(status: &luma_engine::TrayStatus) -> String {
    if status.state == "recording" {
        format!(
            "录制中 {:02}:{:02}:{:02} · {}",
            status.elapsed_seconds as u64 / 3600,
            status.elapsed_seconds as u64 % 3600 / 60,
            status.elapsed_seconds as u64 % 60,
            status.encoder_active.as_deref().unwrap_or("编码器未知")
        )
    } else if let Some(error) = &status.error {
        format!("错误 · {error}")
    } else if let Some(path) = &status.output_path {
        format!("空闲 · 已保存 {path}")
    } else {
        "空闲".into()
    }
}

fn make_icon(recording: bool) -> Icon {
    let size = 32usize;
    let mut rgba = vec![0u8; size * size * 4];
    let color = if recording {
        [196, 43, 28, 255]
    } else {
        [45, 45, 48, 255]
    };
    for y in 0..size {
        for x in 0..size {
            let dx = x as i32 - 15;
            let dy = y as i32 - 15;
            if dx * dx + dy * dy <= 12 * 12 {
                let index = (y * size + x) * 4;
                rgba[index..index + 4].copy_from_slice(&color);
            }
        }
    }
    Icon::from_rgba(rgba, size as u32, size as u32).expect("valid generated tray icon")
}

async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
}
