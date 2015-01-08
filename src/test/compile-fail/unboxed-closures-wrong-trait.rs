// Copyright 2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![feature(lang_items, overloaded_calls, unboxed_closures)]

fn c<F:Fn(isize, isize) -> isize>(f: F) -> isize {
    f(5, 6)
}

fn main() {
    let z: isize = 7;
    assert_eq!(c(|&mut: x: isize, y| x + y + z), 10);
    //~^ ERROR not implemented
}

