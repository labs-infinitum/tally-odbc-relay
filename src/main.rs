use std::net::SocketAddr;

use clap::Parser;
use tokio::net::TcpListener;

use tally_odbc_relay::server;

#[derive(Debug, Parser)]
#[command(
    name = "tally-odbc-relay",
    about = "Localhost HTTP relay for TallyPrime ODBC SQL queries."
)]
struct Args {
    /// Address the HTTP listener binds to.
    #[arg(long, env = "TALLY_ODBC_BIND", default_value = "127.0.0.1")]
    bind: String,

    /// HTTP port.
    #[arg(long, env = "TALLY_ODBC_PORT", default_value_t = 9001)]
    port: u16,

    /// Tally ODBC DSN.
    #[arg(long, env = "TALLY_ODBC_DSN", default_value = "TallyODBC64_9000")]
    dsn: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    match tally_odbc_relay::driver::ensure_dll_driver(&args.dsn) {
        Ok(Some(msg)) => println!("{msg}"),
        Ok(None) => {}
        Err(err) => eprintln!("warning: could not prepare Tally ODBC driver: {err}"),
    }
    let addr: SocketAddr = format!("{}:{}", args.bind, args.port).parse()?;
    let listener = TcpListener::bind(addr).await?;
    println!(
        "tally-odbc-relay listening on http://{} (DSN={})",
        listener.local_addr()?,
        args.dsn
    );
    axum::serve(listener, server::router(args.dsn))
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
}
