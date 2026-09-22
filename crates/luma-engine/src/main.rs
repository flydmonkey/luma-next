use std::net::{IpAddr, SocketAddr};

use clap::Parser;

#[derive(Debug, Parser)]
#[command(name = "luma-engine", about = "Luma Next local HTTP engine")]
struct Args {
    #[arg(long, default_value = "127.0.0.1")]
    bind: IpAddr,

    #[arg(long, default_value_t = 18765)]
    port: u16,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    eprintln!(
        "Initializing embedded libobs from {}",
        env!("LUMA_OBS_RUNDIR")
    );
    let recorder = luma_engine::ObsRecorder::initialize()
        .map_err(|error| format!("failed to initialize embedded libobs: {error}"))?;
    let address = SocketAddr::new(args.bind, args.port);
    let listener = tokio::net::TcpListener::bind(address).await?;
    eprintln!("Luma Next engine listening on http://{address}");
    axum::serve(listener, luma_engine::app_with_recorder(recorder))
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
}
