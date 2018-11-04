// Copyright 2018 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// compile-pass

#![crate_type = "lib"]
#![feature(linkage)]

// MergeFunctions will merge these via an anonymous internal
// backing function, which must be named if ThinLTO buffers are used

#[linkage = "weak"]
pub fn fn1(a: u32, b: u32, c: u32) -> u32 {
    a + b + c
}

#[linkage = "weak"]
pub fn fn2(a: u32, b: u32, c: u32) -> u32 {
    a + b + c
}
