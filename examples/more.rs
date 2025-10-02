use anyhow::Result;
use derive_more::{Add, Display, From, Into};

#[derive(PartialEq, Clone, Copy, From, Add, Into, Display)]
struct MyInt(i32);
// struct MyInt(i32); wraps an i32.
// Derives:
// PartialEq, Clone, Copy: plain value semantics.
// From: implements From<i32> for MyInt, so 10.into() -> MyInt(10).
// Into: implements Into<i32> for MyInt, so MyInt(10).into() -> 10.
// Add: implements std::ops::Add<MyInt> for MyInt by delegating to inner i32, so MyInt(10) + MyInt(20) -> MyInt(30).
// Display: prints the inner value (delegates to i32’s Display), so format!("{my_int}") shows the number.

// “Plain value semantics” means the type behaves like a small numeric value (e.g., i32) rather than an owned resource or handle.

// PartialEq: enables ==/!= that compare the contained values.
// For MyInt(i32), equality is based on the inner i32.
// Clone: allows explicit duplication via .clone().
// For simple value types, Clone is cheap (just copies the bits).
// Copy: allows implicit, bitwise copies on assignment, passing, returns.
// After let b = a;, both a and b are usable and independent.
// Copy implies Clone and forbids Drop (no custom destruction).

// let a = MyInt(10);
// let b = a;              // Copy, not move (a is still usable)
// assert!(a == b);        // PartialEq
// let c = a.clone();      // Same as Copy for this type
// let sum = a + b;        // Add from derive_more
// println!("{a}, {b}, {c}, {sum}");

#[derive(PartialEq, From)]
struct Point2D {
    x: i32,
    y: i32,
}
// Derives:
// PartialEq for comparisons.
// From: implements From<(i32, i32)> for Point2D, so (1, 2).into() -> Point2D { x: 1, y: 2 }.

// Here’s how PartialEq works for Point2D: two values are equal only if all fields are equal (x and y). It enables ==, != and APIs like Vec::contains and iter().position that rely on equality.
// --- PartialEq examples for Point2D ---
// let p1 = Point2D { x: 1, y: 2 };
// let p2: Point2D = (1, 2).into(); // uses From<(i32, i32)>
// let p3 = Point2D { x: 2, y: 3 };

// assert!(p1 == p2);        // equal: both fields match
// assert!(p1 != p3);        // not equal: fields differ

// let pts = vec![p1, p3];
// assert!(pts.contains(&p2));                   // uses PartialEq under the hood
// assert!(pts.iter().position(|p| *p == p3).is_some());

#[derive(Debug, PartialEq, From, Add, Display)]
enum MyEnum {
    #[display("int: {_0}")]
    Int(i32),
    Uint(u32),
    #[display("nothing")]
    Nothing,
}
// Derives:
// Debug, PartialEq: for debugging/compare.
// From: creates From<i32> -> MyEnum::Int and From<u32> -> MyEnum::Uint, so 10i32.into() == MyEnum::Int(10).
// Display:
// #[display("int: {_0}")] customizes Int to print “int: <value>”.
// #[display("nothing")] prints “nothing” for Nothing.
// Uint has no Display attr here; if you Display it, you’ll get a default or a compile error depending on derive_more rules. You’re using Debug for printing enums, so it’s fine.
// Note: deriving Add for an enum usually fails (mixed variants). If you see errors, remove Add from MyEnum or implement Add manually.

// “Uint” in your code isn’t a type; it’s an enum variant name.
// MyEnum::Uint(u32) wraps a u32 (unsigned 32‑bit integer).
// u32 range: 0..=4_294_967_295. No negative values.
// Rust has no primitive named uint; use u8, u16, u32, u64, u128, or usize.

fn main() -> Result<()> {
    let my_int: MyInt = 10.into();
    let v = my_int + 20.into();
    let v1: i32 = v.into();

    println!("my_int: {}, v: {}, v1: {}", my_int, v, v1);

    let e: MyEnum = 10i32.into();
    let e1: MyEnum = 20u32.into();
    let e2 = MyEnum::Nothing;
    println!("e: {:?}, e1: {:?}, e2: {:?}", e, e1, e2);

    Ok(())
}
// let my_int: MyInt = 10.into(); uses From<i32> for MyInt.
// let v = my_int + 20.into(); adds two MyInt values (Add impl).
// let v1: i32 = v.into(); converts MyInt to i32 (Into).
// Prints MyInt, v (MyInt), and v1 (i32). MyInt uses Display, so numbers print nicely.
// let e: MyEnum = 10i32.into(); -> MyEnum::Int(10)
// let e1: MyEnum = 20u32.into(); -> MyEnum::Uint(20)
// let e2 = MyEnum::Nothing;
// Prints enums with {:?} (Debug).

// Tip:
// If Add for MyEnum causes a compile error, drop it (it’s not used in main).
