// Strum is a utility crate that adds derive macros and helpers for enums (and a few for structs). It saves you from hand-writing boilerplate like Display, FromStr, iterators, etc.

use anyhow::Result;
use serde::Serialize;
use strum::{
    Display, EnumCount, EnumDiscriminants, EnumIs, EnumIter, EnumString, IntoEnumIterator,
    IntoStaticStr, VariantNames,
};

#[derive(Display, Debug, Serialize)]
#[allow(unused)]
enum Color {
    #[strum(serialize = "redred", to_string = "bright red")] // to_string 优先于 serialize
    Red,
    Green {
        range: usize,
    },
    Blue(usize),
    Yellow,
    #[strum(to_string = "purple with {sat} saturation")]
    Purple {
        sat: usize,
    },
}

#[derive(
    Debug, EnumString, EnumCount, EnumDiscriminants, EnumIter, EnumIs, IntoStaticStr, VariantNames,
)]
#[allow(unused)]
enum MyEnum {
    A,
    B(String),
    C,
}

fn main() -> Result<()> {
    // MyEnum::VARIANTS.iter().for_each(|v| println!("{:?}", v));
    println!("{:?}", MyEnum::VARIANTS);
    MyEnum::iter().for_each(|v| println!("{:?}", v));
    println!("MyEnum has {} variants", MyEnum::COUNT);

    let my_enum = MyEnum::B("hello".to_string());
    println!("{:?}", my_enum.is_b());

    let s: &'static str = my_enum.into();
    println!("{}", s);

    let red = Color::Red;
    let green = Color::Green { range: 10 };
    let blue = Color::Blue(20);
    let yellow = Color::Yellow;
    let purple = Color::Purple { sat: 30 };
    println!(
        "red: {}, green: {}, blue: {}, yellow: {}, purple: {}",
        red, green, blue, yellow, purple
    );
    let red_str = serde_json::to_string(&red)?;
    println!("red json: {}", red_str);
    Ok(())
}

// ["A", "B", "C"]
// A
// B("")
// C
// MyEnum has 3 variants
// true
// B
// red: bright red, green: Green, blue: Blue, yellow: Yellow, purple: purple with 30 saturation
// red json: "Red"

// What your derives do

// Display: implements fmt::Display for the enum/variant. Customize with attributes:
// Per-variant: #[strum(to_string = "purple with {sat} saturation")]
// Aliases: #[strum(serialize = "redred")] (also used by EnumString)
// Placeholders use field names or indices: {_0}, {sat}
// EnumString: impl FromStr for unit variants. Use attributes:
// #[strum(serialize = "b", serialize = "bee")]
// #[strum(ascii_case_insensitive)]
// #[strum(serialize_all = "kebab_case")] on the enum
// Note: Only unit variants are parsable; data variants (like B(String)) are ignored or should be #[strum(disabled)].
// EnumIter + IntoEnumIterator: adds MyEnum::iter() over all variants (unit and data variants yield just the variant “shape,” not every payload).
// EnumCount: adds const COUNT: usize.
// VariantNames: adds const VARIANTS: &'static [&'static str] with variant names.
// EnumIs: generates is_a(), is_b(), etc.
// IntoStaticStr: impl Into<&'static str> returning the variant name (data is ignored).
// EnumDiscriminants: generates a parallel enum with the same variant set but without payloads; you can derive traits on it via #[strum_discriminants(derive(Hash, Eq, …))].
// Handy attributes

// On enum: #[strum(serialize_all = "snake_case" | "kebab_case" | "SCREAMING_SNAKE_CASE", ascii_case_insensitive)]
// On variant: #[strum(serialize = "alias1", serialize = "alias2", to_string = "...", disabled)]
