#![warn(clippy::doc_markdown)]

//! A comment with a_b(x) and a_c in it and (a_b((c)) ) too and (maybe a_b((c)))
//~^ ERROR: item in documentation is missing backticks
//~| ERROR: item in documentation is missing backticks
//~| ERROR: item in documentation is missing backticks
//~| ERROR: item in documentation is missing backticks

pub fn main() {}
