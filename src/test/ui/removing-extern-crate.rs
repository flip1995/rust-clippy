// edition:2018
// aux-build:removing-extern-crate.rs
// run-rustfix
// compile-pass

#![warn(rust_2018_idioms)]
#![allow(unused_imports)]

extern crate removing_extern_crate as foo;
extern crate core;

mod another {
    extern crate removing_extern_crate as foo;
    extern crate core;
}

fn main() {}
