// Copyright 2013-2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// aux-build:issue_3907.rs
extern crate issue_3907;

type Foo = issue_3907::Foo;

struct S {
    name: isize
}

impl Foo for S { //~ ERROR: `Foo` is not a trait
                 //~| NOTE: expected trait, found type alias
                 //~| NOTE: type aliases cannot be used for traits
    fn bar() { }
}

fn main() {
    let s = S {
        name: 0
    };
    s.bar();
}
