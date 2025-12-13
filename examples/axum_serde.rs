use std::sync::{Arc, Mutex};

use anyhow::{Ok, Result};
use axum::{
    extract::State,
    routing::{get, patch},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use tokio::net::TcpListener;
use tracing::{info, instrument};
use tracing_subscriber::{filter::LevelFilter, fmt, prelude::*, Registry};

#[derive(Serialize, PartialEq, Debug, Clone)]
struct User {
    name: String,
    age: u8,
    skills: Vec<String>,
}

#[derive(Deserialize, Debug, Clone)]
struct UserUpdate {
    age: Option<u8>,
    skills: Option<Vec<String>>,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Build and set a global subscriber using the latest tracing-subscriber APIs
    let subscriber = Registry::default().with(fmt::layer().pretty().with_filter(LevelFilter::INFO));

    tracing::subscriber::set_global_default(subscriber)?;

    let user = User {
        name: "Alice".to_string(),
        age: 30,
        skills: vec!["Rust".to_string(), "WebAssembly".to_string()],
    };
    let user = Arc::new(Mutex::new(user));

    let addr = "0.0.0.0:8080";
    let listener = TcpListener::bind(addr).await?;
    info!("Listening on {}", addr);

    let app = Router::new()
        .route("/", get(user_handler))
        .route("/", patch(update_handler))
        .with_state(user);
    axum::serve(listener, app.into_make_service()).await?;

    Ok(())
}

// GET: Safe, read-only. Retrieves a resource. No request body is required. Should be cacheable and idempotent.
// PATCH: Applies partial updates to a resource. Carries a body with only the fields to change. Not necessarily cacheable; should be idempotent by design, but can be non-idempotent depending on implementation.

#[instrument]
async fn user_handler(State(user): State<Arc<Mutex<User>>>) -> Json<User> {
    // Json(user.lock().unwrap().clone())
    // // or

    // (*user.lock().unwrap()).clone().into()
    // (user.lock().unwrap()).clone().into()
    // there’s effectively no difference here.
    // MutexGuard<User> implements Deref<Target = User>, so method calls auto-deref.
    // In (user.lock().unwrap()).clone().into(), auto-deref makes clone() act on User, producing a User, and then into() converts User → Json<User>.
    // The explicit * in (*user.lock().unwrap()).clone().into() just makes the deref visible; it doesn’t change behavior.
    // Prefer the clearer version:
    let user = user.lock().unwrap().clone();
    Json(user) // or: user.into()
}

// Rust deref/coercion rules are the difference.

// user.lock() returns a MutexGuard<User>, not a User.
// Json(user.lock().unwrap().clone()):
// Json(T) needs a User.
// .clone() is a method on User, and method calls use deref coercion, so Rust treats guard.clone() as (*guard).clone() automatically.
// (*user.lock().unwrap()).clone().into():
// into() must be called on a User (there’s an impl From<User> for Json<User>).
// Trait method resolution also supports deref, so guard.clone().into() would work, but explicitly writing (*guard) makes the coercion obvious.

#[instrument]
async fn update_handler(
    State(user): State<Arc<Mutex<User>>>,
    Json(user_update): Json<UserUpdate>,
) -> Json<User> {
    let mut user = user.lock().unwrap();
    if let Some(age) = user_update.age {
        user.age = age;
    }
    if let Some(skills) = user_update.skills {
        user.skills = skills;
    }
    Json(user.clone())
}
