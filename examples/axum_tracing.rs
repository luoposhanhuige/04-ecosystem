// let’s walk through this code step by step. It’s a small Axum web server that integrates Tokio (for async runtime) and tracing (for structured, asynchronous logging).

// Duration: For time measurements
// axum: Web framework for routing and HTTP handling
// tokio: Async runtime for networking and timers
// tracing: Structured logging framework
// tracing_subscriber: Configures how logs are formatted and output

use axum::{extract::Request, routing::get, Router};
use opentelemetry::{
    global,
    propagation::Extractor,
    trace::{TraceContextExt, TracerProvider as _},
    KeyValue,
};
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::{propagation::TraceContextPropagator, trace::SdkTracerProvider, Resource};
use std::time::Duration;
use tokio::{
    join,
    net::TcpListener,
    time::{sleep, Instant},
};
use tracing::{debug, info, instrument, level_filters::LevelFilter, warn};
use tracing_opentelemetry::OpenTelemetrySpanExt; // for setting parent context on current span
use tracing_subscriber::{
    fmt::{self, format::FmtSpan},
    layer::SubscriberExt,
    util::SubscriberInitExt,
    Layer,
};

// Main Function Setup
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // --------------------------
    // Console Layer for tracing-subscriber
    // --------------------------
    let console = fmt::Layer::new()
        .with_span_events(FmtSpan::CLOSE) // log when spans close
        .pretty() // pretty formatting
        .with_filter(LevelFilter::DEBUG); // console shows DEBUG+

    // --------------------------
    // File Layer
    // --------------------------
    // Logging Configuration
    // Logging setup breakdown:
    // File appender: Creates daily rotating log files in logs
    // Non-blocking writer: Prevents I/O blocking the main thread
    // Console layer: Logs to stdout with DEBUG level and pretty formatting
    // File layer: Logs to file with INFO level and pretty formatting
    // Registry: Combines multiple logging layers
    let file_appender = tracing_appender::rolling::daily("/tmp/logs", "ecosystem.log");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);
    let file = fmt::Layer::new()
        .with_writer(non_blocking) // write asynchronously to file
        .pretty()
        .with_filter(LevelFilter::INFO); // file shows INFO+

    // --------------------------
    // OpenTelemetry Layer for tracing-subscriber
    // --------------------------
    // Initialize OpenTelemetry (new API)
    let tracer_provider = init_tracer_provider()?; // creates SdkTracerProvider with batch exporter.

    // Create tracer bound to our SDK provider (SdkTracer implements required traits)
    let otel_tracer = tracer_provider.tracer("axum-tracing");
    let opentelemetry = tracing_opentelemetry::layer().with_tracer(otel_tracer);

    // Then you combine them with:
    // Compose subscriber:
    tracing_subscriber::registry()
        .with(console)
        .with(file)
        .with(opentelemetry)
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
    axum::serve(listener, app.into_make_service()).await?; // ← AXUM API, uses HYPER server on top of TOKIO. runs Hyper on Tokio.

    // Optionally force flush before exiting (best-effort)
    // (In 0.30, dropping the provider will flush. Explicit flush omitted for simplicity.)
    drop(tracer_provider); // Cleanup: to flush spans on shutdown.
    Ok(())
}

// ── AXUM: handlers (your business logic = “recipes”) ──────────────────────
// #[instrument] is a procedural macro from the tracing ecosystem.
// When you put it on a function, it automatically creates and manages a span for every call to that function.
// Request Handler Chain

// Adds http.uri and http.method fields to the span.
// extract_remote_context(req) tries to read W3C traceparent/tracestate headers; if present, sets current span’s parent.
// Awaits long_task(); logs info with status_code=200; returns response string.
#[instrument(fields(http.uri = req.uri().path(), http.method = req.method().as_str()))]
async fn index_handler(req: Request) -> &'static str {
    debug!("index handler started");
    // Extract remote trace context (if any) from incoming headers and set current span parent
    if let Some(ctx) = extract_remote_context(&req) {
        tracing::Span::current().set_parent(ctx);
    }
    sleep(Duration::from_millis(10)).await;
    let ret = long_task().await;
    info!(http.status_code = 200, "index handler completed");
    ret
}

// Starts timer, concurrently awaits sl + task1 + task2 + task3 via join!.
// Logs warn! with total duration (ms).
#[instrument]
async fn long_task() -> &'static str {
    let start = Instant::now();
    let sl = sleep(Duration::from_millis(11));
    // spawn multiple tasks
    let t1 = task1();
    let t2 = task2();
    let t3 = task3();
    join!(sl, t1, t2, t3);
    let elapsed = start.elapsed().as_millis() as u64;
    warn!(app.task_duration = elapsed, "task takes too long");
    "Hello, World!"
}

// Sleep to simulate work; each has its own span from #[instrument].
#[instrument]
async fn task1() {
    sleep(Duration::from_millis(10)).await;
}

#[instrument]
async fn task2() {
    sleep(Duration::from_millis(50)).await;
}

#[instrument]
async fn task3() {
    sleep(Duration::from_millis(30)).await;
}

// opentelemetry 旧版本代码
// fn init_tracer() -> anyhow::Result<Tracer> {
//     let tracer = opentelemetry_otlp::new_pipeline()
//         .tracing()
//         .with_exporter(
//             opentelemetry_otlp::new_exporter()
//                 .tonic()
//                 .with_endpoint("http://localhost:4317"),
//         )
//         .with_trace_config(
//             trace::config()
//                 .with_id_generator(RandomIdGenerator::default())
//                 .with_max_events_per_span(32)
//                 .with_max_attributes_per_span(64)
//                 .with_resource(Resource::new(vec![KeyValue::new(
//                     "service.name",
//                     "axum-tracing",
//                 )])),
//         )
//         .install_batch(runtime::Tokio)?;
//     Ok(tracer)
// }

// fn init_tracer_provider() -> anyhow::Result<SdkTracerProvider> {
//     // W3C context propagation
//     global::set_text_map_propagator(TraceContextPropagator::new());

//     // OTLP exporter (gRPC / tonic)
//     let endpoint = std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT")
//         .unwrap_or_else(|_| "http://localhost:4317".to_string());
//     let exporter = opentelemetry_otlp::SpanExporter::builder()
//         .with_tonic()
//         .with_endpoint(endpoint)
//         .build()?;

//     // Resource describing this service (builder API; Resource::new is private in 0.30)
//     let resource = Resource::builder()
//         .with_attribute(KeyValue::new("service.name", "axum-tracing"))
//         .with_attribute(KeyValue::new("service.version", env!("CARGO_PKG_VERSION")))
//         .with_attribute(KeyValue::new(
//             "deployment.environment",
//             std::env::var("RUST_ENV").unwrap_or_else(|_| "dev".into()),
//         ))
//         .build();

//     // Tracer provider with batch exporter on Tokio runtime
//     let provider = SdkTracerProvider::builder()
//         .with_resource(resource)
//         .with_batch_exporter(exporter) // runtime inferred via feature rt-tokio
//         .build();

//     // Install globally so tracing_opentelemetry can fetch it
//     global::set_tracer_provider(provider.clone());
//     Ok(provider)
// }

// ...existing code...
fn init_tracer_provider() -> anyhow::Result<SdkTracerProvider> {
    // Enables W3C context extraction/injection.
    global::set_text_map_propagator(TraceContextPropagator::new());

    // Endpoint selection:
    // Prefer per-signal var, then global; default to local gRPC
    // Reads OTEL_EXPORTER_OTLP_TRACES_ENDPOINT or OTEL_EXPORTER_OTLP_ENDPOINT.
    // Default: http://127.0.0.1:4317.
    let endpoint = std::env::var("OTEL_EXPORTER_OTLP_TRACES_ENDPOINT")
        .or_else(|_| std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT"))
        .unwrap_or_else(|_| "http://127.0.0.1:4317".to_string());

    // Allow switching protocol: grpc (4317) or http/protobuf (4318)
    // Protocol selection:
    // Reads OTEL_EXPORTER_OTLP_PROTOCOL; default "grpc".
    // If "http" or "http/protobuf":
    // Builds HTTP exporter; ensures endpoint ends with /v1/traces.
    // Else:
    // Builds gRPC exporter via tonic.
    let protocol =
        std::env::var("OTEL_EXPORTER_OTLP_PROTOCOL").unwrap_or_else(|_| "grpc".to_string()); // "grpc" or "http/protobuf"

    // Resource describing this service
    // service.name="axum-tracing", service.version from Cargo.
    let resource = Resource::builder()
        .with_attribute(KeyValue::new("service.name", "axum-tracing"))
        .with_attribute(KeyValue::new("service.version", env!("CARGO_PKG_VERSION")))
        .build();

    let provider = if protocol == "http/protobuf" || protocol == "http" {
        // Requires opentelemetry-otlp feature: http-proto (you can enable both http-proto and grpc-tonic)
        let http_endpoint = if endpoint.ends_with("/v1/traces") {
            endpoint.clone()
        } else {
            format!("{endpoint}/v1/traces")
        };
        let exporter = opentelemetry_otlp::SpanExporter::builder()
            .with_http() // OTLP/HTTP
            .with_endpoint(http_endpoint)
            .build()?;
        SdkTracerProvider::builder()
            .with_resource(resource)
            .with_batch_exporter(exporter)
            .build()
    } else {
        // Default gRPC over tonic (h2c, plaintext)
        let exporter = opentelemetry_otlp::SpanExporter::builder()
            .with_tonic() // OTLP/gRPC
            .with_endpoint(endpoint)
            .build()?;
        SdkTracerProvider::builder()
            .with_resource(resource)
            .with_batch_exporter(exporter)
            .build()
    };

    // Makes it discoverable by tracing-opentelemetry.
    global::set_tracer_provider(provider.clone());
    Ok(provider)
}
// ...existing code...

// ---- Distributed trace context extraction helpers ----
struct HeaderExtractor<'a>(&'a axum::http::HeaderMap);

// HeaderExtractor implements opentelemetry::propagation::Extractor for Axum headers.
impl<'a> Extractor for HeaderExtractor<'a> {
    fn get(&self, key: &str) -> Option<&str> {
        self.0.get(key).and_then(|v| v.to_str().ok())
    }
    fn keys(&self) -> Vec<&str> {
        self.0.keys().map(|k| k.as_str()).collect()
    }
}

// Uses global propagator to extract parent Context from headers.
// Returns Some(ctx) only if a valid remote SpanContext exists.
fn extract_remote_context(req: &Request) -> Option<opentelemetry::Context> {
    let headers = req.headers();
    if headers.is_empty() {
        return None;
    }
    let carrier = HeaderExtractor(headers);
    let ctx = global::get_text_map_propagator(|prop| prop.extract(&carrier));
    // If no remote trace, the extracted context will have no span – heuristic check
    if ctx.span().span_context().is_valid() {
        Some(ctx)
    } else {
        None
    }
}

// Env vars that control export

// OTEL_EXPORTER_OTLP_PROTOCOL: "grpc" (4317) or "http/protobuf" (4318).
// OTEL_EXPORTER_OTLP_ENDPOINT or OTEL_EXPORTER_OTLP_TRACES_ENDPOINT: target URL.
// NO_PROXY must include 127.0.0.1,localhost,::1 to bypass proxies for local Collector.
// What you get

// Pretty console logs (DEBUG+), rotating file logs (INFO+).
// Structured spans for each handler/task with automatic parenting.
// Export to Collector with protocol chosen at runtime via envs.

// Short answer: tracing is the idiomatic in-process instrumentation and logging API for Rust; OpenTelemetry is the vendor-neutral telemetry API/SDK and exporter. You typically want both, bridged by tracing-opentelemetry.

// Why not “OpenTelemetry only”:

// Ecosystem integration: Most Rust libs (axum, tower, hyper, reqwest, tokio, sqlx) emit tracing spans/events. You get that instrumentation “for free” by using tracing. OpenTelemetry-only would miss these or require custom glue.
// Logging and formatting: tracing-subscriber + tracing-appender provide console formatting, filters (EnvFilter), and rolling/non-blocking file logs. The OTel SDK doesn’t do pretty console/file logs.
// Layered fan-out: The tracing subscriber model lets you send the same spans/events to multiple sinks at once (console, files, OTel/exporter) with composable layers. OTel alone focuses on exporting to backends.
// Ergonomics and performance: tracing macros/attributes (#[instrument], info!, warn!) are zero-cost when disabled and very fast when enabled. The API is Rust-first and widely adopted.
// Separation of concerns: tracing = in-process instrumentation pipeline; OTel = propagation, resource, sampling, batching, and export (OTLP via tonic). The bridge (tracing-opentelemetry) lets you keep rich local logs and also export to a Collector.
// Portability and resilience: Your app remains useful without a Collector (console/file logs still work). Adding/removing OTel export is a subscriber-layer change, not a rewrite.
// How yours fits together:

// tracing: write spans/events in app code.
// tracing-subscriber: registry, EnvFilter, fmt layer.
// tracing-appender: non-blocking, rolling file output.
// tracing-opentelemetry: converts tracing spans to OTel spans.
// opentelemetry + opentelemetry_sdk: API + SDK (provider, batch processor, resources, propagators).
// opentelemetry-otlp + tonic: export spans to Collector over gRPC.
// This stack gives you great dev-time logs and production-grade telemetry export with minimal duplication.

// Here’s what your axum_tracing.rs does, section by section.

// High-level flow

// Incoming HTTP request → Axum handler creates spans (tracing).
// tracing-opentelemetry layer converts spans to OpenTelemetry.
// opentelemetry_sdk batches them.
// opentelemetry-otlp exporter sends to Collector (gRPC 4317 or HTTP 4318), chosen by env.
// Imports (what each crate is used for)

// axum: HTTP router/server.
// tracing, tracing_subscriber, tracing_appender: spans/logs, formatting, and file output.
// tracing-opentelemetry: bridge tracing → OpenTelemetry.
// opentelemetry + opentelemetry_sdk: OTel API/SDK, propagators, resources, tracer provider.
// opentelemetry-otlp: OTLP exporter (gRPC via tonic or HTTP/proto).
// tokio: async runtime, TCP listener, timers.
// main()

// Build console layer:
// Pretty logs, span close events, LevelFilter::DEBUG.
// Build file layer:
// Daily rotating file at /tmp/logs/ecosystem.log.
// Non-blocking writer (keeps _guard alive).
// INFO+ to file.
// OpenTelemetry:
// init_tracer_provider() creates SdkTracerProvider with batch exporter.
// Get tracer and make tracing-opentelemetry layer with it.
// Compose subscriber:
// registry().with(console).with(file).with(opentelemetry).init().
// Server:
// Bind 127.0.0.1:8080 with TcpListener.
// axum::serve(listener, app.into_make_service()) runs Hyper on Tokio.
// Cleanup:
// drop(tracer_provider) to flush spans on shutdown.
// Handlers and spans

// #[instrument] on functions:
// Automatically creates a span per call and records arguments.
// index_handler(req):
// Adds http.uri and http.method fields to the span.
// extract_remote_context(req) tries to read W3C traceparent/tracestate headers; if present, sets current span’s parent.
// Awaits long_task(); logs info with status_code=200; returns response string.
// long_task():
// Starts timer, concurrently awaits sl + task1 + task2 + task3 via join!.
// Logs warn! with total duration (ms).
// task1/task2/task3():
// Sleep to simulate work; each has its own span from #[instrument].
// OpenTelemetry initialization

// global::set_text_map_propagator(TraceContextPropagator::new()):
// Enables W3C context extraction/injection.
// Endpoint selection:
// Reads OTEL_EXPORTER_OTLP_TRACES_ENDPOINT or OTEL_EXPORTER_OTLP_ENDPOINT.
// Default: http://127.0.0.1:4317.
// Protocol selection:
// Reads OTEL_EXPORTER_OTLP_PROTOCOL; default "grpc".
// If "http" or "http/protobuf":
// Builds HTTP exporter; ensures endpoint ends with /v1/traces.
// Else:
// Builds gRPC exporter via tonic.
// Resource:
// service.name="axum-tracing", service.version from Cargo.
// Provider:
// SdkTracerProvider::builder() .with_resource(resource) .with_batch_exporter(exporter) .build();
// rt-tokio feature on opentelemetry_sdk wires the Tokio runtime for batching.
// global::set_tracer_provider(provider.clone()):
// Makes it discoverable by tracing-opentelemetry.
// HTTP context propagation helpers

// HeaderExtractor implements opentelemetry::propagation::Extractor for Axum headers.
// extract_remote_context(req):
// Uses global propagator to extract parent Context from headers.
// Returns Some(ctx) only if a valid remote SpanContext exists.
// In index_handler, current span adopts that parent (distributed tracing).
// Env vars that control export

// OTEL_EXPORTER_OTLP_PROTOCOL: "grpc" (4317) or "http/protobuf" (4318).
// OTEL_EXPORTER_OTLP_ENDPOINT or OTEL_EXPORTER_OTLP_TRACES_ENDPOINT: target URL.
// NO_PROXY must include 127.0.0.1,localhost,::1 to bypass proxies for local Collector.
// What you get

// Pretty console logs (DEBUG+), rotating file logs (INFO+).
// Structured spans for each handler/task with automatic parenting.
// Export to Collector with protocol chosen at runtime via envs.
