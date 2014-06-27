// Copyright 2012 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

fn test_if_fail() {
    let x = if false { fail!() } else { 10i };
    assert!((x == 10));
}

fn test_else_fail() {
    let x = if true { 10i } else { fail!() };
    assert_eq!(x, 10i);
}

fn test_elseif_fail() {
    let x = if false { 0i } else if false { fail!() } else { 10i };
    assert_eq!(x, 10i);
}

pub fn main() { test_if_fail(); test_else_fail(); test_elseif_fail(); }
