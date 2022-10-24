// run-rustfix
// aux-build:macro_rules.rs

#![feature(custom_inner_attributes)]
#![warn(clippy::manual_rem_euclid)]

#[macro_use]
extern crate macro_rules;

macro_rules! internal_rem_euclid {
    () => {
        let value: i32 = 5;
        let _: i32 = ((value % 4) + 4) % 4;
    };
}

fn main() {
    let value: i32 = 5;

    let _: i32 = ((value % 4) + 4) % 4;
    let _: i32 = (4 + (value % 4)) % 4;
    let _: i32 = (value % 4 + 4) % 4;
    let _: i32 = (4 + value % 4) % 4;
    let _: i32 = 1 + (4 + value % 4) % 4;

    let _: i32 = (3 + value % 4) % 4;
    let _: i32 = (-4 + value % -4) % -4;
    let _: i32 = ((5 % 4) + 4) % 4;

    // Make sure the lint does not trigger if it would cause an error, like with an ambiguous
    // integer type
    let not_annotated = 24;
    let _ = ((not_annotated % 4) + 4) % 4;
    let inferred: _ = 24;
    let _ = ((inferred % 4) + 4) % 4;

    // For lint to apply the constant must always be on the RHS of the previous value for %
    let _: i32 = 4 % ((value % 4) + 4);
    let _: i32 = ((4 % value) + 4) % 4;

    // Lint in internal macros
    internal_rem_euclid!();

    // Do not lint in external macros
    manual_rem_euclid!();
}

// Should lint for params too
pub fn rem_euclid_4(num: i32) -> i32 {
    ((num % 4) + 4) % 4
}

// Constant version came later, should still lint
pub const fn const_rem_euclid_4(num: i32) -> i32 {
    ((num % 4) + 4) % 4
}

pub fn msrv_1_37() {
    #![clippy::msrv = "1.37"]

    let x: i32 = 10;
    let _: i32 = ((x % 4) + 4) % 4;
}

pub fn msrv_1_38() {
    #![clippy::msrv = "1.38"]

    let x: i32 = 10;
    let _: i32 = ((x % 4) + 4) % 4;
}

// For const fns:
pub const fn msrv_1_51() {
    #![clippy::msrv = "1.51"]

    let x: i32 = 10;
    let _: i32 = ((x % 4) + 4) % 4;
}

pub const fn msrv_1_52() {
    #![clippy::msrv = "1.52"]

    let x: i32 = 10;
    let _: i32 = ((x % 4) + 4) % 4;
}
