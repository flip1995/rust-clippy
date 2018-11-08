// Copyright 2016 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// aux-build:two_macros.rs

// compile-pass
#![allow(unused)]

fn f() {
    let _ = macro_one!();
}
#[macro_use(macro_one)] // Check that this macro is usable in the above function
extern crate two_macros;

fn g() {
    macro_two!();
}
macro_rules! m { () => {
    #[macro_use(macro_two)] // Check that this macro is usable in the above function
    extern crate two_macros as _two_macros;
} }
m!();


fn main() {}
