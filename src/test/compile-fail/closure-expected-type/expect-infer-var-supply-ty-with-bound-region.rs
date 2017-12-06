// Copyright 2016 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// must-compile-successfully

fn with_closure<F, A>(_: F)
    where F: FnOnce(A, &u32)
{
}

fn foo() {
    // This version works; we infer `A` to be `u32`, and take the type
    // of `y` to be `&u32`.
    with_closure(|x: u32, y| {});
}

fn bar() {
    // This version also works.
    with_closure(|x: &u32, y| {});
}

fn main() { }
