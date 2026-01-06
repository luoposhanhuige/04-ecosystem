use anyhow::Result;
use serde::{Deserialize, Serialize};
// use serde_json;

#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct User {
    name: String,
    age: u8,
    skills: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
enum MyState {
    Init(String),
    Running(Vec<String>),
    Done(u32),
}
fn main() -> Result<()> {
    let user = User {
        name: "Alice".to_string(),
        age: 30,
        skills: vec!["Rust".to_string(), "WebAssembly".to_string()],
    };

    // Serialize the user to a JSON string.
    let json = serde_json::to_string(&user)?;
    println!("{}", json);

    // Deserialize the JSON string back to a User struct.
    let user1: User = serde_json::from_str(&json)?;
    println!("{:?}", user1);
    // assert_eq!(user, user1);

    let state = MyState::Running(vec!["Rust".to_string(), "Python".to_string()]);
    let json = serde_json::to_string(&state)?;
    println!("{}", json);

    Ok(())
}
