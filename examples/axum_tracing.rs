// let’s walk through this code step by step. It’s a small Axum web server that integrates Tokio (for async runtime) and tracing (for structured, asynchronous logging).

// Duration: For time measurements
// axum: Web framework for routing and HTTP handling
// tokio: Async runtime for networking and timers
// tracing: Structured logging framework
// tracing_subscriber: Configures how logs are formatted and output

use std::time::Duration;

use axum::{routing::get, Router};
use tokio::{
    net::TcpListener,
    time::{sleep, Instant},
};
use tracing::{debug, info, instrument, level_filters::LevelFilter, warn};
use tracing_subscriber::{
    fmt::{self, format::FmtSpan},
    layer::SubscriberExt,
    util::SubscriberInitExt,
    Layer,
};

// Main Function Setup
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Logging Configuration
    // Logging setup breakdown:
    // File appender: Creates daily rotating log files in logs
    // Non-blocking writer: Prevents I/O blocking the main thread
    // Console layer: Logs to stdout with DEBUG level and pretty formatting
    // File layer: Logs to file with INFO level and pretty formatting
    // Registry: Combines multiple logging layers
    let file_appender = tracing_appender::rolling::daily("/tmp/logs", "ecosystem.log");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

    let console = fmt::Layer::new()
        .with_span_events(FmtSpan::CLOSE) // log when spans close
        .pretty() // pretty formatting
        .with_filter(LevelFilter::DEBUG); // console shows DEBUG+

    let file = fmt::Layer::new()
        .with_writer(non_blocking) // write asynchronously to file
        .pretty()
        .with_filter(LevelFilter::INFO); // file shows INFO+

    // Then you combine them with:
    tracing_subscriber::registry()
        .with(console)
        .with(file)
        .init();

    // Server Setup
    let addr = "127.0.0.1:8080";
    // ── AXUM: define routes, handlers (the “menu + chef”) ─────
    let app = Router::new().route("/", get(index_handler));

    // --- bind a TCP socket with Tokio (OS socket via runtime reactor) ---
    let listener = TcpListener::bind(addr).await?;
    // --- serve the app (Hyper under the hood via Axum server) ---
    // Axum converts `Router` into a Hyper `Service`, Hyper does HTTP I/O on Tokio.
    info!("Starting server on {}", addr);
    axum::serve(listener, app.into_make_service()).await?; // ← AXUM API, uses HYPER server on top of TOKIO
    Ok(())
}

// ── AXUM: handlers (your business logic = “recipes”) ──────────────────────
// #[instrument] is a procedural macro from the tracing ecosystem.
// When you put it on a function, it automatically creates and manages a span for every call to that function.
// Request Handler Chain
#[instrument]
async fn index_handler() -> &'static str {
    debug!("index handler started");
    sleep(Duration::from_millis(10)).await;
    let ret = long_task().await;
    info!(http.status = 200, "index handler completed");
    ret
}

#[instrument]
async fn long_task() -> &'static str {
    let start = Instant::now();
    sleep(Duration::from_millis(112)).await;
    let elapsed = start.elapsed().as_millis() as u64;
    warn!(app.task_duration = elapsed, "task takes too long");
    "Hello, World!"
}
