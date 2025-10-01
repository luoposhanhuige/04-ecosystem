use anyhow::Result;
use chrono::{DateTime, Datelike, Utc};
use derive_builder::Builder;

#[allow(unused)]
#[derive(Builder, Debug)]
#[builder(build_fn(name = "_priv_build"))]
struct User {
    #[builder(setter(into))]
    name: String,
    #[builder(setter(into, strip_option), default)]
    email: Option<String>,
    #[builder(setter(custom))]
    dob: DateTime<Utc>, // date of birth
    // #[builder(default = "30")]
    #[builder(setter(skip))]
    age: u32,
    #[builder(default = "vec![]", setter(each(name = "skill", into)))]
    skills: Vec<String>,
}

fn main() -> Result<()> {
    let user = UserBuilder::default()
        .name("Alice")
        .email("alice@example.com")
        .dob("1990-01-01T00:00:00Z")
        // .age(30)
        .skill("Rust")
        .skill("Web Development")
        .build()?;

    println!("{:?}", user);
    Ok(())
}

#[allow(unused)]
impl User {
    pub fn build() -> UserBuilder {
        UserBuilder::default()
    }
}

impl UserBuilder {
    pub fn build(&self) -> Result<User> {
        let mut user = self._priv_build()?;
        user.age = (Utc::now().year() - user.dob.year()) as _;
        Ok(user)
    }
    pub fn dob(&mut self, value: &str) -> &mut Self {
        self.dob = DateTime::parse_from_rfc3339(value)
            .map(|dt| dt.with_timezone(&Utc))
            .ok();
        self
    }
}
