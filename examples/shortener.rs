// cargo add sqlx --features postgres --features runtime-tokio --features tls-rustls
// brew install postgresql
// psql --version
// brew services start postgresql@18
// brew services info postgresql@18
// brew services stop postgresql@18
// createdb shortener // 创建一个名为 shortener 的数据库
// psql shortener
// shortener=# \q
// shortener=# \l
// psql -l

// brew install pgcli
// pgcli --version
// pgcli shortener
// \dt
// SELECT * FROM urls;
// \q

// psql postgres -c "CREATE ROLE postgres WITH LOGIN PASSWORD 'postgres';"
// psql shortener -c "ALTER DATABASE shortener OWNER TO postgres;"
// psql shortener -c "GRANT ALL ON SCHEMA public TO postgres;"

// cargo run --example shortener

// curl -X POST http://127.0.0.1:9876/ \
//   -H "Content-Type: application/json" \
//   -d '{"original_url": "https://developer.mozilla.org/en-US/docs/Web/HTTP/Status/422"}'

// curl -X POST http://127.0.0.1:9876/ \
//   -H "Content-Type: application/json" \
//   -d '{"original_url": "https://movie.douban.com/subject/35010610/?from=showing"}'

// curl -L http://127.0.0.1:9876/abc123
// The -L flag tells curl to follow redirects. You should see the HTML/content from the original long URL.

// Or without -L to see the 308 redirect response:
// curl -i http://127.0.0.1:9876/abc123

// Useful curl flags
// Flag	Purpose
// -X POST	Specify HTTP method (POST)
// -H "Header: value"	Add a request header
// -d '...'	Send request body data
// -L	Follow redirects automatically
// -i	Include response headers in output
// -v	Verbose mode (shows all details)
// -s	Silent mode (hide progress bar)

use anyhow::Result;
use axum::{
    extract::{Path, State},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};

// LOCATION: The value is the URL the client should redirect to.
// Example HTTP response:
// HTTP/1.1 308 Permanent Redirect
// Location: https://www.rust-lang.org

// HeaderMap is a collection/map of HTTP headers (name-value pairs).
// It's essentially a map:
// Key: HeaderName (like "location", "content-type", etc.)
// Value: HeaderValue (the actual header value as a string)
// let mut headers = HeaderMap::new();
// headers.insert(LOCATION, "https://www.rust-lang.org".parse().unwrap());

// StatusCode is a type-safe wrapper around HTTP status codes.
// It wraps an HTTP status code (100-599)
// pub struct StatusCode(NonZeroU16);
// Represents HTTP response status codes (200 OK, 404 Not Found, etc.)
// Provides type safety—you can't create invalid status codes
// Has predefined constants for common codes (e.g., StatusCode::OK, StatusCode::NOT_FOUND)
// StatusCode is a struct that wraps a NonZeroU16, which means it can only represent valid HTTP status codes (100-599). This design choice ensures that you cannot create invalid status codes, while still allowing for flexibility in defining custom status codes if needed. The use of a struct instead of an enum allows for a wider range of status codes without having to define each one as a separate variant, while still providing the benefits of type safety and validation for valid HTTP
// status codes (any integer between 100 and 999)
// By providing associated constants (e.g., StatusCode::OK, StatusCode::NOT_FOUND), the struct still behaves and feels very much like an enum to the user, providing the best of both worlds.

// POST handler - returns 201 Created
// Ok((StatusCode::CREATED, body))

// GET handler - returns 308 Permanent Redirect
// Ok((StatusCode::PERMANENT_REDIRECT, headers))

// Error cases
// StatusCode::NOT_FOUND  // ID doesn't exist
// StatusCode::UNPROCESSABLE_ENTITY  // URL shortening failed

// The Rust http crate uses type safety to prevent bugs:
// StatusCode is a struct, not just an integer, so you can't accidentally use an invalid code like 999
// LOCATION is a constant, so you can't typo the header name as "Locaton" or "location" (case-insensitive, but the constant ensures correctness)
// HeaderMap provides a strongly-typed container that knows how to serialize headers correctly
// This is why your code is safe, efficient, and self-documenting!

use http::{header::LOCATION, HeaderMap, HeaderValue, StatusCode};
use nanoid::nanoid;
use serde::{Deserialize, Serialize};
// PgPool
// An alias for [`Pool`][crate::pool::Pool], specialized for Postgres.
// pub type PgPool = crate::pool::Pool<Postgres>;
// FromRow
// A record that can be built from a row returned by the database.
// In order to use query_as the output type must implement FromRow.
// This trait can be derived by SQLx for any struct. The generated implementation will consist of a sequence of calls to Row::try_get using the name from each struct field.
use sqlx::{FromRow, PgPool};
use tokio::net::TcpListener;
use tracing::{info, level_filters::LevelFilter, warn};
use tracing_subscriber::{fmt::Layer, layer::SubscriberExt, util::SubscriberInitExt, Layer as _};

// Separation of Concerns (Directionality):
// ShortenReq represents the incoming data from the client (the long URL the client wants to shorten). Thus, it only needs to derive Deserialize.
// ShortenRes represents the outgoing data from the server (the newly generated short URL). Thus, it only needs to derive Serialize.
// Future Extensibility:
// Even though they look structurally identical right now (both just contain a url: String), their shapes will likely diverge as the application grows.
// If you later want clients to send an optional custom alias or expiration date, you would add fields to ShortenReq.
// If you later want to return the exact ID generated or creation timestamp to the client, you would add fields to ShortenRes.
// Keeping them separate means you can easily change the request schema without affecting the response schema, and vice versa.
// Type Safety and Clarity:
// It makes the handler signature (async fn shorten) extremely self-documenting. It's clear that the handler accepts a specific "Request" type and returns a specific "Response" type, preventing accidental mixing of the two.
#[derive(Debug, Deserialize)]
struct ShortenRequest {
    original_url: String,
}

#[derive(Debug, Serialize)]
struct ShortenResponse {
    shortened_url: String,
}

// the db field of type PgPool (from the sqlx crate) is indeed a PostgreSQL connection pool manager.
// By having AppState derive Clone, the underlying database connection pool can be cheaply and safely shared across all of your concurrent Axum request handlers (like shorten and redirect).
// Each request can then independently acquire a connection from the pool to execute its queries.
#[derive(Debug, Clone)]
struct AppState {
    db: PgPool,
}

// The purpose of the UrlRecord struct is to map database query results into a strongly-typed Rust data structure.
// Database Row Mapping: By deriving sqlx::FromRow, sqlx can automatically map columns returned by a SQL query to the fields of this struct (id and url).
#[derive(Debug, FromRow)]
struct UrlRecord {
    #[sqlx(default)]
    id: String, // The short identifier (e.g., "50mzmm"), short ID (the unique identifier for the shortened URL, typically a 6-character string generated by nanoid).
    #[sqlx(default, rename = "url")]
    // The original URL stored in the database is in a column named url, but we want to map it to a field named original_url.
    original_url: String, // long URL (the original URL that the user wants to shorten).
}

const LISTEN_ADDR: &str = "127.0.0.1:9876";

#[tokio::main]
async fn main() -> Result<()> {
    // This code sets up the global logging (tracing) system for your application. Here is a breakdown of what each part does:
    // let layer = Layer::new().with_filter(LevelFilter::INFO);

    // Layer::new(): Creates a new formatting layer, which dictates how the logs will be output (by default, writing human-readable text to the terminal).
    // .with_filter(LevelFilter::INFO): Applies a filter so that only log levels of INFO or higher (i.e., INFO, WARN, and ERROR) are processed. Developer logs like DEBUG or TRACE will be muted.
    // tracing_subscriber::registry().with(layer).init();

    // registry(): Creates a central subscriber to collect all tracing events across your application.
    // .with(layer): Attaches your previously configured formatting and filtering layer to this subscriber.
    // .init(): Initializes this subscriber as the global default. From this point forward, whenever you call macros like info!("...") or warn!("...") anywhere in your code, the events will be routed through this system and printed to your console.

    let layer = Layer::new().with_filter(LevelFilter::INFO);
    tracing_subscriber::registry().with(layer).init();

    // The string "postgres://postgres:postgres@localhost:5432/shortener" follows the standard database connection URI format:
    // protocol://username:password@host:port/database_name
    // Here is what each postgres means respectively:
    // First postgres (postgres://...): The protocol/scheme. It tells the sqlx library which specific database driver to use to establish the connection (PostgreSQL in this case).
    // Second postgres (...//postgres:...): The username. You are authenticating with the PostgreSQL server as the default superuser named postgres.
    // Third postgres (...:postgres@...): The password. The password for the aforementioned user happens to also be set to postgres on your local setup.

    let url = "postgres://postgres:postgres@localhost:5432/shortener";
    let state = AppState::try_new(url).await?;
    info!("Connected to database: {url}");
    let listener = TcpListener::bind(LISTEN_ADDR).await?;
    info!("Listening on: {}", LISTEN_ADDR);

    // In older versions of axum, parameters started with a colon (/:id),
    // but in newer versions, they must be wrapped in curly braces (/{id}).
    // post and get are two different HTTP methods that specify the type of operation the client wants to perform on the server. Here are the key differences:
    // HTTP Method: post is used to create a new resource on the server (e.g., shorten a URL), while get is used to retrieve an existing resource (e.g., redirect to the original URL).
    // Request Body: post typically expects a request body containing data (like the original URL to shorten), whereas get usually does not have a body and relies on URL parameters (like the short ID) to specify what resource to retrieve.
    // POST: Submit/create data, Create short URL; GET: Retrieve data, Retrieve original URL, Redirect to original URL.
    // POST = "I want to create something new on the server"
    // GET = "I want to read/retrieve existing data from the server"
    let app = Router::new()
        .route("/", post(shorten))
        // .route("/:id", get(redirect))
        .route("/{id}", get(redirect))
        .with_state(state);

    axum::serve(listener, app.into_make_service()).await?;
    // axum::serve(...).await?;
    // starts a hyper HTTP server, not an Axum server.
    // axum::serve is an Axum convenience wrapper function that internally uses hyper to start the server. It's a thin abstraction layer.

    // 6. The Server (axum::serve(...).await?)
    // This is the runtime engine. It takes the TCP listener and your newly minted MakeService. It starts an infinite asynchronous loop:

    // Wait for a new TCP connection on the socket.
    // Ask the MakeService for a fresh Router clone for this specific connection.
    // Hand the raw TCP stream and the Router clone off to hyper in a new Tokio background task.
    // hyper reads the bytes, builds the HTTP Request, and feeds it to the Router.

    // When you run axum::serve(listener, app.into_make_service()).await?, a core loop starts running that looks roughly like this (pseudo-code):
    // // 1. You gave the server your IntoMakeService factory
    // let mut make_service = app.into_make_service();

    // // 2. The server loops forever, listening for TCP connections
    // loop {
    //     // Wait for a client to connect (e.g., your browser or curl)
    //     let (tcp_stream, remote_addr) = listener.accept().await.unwrap();

    //     // 3. Ask your factory for a fresh Router for this specific client
    //     let router_clone = make_service.call(remote_addr).await.unwrap();

    //     // 4. Spawn a new Tokio background task to handle this connection concurrently
    //     tokio::spawn(async move {
    //         // 5. Hand the raw TCP socket AND the fresh Router to hyper
    //         hyper::server::conn::serve_connection(tcp_stream, router_clone).await;
    //     });
    // }

    // There are actually TWO nested infinite loops:
    // ┌─────────────────────────────────────────────────────────────┐
    // │  OUTER LOOP: axum::serve (or rather, inside it)            │
    // │                                                              │
    // │  loop {                                                      │
    // │    (tcp_stream, remote_addr) = listener.accept().await       │
    // │                                                              │
    // │    tokio::spawn(async move {                                 │
    // │        ┌──────────────────────────────────────────────────┐  │
    // │        │ INNER LOOP: hyper::serve_connection             │  │
    // │        │                                                  │  │
    // │        │ while client_is_connected {                     │  │
    // │        │   bytes = tcp_stream.read().await               │  │
    // │        │   request = parse_http(bytes)                   │  │
    // │        │   response = router.call(request).await         │  │
    // │        │   response_bytes = serialize_http(response)     │  │
    // │        │   tcp_stream.write(response_bytes).await        │  │
    // │        │ }                                                │  │
    // │        └──────────────────────────────────────────────────┘  │
    // │    })                                                        │
    // │  }                                                           │
    // └─────────────────────────────────────────────────────────────┘

    Ok(())
}

// 当 Json(data): Json<ShortenRequest> 参数放在 State(state): State<AppState> 前面时，Axum 的 top_level_handler_fn! 宏无法正确处理参数提取，导致编译错误。将 State(state) 放在 Json(data) 前面可以解决这个问题，因为 Axum 的宏需要按照特定的顺序提取参数，State 提取器必须在 Json 提取器之前处理。
// The Real Issue
// The error occurs because of how Axum's top_level_handler_fn! macro works internally. When you have:
// Axum's macro needs to extract parameters in a specific order based on how the extractor traits are implemented. The macro generates code that processes extractors, but it has limitations with certain parameter orderings.

// Why This Happens
// Axum uses a macro system to automatically generate the extraction and handler calling code. The macro has bounds on what trait combinations it can handle depending on parameter order.

async fn shorten(
    State(state): State<AppState>,
    Json(data): Json<ShortenRequest>,
) -> Result<impl IntoResponse, StatusCode> {
    let id = state.shorten(&data.original_url).await.map_err(|e| {
        warn!("Failed to shorten URL: {e}");
        StatusCode::UNPROCESSABLE_ENTITY
    })?;
    let body = Json(ShortenResponse {
        shortened_url: format!("http://{}/{}", LISTEN_ADDR, id),
    });
    Ok((StatusCode::CREATED, body))
}
// the tuple is: (StatusCode, Json<ShortenResponse>)
// Which IntoResponse Implementation?
// impl<T> IntoResponse for (StatusCode, T)
// This is a generic implementation of IntoResponse for any tuple where the first element is a StatusCode and the second element can be any type T that itself implements IntoResponse. In your case, T is Json<ShortenResponse>, which does implement IntoResponse, so this implementation applies perfectly to your return type.

async fn redirect(
    Path(id): Path<String>,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, StatusCode> {
    let url = state
        .get_url(&id)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;

    let mut headers = HeaderMap::new();
    // headers.insert(LOCATION, url.parse().unwrap());
    let header_value =
        HeaderValue::from_str(&url).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    headers.insert(LOCATION, header_value);
    Ok((StatusCode::PERMANENT_REDIRECT, headers))
}
// The IntoResponse implementation for (StatusCode, HeaderMap) is defined in the axum crate itself.
// impl IntoResponse for (StatusCode, HeaderMap) { ... }

// PgPool::connect(url).await?
// Return value of PgPool::connect(url): It returns a type that implements Future<Output = Result<PgPool, sqlx::Error>>.
// You must .await it to get the Result, and unwrap/propagate it to get the PgPool.
// Connection count and scaling: By default, it creates a pool configured for multiple connections (the default maximum is usually 10).
// It starts with a minimum number of connections and creates more connections when concurrent requests (like multiple Axum handlers running simultaneously) ask for a database connection, up to the maximum limit.
impl AppState {
    async fn try_new(url: &str) -> Result<Self> {
        let pool = PgPool::connect(url).await?;
        // Create table if not exists
        // This is a raw string literal in Rust. It allows you to write strings without needing to escape characters like quotes (") or backslashes (\). It is highly useful for multi-line SQL queries or JSON payloads.
        // Chaining .execute(&pool): The sqlx::query(...) function builds and returns a Query struct. This struct provides the .execute() method, which accepts any type that implements the sqlx::Executor trait (like &PgPool or a mutable connection).
        // Is the returned value a Future?: Yes. .execute(&pool) returns an asynchronous operation implementing Future<Output = Result<PgQueryResult, sqlx::Error>>. This is why you must chain .await at the end to actually execute the query and get the result.
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS urls (
                id CHAR(6) PRIMARY KEY,
                url TEXT NOT NULL UNIQUE
            )
            "#,
        )
        .execute(&pool)
        .await?;
        // sqlx::query(...)
        // This simply builds an in-memory representation of your parameterized SQL query. No database interaction happens yet.
        // .execute(&pool)
        // Returns: A type that implements Future<Output = Result<PgQueryResult, sqlx::Error>>.
        // Explanation: This consumes the Query builder, takes an executor reference (your connection pool), and constructs an asynchronous task (a Future) ready to be scheduled on the Tokio runtime.
        // .await returns Result<PgQueryResult, sqlx::Error>,
        // (The try operator): ? is used to propagate errors. If the query execution fails, the error will be returned from the try_new function immediately. If it succeeds, the PgQueryResult is returned and then immediately dropped since it's not assigned to any variable.
        // while ? returns PgQueryResult (if successful).
        // Because you don't assign the result to a variable (e.g., let result = ...;), the PgQueryResult is purely evaluated and then immediately dropped.
        // The ? operator can only be used on types that represent success or failure, like Result or Option.
        // the ? operator cannot be applied to type String because it doesn't implement the std::ops::Try trait.

        Ok(Self { db: pool })
    }

    // INSERT INTO urls (id, url) VALUES ($1, $2): Attempts to insert a new row with the newly generated id ($1) and the target url ($2).
    // ON CONFLICT(url): Triggers if the url already exists in the table. (The urls table was created with url TEXT NOT NULL UNIQUE, meaning duplicate URLs will cause a conflict).
    // DO UPDATE SET url=EXCLUDED.url: If a conflict occurs, instead of throwing an error, it performs a dummy update. It sets the url column to the value that was just attempted to be inserted (represented by the special EXCLUDED keyword). This doesn't actually change the data, but it counts as a successful "update" operation.
    // RETURNING id: Whether the row was newly inserted or updated due to the conflict, this returns the id of the row.

    // .fetch_one technically returns a complete UrlRecord struct, not just the ID.

    // Here is exactly what happens:

    // The Database: Because of RETURNING id, PostgreSQL only sends back a single column (id) for the row.
    // query_as Mapping: You told sqlx to map this row into a UrlRecord using query_as.
    // The Result: .fetch_one constructs and returns a UrlRecord. It puts the returned ID into the ret.id field.
    // The Missing Field: Because the database didn't return a url column, sqlx looks at your UrlRecord struct. Since you added the #[sqlx(default)] macro to the fields, it safely fills ret.url with an empty string "" instead of throwing a missing column error.
    // So ret is a full UrlRecord { id: "...", url: "" }, and then your next line (Ok(ret.id)) extracts just the ID string to return it.

    async fn shorten(&self, url: &str) -> Result<String> {
        let id = nanoid!(6);
        let ret: UrlRecord = sqlx::query_as(
            "INSERT INTO urls (id, url) VALUES ($1, $2) ON CONFLICT(url) DO UPDATE SET url=EXCLUDED.url RETURNING id",
        )
        .bind(&id)
        .bind(url)
        .fetch_one(&self.db)
        .await?;
        Ok(ret.id)
    }

    async fn get_url(&self, id: &str) -> Result<String> {
        let ret: UrlRecord = sqlx::query_as("SELECT url FROM urls WHERE id = $1")
            .bind(id)
            .fetch_one(&self.db)
            .await?;
        Ok(ret.original_url)
    }

    // If the SQL query returns absolutely no rows (meaning the id was not found in the database), the .fetch_one(...) method will return an error specifically representing a "not found" state.

    // Here is exactly what happens:

    // The returned error: .fetch_one expects exactly one row. Since there are zero rows, the Future output resolves to Result::Err, specifically pointing to the sqlx::Error::RowNotFound variant.
    // The ? operator: Because you have the ? operator at the end, it unpacks the Result. Seeing the Err, it immediately aborts the get_url execution.
    // Propagation: It converts the sqlx::Error into an anyhow::Error and returns it to the caller (which in your code is the redirect Axum handler).
    // (Note: If you wanted to handle a missing row without it being an explicit error, you would use .fetch_optional(...) instead, which returns Result<Option<UrlRecord>, sqlx::Error>.)
}
