use anyhow::Result;
use chrono::{DateTime, Datelike, Utc}; // time handling; Datelike gives .year().
use derive_builder::Builder; // procedural macro that generates a builder for your struct.

// DateTime<Tz>: a timezone-aware timestamp. Generic over a time zone type Tz (e.g., Utc, Local, FixedOffset).
// Utc: the UTC time zone type (zero offset). Used as the Tz in DateTime<Utc>.
// Datelike: a trait that provides date parts (year, month, day). Implemented by DateTime<Tz>, NaiveDate, etc.
// How they connect
// Utc implements TimeZone, so you can have DateTime<Utc>.
// DateTime<Tz> implements Datelike, so you can call .year(), .month(), .day() (bring chrono::Datelike into scope).
// Parsing often yields DateTime<FixedOffset>; convert to UTC with with_timezone(&Utc).

#[allow(unused)]
#[derive(Builder, Debug)] // derive_builder generates a UserBuilder with setters and a build() (you renamed it).
#[builder(build_fn(name = "_priv_build"))] // #[builder(build_fn(name = "_priv_build"))] renames the generated build() to _priv_build(), so you can write your own build() wrapper.
struct User {
    #[builder(setter(into))] // Setter accepts anything Into<String> (so .name("Alice") works).
    name: String,
    // strip_option literally means “strip off the Option wrapper” for the builder setter.
    #[builder(setter(into, strip_option), default)]
    // strip_option: the setter takes a String/Into<String> (not Option<String>); it wraps it in Some. default: if you don’t call .email(...), it defaults to None.
    email: Option<String>,
    // No auto-generated setter. You implement UserBuilder::dob(&mut self, &str) yourself (see impl block) that parses RFC3339 and stores Some(DateTime<Utc>) or None on parse failure.
    // If it’s None at build time, _priv_build() returns an error (“dob not set”).
    #[builder(setter(custom))]
    dob: DateTime<Utc>, // date of birth
    // No setter is generated. You plan to compute it in your custom build().
    // Important: without a default, the generated _priv_build() will fail because age wasn’t set. Add default (e.g., 0) so the generated build can succeed and you can overwrite the field afterward.
    // #[builder(default = "30")]
    // Keep #[builder(setter(skip), default)]. For u32, Default::default() is 0, so this already sets age to 0 when not provided.
    #[builder(setter(skip), default)] // _priv_build() builds User with age = 0 (default).
    age: u32,
    // default empty Vec.
    // each(...) lets you call .skill("Rust") multiple times to push items; into allows &str.
    #[builder(default = "vec![]", setter(each(name = "skill", into)))]
    skills: Vec<String>,
}

fn main() -> Result<()> {
    // Setters return a mutable reference to the same builder, not a new object:
    // name/email/dob/skill have signatures like fn ...(&mut self, ...) -> &mut Self
    // This mutates the same UserBuilder and returns &mut UserBuilder to enable chaining.
    // The final call builds the value:
    // Generated: fn _priv_build(&self) -> Result<User, ...>
    // Yours: fn build(&self) -> anyhow::Result<User>
    // .build() returns a Result<User>; with ?, you get a User.
    // So the chain mutates one builder and then returns a User at the end:
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

// UserBuilder::default() is the constructor that derive_builder generates for the builder type. It returns a fresh builder with all fields “unset” so you can start chaining setters.

// Conceptually the macro generates something like:

// struct UserBuilder {
//     name: Option<String>,            // required (no default)
//     email: Option<Option<String>>,   // Option<T> + strip_option → track “unset” vs Some(None)/Some(Some(v))
//     dob: Option<DateTime<Utc>>,      // required (custom setter)
//     age: Option<u32>,                // setter(skip); will use default at build time
//     skills: Option<Vec<String>>,     // will default to vec![] at build time
// }

// impl Default for UserBuilder {
//     fn default() -> Self {
//         Self { name: None, email: None, dob: None, age: None, skills: None }
//     }
// }

// So calling UserBuilder::default():

// Allocates no data (all fields are None).
// Does not validate anything.
// Just gives you a builder ready for .name(...), .email(...), .dob(...), .skill(...), then .build().
// Defaults are applied later, in _priv_build():

// name and dob: must have been set, otherwise error.
// email: if never set, becomes None (because of #[builder(default)]).
// age: uses Default (0) because of #[builder(default)] and setter(skip), then you overwrite it in your custom build().
// skills: becomes vec![] if never set (#[builder(default = "vec![]")]).

// You provided a convenience constructor User::build() → UserBuilder::default().
// 这段代码，暂时没用到
#[allow(unused)]
impl User {
    pub fn build() -> UserBuilder {
        UserBuilder::default()
    }
}

impl UserBuilder {
    pub fn build(&self) -> Result<User> {
        // Calls self._priv_build()? to let derive_builder assemble User and validate required fields.
        let mut user = self._priv_build()?;
        // Computes age from current year − dob.year() and writes it into user.age.
        user.age = (Utc::now().year() - user.dob.year()) as _; // “as _” is a cast to “the type expected here.” It’s shorthand for as <inferred type>.
        Ok(user)
    }
    pub fn dob(&mut self, value: &str) -> &mut Self {
        self.dob = DateTime::parse_from_rfc3339(value)
            .map(|dt| dt.with_timezone(&Utc)) // with_timezone changes the timezone type while preserving the instant. Example: 2025-10-05T12:00:00+08:00.with_timezone(&Utc) → 2025-10-05T04:00:00Z.
            .ok(); // Converts Result<DateTime<Utc>, ParseError> into Option<DateTime<Utc>>.
        self
    }
}

// “as _” is a cast to “the type expected here.”

// It’s shorthand for as <inferred type>.
// The target type is taken from context (e.g., the variable being assigned to, a function parameter type, etc.).
// In your code: user.age = (Utc::now().year() - user.dob.year()) as _; Since age is u32, this is the same as as u32.

// Note: i32 → u32 can wrap if negative. A safer conversion:

// // ...existing code...
//         user.age = (Utc::now().year() - user.dob.year())
//             .try_into()
//             .unwrap_or(0);
// // ...existing code...

// RFC 3339 is an IETF standard (“Request for Comments” 3339) that defines a precise, internet-friendly timestamp format. It’s a strict profile of ISO 8601.

// Key rules

// Layout: YYYY-MM-DDTHH:MM:SS[.fraction]Z or ±HH:MM offset.
// “T” separates date and time; “Z” means UTC.
// Time zone is required (either Z or an explicit offset).
// Offset must include a colon (e.g., +08:00, not +0800).
// Valid examples

// 1990-01-01T00:00:00Z
// 1990-01-01T08:00:00+08:00
// 2025-10-05T12:34:56.789123Z

// _priv_build() is the build method generated by derive_builder for UserBuilder. When you call self._priv_build()?:

// It validates required fields:
// For each field, if a value was provided (via setters) it uses it.
// If the field has #[builder(default …)] it uses that default.
// If the field type is Option<T> and no value was set, it becomes None (unless you used strip_option, which changes only the setter shape).
// Otherwise it returns an error “<field> must be initialized”.
// It constructs and returns User if everything is satisfied.
// The ? operator propagates any build error out of your UserBuilder::build() as anyhow::Error.

// is this _priv_build() generated inside the code of my impl UserBuilder {...}?
// No. _priv_build() is generated by the derive_builder macro, not written inside your impl UserBuilder block.

// #[derive(Builder)] creates the UserBuilder type and an impl UserBuilder with methods (setters, build).
// #[builder(build_fn(name = "_priv_build"))] tells the macro to name the generated build method _priv_build.
// Your impl UserBuilder {...} is a separate, hand-written impl that adds build(&self) -> anyhow::Result<User> and dob(&mut self, &str). Both impls coexist; the type ends up with all methods.

// See the generated code
// On macOS in your workspace:
// cargo install cargo-expand
// cargo expand --example builder | less

// You’ll find something conceptually like:
// impl UserBuilder {
//     pub fn _priv_build(&self) -> Result<User, derive_builder::UninitializedFieldError> {
//         // validates required fields, applies defaults, constructs User
//     }
// }
// Your build() calls self._priv_build()?, then computes age and returns Ok(user).

// the derive_builder macro generated code of "impl UserBuilder {...}" doesn't conflict with the code of "impl UserBuilder {...}" i created, why?

// They don’t conflict because Rust allows multiple inherent impl blocks for the same type. All methods from all impl blocks are merged as long as there are no duplicate method names.

// derive_builder expands to an impl UserBuilder { … } that adds setters and a build-like method.
// You wrote another impl UserBuilder { … } with your own build() and dob().
// No clash occurs because:

// You renamed the macro’s build to _priv_build via #[builder(build_fn(name = "_priv_build"))], so there isn’t a second build() with the same name.
// You marked dob with #[builder(setter(custom))], so the macro doesn’t generate a dob setter; only your version exists.
// If both impls defined the same method name (even with different signatures), the compiler would error with a duplicate definition.

// when the cursor hovers on the UserBuilder::default(), why the runtime can show up a detail of UserBuilder struct since the procedural macro of Builder doesn't work before the compiling?

// It’s your IDE (rust-analyzer), not “runtime.”
// rust-analyzer can execute procedural macros during analysis. It compiles proc-macro crates (like derive_builder) for the host, loads them, and asks them to expand. It caches the result.
// The expansion creates UserBuilder and its methods (including _priv_build/default). With that, rust-analyzer knows the symbol and can show hover, completions, go-to-def, etc.
// This happens before you do a full cargo build; it’s editor-time analysis.

// Tips

// Ensure proc macros are enabled in VS Code:
// Settings → “rust-analyzer.procMacro.enable” = true
// See status: Command Palette → “Rust Analyzer: Status” (should say “Proc macros: enabled”).
// If proc macros are disabled, rust-analyzer can’t see UserBuilder, so no hover info, completions, etc.

// Here’s a close-to-actual expansion that derive_builder would generate for your case. The exact names/types may vary slightly by crate version, but the logic matches.
// Conceptual expansion from #[derive(Builder)] with your attributes.
// impl UserBuilder {
//     pub fn _priv_build(&self)
//         -> Result<User, derive_builder::UninitializedFieldError>
//     {
//         // name: required (no default)
//         let name: String = self
//             .name
//             .clone()
//             .ok_or_else(|| derive_builder::UninitializedFieldError::new("name"))?;

//         // email: Option<String> with #[builder(default)] and strip_option
//         // Builder stores: Option<Option<String>>
//         // - None               → field default (None)
//         // - Some(None)         → explicit None via custom setter (if you add one)
//         // - Some(Some(value))  → value provided
//         let email: Option<String> = match &self.email {
//             Some(v) => v.clone(),
//             None => None, // #[builder(default)]
//         };

//         // dob: required (custom setter)
//         let dob: chrono::DateTime<chrono::Utc> = self
//             .dob
//             .clone()
//             .ok_or_else(|| derive_builder::UninitializedFieldError::new("dob"))?;

//         // age: #[builder(setter(skip), default)]
//         // Builder stores Option<u32>; default uses Default::default() -> 0
//         let age: u32 = self.age.clone().unwrap_or_default();

//         // skills: #[builder(default = "vec![]", setter(each = "skill"))]
//         let skills: Vec<String> = self
//             .skills
//             .clone()
//             .unwrap_or_else(|| vec![]);

//         Ok(User { name, email, dob, age, skills })
//     }

//     // Examples of generated setters (conceptual):

//     // name: #[builder(setter(into))]
//     pub fn name(&mut self, v: impl Into<String>) -> &mut Self {
//         self.name = Some(v.into());
//         self
//     }

//     // skills: #[builder(default, setter(each(name = "skill", into)))]
//     pub fn skill(&mut self, v: impl Into<String>) -> &mut Self {
//         self.skills.get_or_insert_with(Default::default).push(v.into());
//         self
//     }
// }
