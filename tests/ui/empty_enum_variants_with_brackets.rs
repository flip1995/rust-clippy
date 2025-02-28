#![warn(clippy::empty_enum_variants_with_brackets)]
#![allow(dead_code)]

pub enum PublicTestEnum {
    NonEmptyBraces { x: i32, y: i32 }, // No error
    NonEmptyParentheses(i32, i32),     // No error
    EmptyBraces {},
    //~^ empty_enum_variants_with_brackets
    EmptyParentheses(),
    //~^ empty_enum_variants_with_brackets
}

enum TestEnum {
    NonEmptyBraces { x: i32, y: i32 }, // No error
    NonEmptyParentheses(i32, i32),     // No error
    EmptyBraces {},
    //~^ empty_enum_variants_with_brackets
    EmptyParentheses(),
    //~^ empty_enum_variants_with_brackets
    AnotherEnum, // No error
}

enum TestEnumWithFeatures {
    NonEmptyBraces {
        #[cfg(feature = "thisisneverenabled")]
        x: i32,
    }, // No error
    NonEmptyParentheses(#[cfg(feature = "thisisneverenabled")] i32), // No error
}

fn main() {}
