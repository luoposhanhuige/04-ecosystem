use anyhow::Context;
use ecosystem::MyError;
use std::fs;

fn main() -> Result<(), anyhow::Error> {
    println!("size of MyError is {}", std::mem::size_of::<MyError>());
    let filename = "non-existent.txt";
    // let _fd = fs::File::open(filename).context(format!("can't find {}", filename))?;
    let _fd = fs::File::open(filename).with_context(|| format!("can't find {}", filename))?;
    // The .context(filename) method here adds additional context information to the error before it's converted to anyhow::Error.
    // If fs::File::open() fails: The original std::io::Error gets wrapped with the filename as additional context
    // With .context(filename):
    // Error message becomes:
    // "non-existent.txt: No such file or directory (os error 2)"
    fail_with_error()?;
    Ok(())
}

fn fail_with_error() -> Result<(), MyError> {
    Err(MyError::Custom("An error occurred".into()))
}
