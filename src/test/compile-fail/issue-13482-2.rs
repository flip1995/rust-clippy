// Copyright 2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// compile-flags:-Z verbose

#![feature(slice_patterns)]

fn main() {
    let x = [1,2];
    let y = match x {
        [] => None,
//~^ ERROR mismatched types
//~| expected `[_#1i; 2]`
//~| found `[_#7t; 0]`
//~| expected an array with a fixed size of 2 elements
//~| found one with 0 elements
        [a,_] => Some(a)
    };
}
