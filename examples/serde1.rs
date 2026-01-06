// Overview
// This code creates a User struct with various field serialization customizations, including encrypted sensitive data, base64-encoded binary data, and custom enum serialization.
// Key Components
// Dependencies

// serde/serde_json: Serialization framework
// chacha20poly1305: Encryption cipher
// base64: Binary-to-text encoding
// chrono: Date/time handling
// http: URI parsing

// 1. High-level intent
// This program demonstrates how to:
// Model a rich domain object (User)
// Serialize/deserialize it to JSON using Serde
// Apply custom field-level transformations:
// Base64 encoding for binary data
// Authenticated encryption (ChaCha20-Poly1305) for sensitive strings
// Reuse Rust traits (Display, FromStr) to integrate encryption cleanly into Serde via serde_with
// The design goal is transparent security: encryption happens automatically during serialization, decryption during deserialization, without special handling in application code.

use core::fmt;
use std::str::FromStr;

use anyhow::Result;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use chacha20poly1305::{
    aead::{Aead, OsRng},
    AeadCore, ChaCha20Poly1305, KeyInit,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DisplayFromStr};

const KEY: &[u8] = b"01234567890123456789012345678901";
// ChaCha20Poly1305 requires exactly 32 bytes (256 bits) for the key.
// This is a cryptographic requirement, not optional. ChaCha20 algorithm is designed to accept exactly 256-bit keys; the nonce must be 96 bits (12 bytes, which you see in ChaCha20Poly1305::generate_nonce() and decoded[..12]).

// Example: hash user password to 32 bytes
// use sha2::{Sha256, Digest};

// let password = "my-secret";
// let mut hasher = Sha256::new();
// hasher.update(password.as_bytes());
// let key: [u8; 32] = hasher.finalize().into(); // Exactly 32 bytes

#[serde_as]
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct User {
    name: String, // Standard field
    #[serde(rename = "privateAge")]
    age: u8, // Renamed in JSON to "privateAge"
    date_of_birth: DateTime<Utc>, // ISO 8601 datetime
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    skills: Vec<String>, // Skipped if empty
    state: WorkState, // Tagged enum
    #[serde(serialize_with = "b64_encode", deserialize_with = "b64_decode")]
    data: Vec<u8>, // Custom base64 encoding
    // #[serde(
    //     serialize_with = "serialize_encrypt",
    //     deserialize_with = "deserialize_decrypt"
    // )]
    #[serde_as(as = "DisplayFromStr")]
    sensitive: SensitiveData, // Encrypted via Display/FromStr
    #[serde_as(as = "Vec<DisplayFromStr>")]
    url: Vec<http::Uri>, // Parsed URIs via Display/FromStr
}

// Vec::is_empty: A method on Vec<T> that returns true if the vector has length 0.
// In the serde attribute, it’s a function path used as a predicate to skip serializing the field when it’s empty.

// default: A serde attribute that tells deserialization to use Default::default() for the field if it’s missing in the input. For Vec<T>, Default::default() is an empty vec.
// When serializing: skills is omitted if skills.is_empty() == true.
// When deserializing: if JSON has no "skills", serde sets skills = Vec::default() (i.e., []).

// serialize_with, deserialize_with
// they are overrides, not fallbacks.
// serialize_with = "path": Always calls that function to serialize this field, replacing Serde’s default logic.
// deserialize_with = "path": Always calls that function to parse this field, replacing the default.
// If the function errors, serialization/deserialization fails; Serde does not “fall back” to the built-in behavior.
// // Tip: #[serde(with = "module_path")] is a shorthand that uses a module providing serialize/deserialize (and friends) for both directions.
// DisplayFromStr is a zero-sized adapter type from the serde_with crate. It’s not a trait; it’s a marker struct used with #[serde_as] to tell Serde to:

// serialize a value via its Display implementation
// deserialize a value via its FromStr implementation
// Requirements for the field type T:

// T: core::fmt::Display + std::str::FromStr
// It can be applied to a single value or per-element inside containers:

// #[serde_as(as = "DisplayFromStr")] for a single T
// #[serde_as(as = "Vec<DisplayFromStr>")] for Vec<T> where T: Display + FromStr

#[derive(Debug)]
struct SensitiveData(String);

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "type", content = "details")] // WorkState is defined with Serde’s tagging attributes
enum WorkState {
    Working(String),
    OnLeave(DateTime<Utc>),
    Terminated,
}

// WorkState is defined with Serde’s tagging attributes.

// tag = "type" adds a discriminator field in JSON.
// content = "details" places the variant’s payload there.
// rename_all = "camelCase" camel-cases the variant names.
// So the struct field state: WorkState serializes as an adjacently tagged enum (a discriminated union). Examples:

// Working("Rust Engineer") → {"type":"working","details":"Rust Engineer"}
// OnLeave(DateTime<Utc>) → {"type":"onLeave","details":"2025-12-13T13:41:00Z"}
// Terminated → {"type":"terminated"}

fn main() -> Result<()> {
    // let state = WorkState::Working("Rust Egineer".to_string());
    let state1 = WorkState::OnLeave(Utc::now());
    let user = User {
        name: "Alice".to_string(),
        age: 30,
        date_of_birth: Utc::now(),
        skills: vec!["Rust".to_string(), "Python".to_string()],
        state: state1,
        data: vec![1, 2, 3, 4, 5],
        sensitive: SensitiveData::new("secret"),
        url: vec!["https://example.com".parse()?],
    };

    let json = serde_json::to_string(&user)?;
    println!("{}", json);

    let user1: User = serde_json::from_str(&json)?;
    println!("{:?}", user1);
    println!("{:?}", user1.url[0].host()); // url: [https://example.com/] -> Some("example.com")

    Ok(())
}

fn b64_encode<S>(data: &Vec<u8>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    let encoded = URL_SAFE_NO_PAD.encode(data);
    serializer.serialize_str(&encoded)
}

fn b64_decode<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let encoded = String::deserialize(deserializer)?;
    let decoded = URL_SAFE_NO_PAD
        .decode(encoded.as_bytes())
        .map_err(serde::de::Error::custom)?;
    Ok(decoded)
}

#[allow(dead_code)]
fn serialize_encrypt<S>(data: &str, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    let encrypted = encrypt(data.as_bytes()).map_err(serde::ser::Error::custom)?;
    serializer.serialize_str(&encrypted)
}

#[allow(dead_code)]
fn deserialize_decrypt<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let encrypted = String::deserialize(deserializer)?;
    let decrypted = decrypt(&encrypted).map_err(serde::de::Error::custom)?;
    let decrypted = String::from_utf8(decrypted).map_err(serde::de::Error::custom)?;
    Ok(decrypted)
}

/// encrypt with chacha20poly1305 and then encode with base64
fn encrypt(data: &[u8]) -> Result<String> {
    let cipher = ChaCha20Poly1305::new(KEY.into());
    let nonce = ChaCha20Poly1305::generate_nonce(&mut OsRng); // 96-bits; unique per message
    let ciphertext = cipher.encrypt(&nonce, data).unwrap();
    let nonce_cypertext: Vec<_> = nonce.iter().copied().chain(ciphertext).collect();
    let encoded = URL_SAFE_NO_PAD.encode(nonce_cypertext);
    Ok(encoded)
}

// Step by step:

// nonce.iter() – Creates an iterator over the nonce bytes

// nonce is type Nonce (a 96-bit/12-byte wrapper)
// iter() yields &u8 references to each byte
// .copied() – Converts borrowed bytes to owned values

// Transforms &u8 → u8
// Now you have an iterator yielding owned u8 values
// .chain(ciphertext) – Concatenates two iterators

// Takes the nonce bytes (12 bytes) and appends the ciphertext bytes
// ciphertext is a Vec<u8> from the encryption operation
// Order: [nonce_bytes... | ciphertext_bytes...]
// .collect() – Gathers all values into a single collection

// Collects into Vec<u8> (from the type annotation Vec<_>)
// Result: One combined vector with nonce + ciphertext

// An iterator is NOT a collection—it's a lazy object that produces values one at a time. Think of it like a vending machine that dispenses items on demand, not a box of pre-made items.
// After .copied(), the type is something like Copied<std::slice::Iter<'_, u8>> (the exact type is complex), but what matters is: it yields u8 values one at a time, not a collection.

// Before .copied()
// nonce.iter()        // Iterator yielding &u8, &u8, &u8, ... (borrowed)
// After .copied()
// .copied()           // Iterator yielding u8, u8, u8, ... (owned, copied values)

// The iterator doesn't store all values in memory—it generates them as you consume them. Only when you call .collect() does it create an actual collection.
// Is the final Vec a two-element vector?
// No! It's a Vec<u8> with many individual byte elements, not two elements.

// If nonce is 12 bytes and ciphertext is 16 bytes:
// nonce_ciphertext: Vec<u8> = vec![
// First 12 elements (nonce bytes):
//     42, 99, 187, 44, 12, 7, 200, 111, 55, 88, 22, 5,
// Next 16 elements (ciphertext bytes):
//     100, 200, 50, 75, 150, 25, 88, 99, 33, 44, 55, 66, 77, 88, 99, 110
// ]

// Total: 28 elements, each a u8 byte value
// Length: nonce_ciphertext.len() == 28

// The iterator chain process:

// nonce.iter()           →  yields: &42, &99, &187, ...
//   .copied()            →  yields: 42, 99, 187, ...
//     .chain(ciphertext) →  yields: 42, 99, 187, ..., 100, 200, 50, ...
//       .collect()       →  Vec<u8> = [42, 99, 187, ..., 100, 200, 50, ...]

// So the final vector is a flat list of individual bytes, not a two-element container. The vector just concatenates all bytes sequentially.

/// decode with base64 and then decrypt with chacha20poly1305
fn decrypt(encoded: &str) -> Result<Vec<u8>> {
    let decoded = URL_SAFE_NO_PAD.decode(encoded.as_bytes())?;
    let cipher = ChaCha20Poly1305::new(KEY.into());
    let nonce = decoded[..12].into();
    let decrypted = cipher.decrypt(nonce, &decoded[12..]).unwrap();
    Ok(decrypted)
}

impl fmt::Display for SensitiveData {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let encrypted = encrypt(self.0.as_bytes()).unwrap();
        write!(f, "{}", encrypted)
    }
}

impl FromStr for SensitiveData {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        let decrypted = decrypt(s)?;
        let decrypted = String::from_utf8(decrypted)?;
        Ok(Self(decrypted))
    }
}

impl SensitiveData {
    fn new(data: impl Into<String>) -> Self {
        Self(data.into())
    }
}

// Here is the conceptual expansion of your code.

// The compiler applies the macros in two stages:

// #[serde_as]: Transforms the AST (Abstract Syntax Tree) to replace serde_as attributes with standard serde attributes (using with = "...").
// #[derive(Serialize, Deserialize)]: Generates the actual trait implementations.
// Here is what the expanded code looks like (simplified for readability):
//
// impl Serialize for User {
//     fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
//     where
//         S: serde::Serializer,
//     {
//         // 1. Start the struct.
//         // Note: 8 fields total.
//         let mut state = serializer.serialize_struct("User", 8)?;

//         // 2. Field: name (Standard)
//         // "camelCase" doesn't change "name"
//         state.serialize_field("name", &self.name)?;

//         // 3. Field: age (Renamed)
//         // Uses "privateAge" as defined in attribute
//         state.serialize_field("privateAge", &self.age)?;

//         // 4. Field: date_of_birth (Standard + camelCase)
//         // Becomes "dateOfBirth"
//         state.serialize_field("dateOfBirth", &self.date_of_birth)?;

//         // 5. Field: skills (Skip if empty)
//         if !Vec::is_empty(&self.skills) {
//             state.serialize_field("skills", &self.skills)?;
//         }

//         // 6. Field: state (Standard)
//         state.serialize_field("state", &self.state)?;

//         // 7. Field: data (serialize_with = "b64_encode")
//         // Serde creates a temporary wrapper to satisfy the type system
//         {
//             struct B64EncodeAdapter<'a>(&'a Vec<u8>);

//             impl<'a> Serialize for B64EncodeAdapter<'a> {
//                 fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
//                 where S: serde::Serializer
//                 {
//                     // Calls YOUR function
//                     b64_encode(self.0, serializer)
//                 }
//             }
//             state.serialize_field("data", &B64EncodeAdapter(&self.data))?;
//         }

//         // 8. Field: sensitive (serde_as / DisplayFromStr)
//         // serde_as converts this to a "with" call pointing to serde_with internals
//         {
//             struct SensitiveAdapter<'a>(&'a SensitiveData);
//             impl<'a> Serialize for SensitiveAdapter<'a> {
//                 fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
//                 where S: serde::Serializer
//                 {
//                     // serde_with internal logic calling Display::fmt
//                     serde_with::As::<DisplayFromStr>::serialize(self.0, serializer)
//                 }
//             }
//             state.serialize_field("sensitive", &SensitiveAdapter(&self.sensitive))?;
//         }

//         // 9. Field: url (serde_as / Vec<DisplayFromStr>)
//         {
//             struct UrlAdapter<'a>(&'a Vec<http::Uri>);
//             impl<'a> Serialize for UrlAdapter<'a> {
//                 fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
//                 where S: serde::Serializer
//                 {
//                     serde_with::As::<Vec<DisplayFromStr>>::serialize(self.0, serializer)
//                 }
//             }
//             state.serialize_field("url", &UrlAdapter(&self.url))?;
//         }

//         state.end()
//     }
// }

// 2. The Deserialize Implementation
// Deserialization is more complex because it uses the Visitor Pattern. It defines an internal Visitor struct that iterates over the JSON map.

// impl<'de> Deserialize<'de> for User {
//     fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
//     where
//         D: serde::Deserializer<'de>,
//     {
//         // 1. Define the Field enum for efficient key matching
//         enum Field { Name, Age, DateOfBirth, Skills, State, Data, Sensitive, Url, Ignore }

//         impl<'de> Deserialize<'de> for Field {
//             // Logic to match strings "privateAge", "dateOfBirth" to enum variants
//             // ...
//         }

//         // 2. Define the Visitor
//         struct UserVisitor;

//         impl<'de> serde::de::Visitor<'de> for UserVisitor {
//             type Value = User;

//             fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
//                 formatter.write_str("struct User")
//             }

//             // The core logic for parsing a JSON object (Map)
//             fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
//             where
//                 A: serde::de::MapAccess<'de>,
//             {
//                 // Variables to hold values as we find them
//                 let mut name = None;
//                 let mut age = None;
//                 let mut date_of_birth = None;
//                 let mut skills = None;
//                 let mut state = None;
//                 let mut data = None;
//                 let mut sensitive = None;
//                 let mut url = None;

//                 // Loop through JSON keys
//                 while let Some(key) = map.next_key::<Field>()? {
//                     match key {
//                         Field::Name => {
//                             if name.is_some() { return Err(/* duplicate field error */); }
//                             name = Some(map.next_value()?);
//                         }
//                         Field::Age => { // Matches "privateAge"
//                             age = Some(map.next_value()?);
//                         }
//                         Field::DateOfBirth => { // Matches "dateOfBirth"
//                             date_of_birth = Some(map.next_value()?);
//                         }
//                         Field::Skills => {
//                             skills = Some(map.next_value()?);
//                         }
//                         Field::State => {
//                             state = Some(map.next_value()?);
//                         }
//                         Field::Data => {
//                             // Calls YOUR b64_decode function
//                             // Serde creates a wrapper to adapt the function signature
//                             data = Some(b64_decode(serde::de::value::MapAccessDeserializer::new(map))?);
//                             // Note: In reality, it uses a seed/wrapper, but conceptually it invokes your function
//                         }
//                         Field::Sensitive => {
//                             // Calls serde_with internal deserialize (FromStr)
//                             sensitive = Some(serde_with::As::<DisplayFromStr>::deserialize(
//                                 serde::de::value::MapAccessDeserializer::new(map)
//                             )?);
//                         }
//                         Field::Url => {
//                             url = Some(serde_with::As::<Vec<DisplayFromStr>>::deserialize(
//                                 serde::de::value::MapAccessDeserializer::new(map)
//                             )?);
//                         }
//                         Field::Ignore => {
//                             let _ = map.next_value::<serde::de::IgnoredAny>()?;
//                         }
//                     }
//                 }

//                 // 3. Construct the final struct
//                 let name = name.ok_or_else(|| serde::de::Error::missing_field("name"))?;
//                 let age = age.ok_or_else(|| serde::de::Error::missing_field("privateAge"))?;
//                 let date_of_birth = date_of_birth.ok_or_else(|| serde::de::Error::missing_field("dateOfBirth"))?;

//                 // Handle default for skills
//                 let skills = skills.unwrap_or_else(Vec::default);

//                 let state = state.ok_or_else(|| serde::de::Error::missing_field("state"))?;
//                 let data = data.ok_or_else(|| serde::de::Error::missing_field("data"))?;
//                 let sensitive = sensitive.ok_or_else(|| serde::de::Error::missing_field("sensitive"))?;
//                 let url = url.ok_or_else(|| serde::de::Error::missing_field("url"))?;

//                 Ok(User {
//                     name,
//                     age,
//                     date_of_birth,
//                     skills,
//                     state,
//                     data,
//                     sensitive,
//                     url,
//                 })
//             }
//         }

//         // 3. Kick off the process
//         const FIELDS: &'static [&'static str] = &["name", "privateAge", "dateOfBirth", "skills", "state", "data", "sensitive",impl<'de> Deserialize<'de> for User {
//     fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
//     where
//         D: serde::Deserializer<'de>,
//     {
//         // 1. Define the Field enum for efficient key matching
//         enum Field { Name, Age, DateOfBirth, Skills, State, Data, Sensitive, Url, Ignore }

//         impl<'de> Deserialize<'de> for Field {
//             // Logic to match strings "privateAge", "dateOfBirth" to enum variants
//             // ...
//         }

//         // 2. Define the Visitor
//         struct UserVisitor;

//         impl<'de> serde::de::Visitor<'de> for UserVisitor {
//             type Value = User;

//             fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
//                 formatter.write_str("struct User")
//             }

//             // The core logic for parsing a JSON object (Map)
//             fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
//             where
//                 A: serde::de::MapAccess<'de>,
//             {
//                 // Variables to hold values as we find them
//                 let mut name = None;
//                 let mut age = None;
//                 let mut date_of_birth = None;
//                 let mut skills = None;
//                 let mut state = None;
//                 let mut data = None;
//                 let mut sensitive = None;
//                 let mut url = None;

//                 // Loop through JSON keys
//                 while let Some(key) = map.next_key::<Field>()? {
//                     match key {
//                         Field::Name => {
//                             if name.is_some() { return Err(/* duplicate field error */); }
//                             name = Some(map.next_value()?);
//                         }
//                         Field::Age => { // Matches "privateAge"
//                             age = Some(map.next_value()?);
//                         }
//                         Field::DateOfBirth => { // Matches "dateOfBirth"
//                             date_of_birth = Some(map.next_value()?);
//                         }
//                         Field::Skills => {
//                             skills = Some(map.next_value()?);
//                         }
//                         Field::State => {
//                             state = Some(map.next_value()?);
//                         }
//                         Field::Data => {
//                             // Calls YOUR b64_decode function
//                             // Serde creates a wrapper to adapt the function signature
//                             data = Some(b64_decode(serde::de::value::MapAccessDeserializer::new(map))?);
//                             // Note: In reality, it uses a seed/wrapper, but conceptually it invokes your function
//                         }
//                         Field::Sensitive => {
//                             // Calls serde_with internal deserialize (FromStr)
//                             sensitive = Some(serde_with::As::<DisplayFromStr>::deserialize(
//                                 serde::de::value::MapAccessDeserializer::new(map)
//                             )?);
//                         }
//                         Field::Url => {
//                             url = Some(serde_with::As::<Vec<DisplayFromStr>>::deserialize(
//                                 serde::de::value::MapAccessDeserializer::new(map)
//                             )?);
//                         }
//                         Field::Ignore => {
//                             let _ = map.next_value::<serde::de::IgnoredAny>()?;
//                         }
//                     }
//                 }

//                 // 3. Construct the final struct
//                 let name = name.ok_or_else(|| serde::de::Error::missing_field("name"))?;
//                 let age = age.ok_or_else(|| serde::de::Error::missing_field("privateAge"))?;
//                 let date_of_birth = date_of_birth.ok_or_else(|| serde::de::Error::missing_field("dateOfBirth"))?;

//                 // Handle default for skills
//                 let skills = skills.unwrap_or_else(Vec::default);

//                 let state = state.ok_or_else(|| serde::de::Error::missing_field("state"))?;
//                 let data = data.ok_or_else(|| serde::de::Error::missing_field("data"))?;
//                 let sensitive = sensitive.ok_or_else(|| serde::de::Error::missing_field("sensitive"))?;
//                 let url = url.ok_or_else(|| serde::de::Error::missing_field("url"))?;

//                 Ok(User {
//                     name,
//                     age,
//                     date_of_birth,
//                     skills,
//                     state,
//                     data,
//                     sensitive,
//                     url,
//                 })
//             }
//         }

//         // 3. Kick off the process
//         const FIELDS: &'static [&'static str] = &["name", "privateAge", "dateOfBirth", "skills", "state", "data", "sensitive",
