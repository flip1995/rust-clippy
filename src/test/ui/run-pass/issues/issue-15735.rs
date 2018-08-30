// Copyright 2015 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// run-pass
struct A<'a> {
    a: &'a i32,
    b: &'a i32,
}

impl <'a> A<'a> {
    fn foo<'b>(&'b self) {
        A {
            a: self.a,
            b: self.b,
        };
    }
}

fn main() { }
