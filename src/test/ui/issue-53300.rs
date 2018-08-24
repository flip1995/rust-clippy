// Copyright 2012-2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// issue 53300

pub trait A {
    fn add(&self, b: i32) -> i32;
}

fn addition() -> Wrapper<impl A> {}
//~^ ERROR cannot find type `Wrapper` in this scope [E0412]

fn main() {
    let res = addition();
}
