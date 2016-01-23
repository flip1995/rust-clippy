// Copyright 2016 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.


macro_rules! foo {
    ($e:expr) => { $e.foo() }
    //~^ ERROR no method named `foo` found for type `i32` in the current scope
}

fn main() {
    let a = 1i32;
    foo!(a);

    foo!(1i32.foo());
    //~^ ERROR attempted access of field `i32` on type `_`, but no field with that name was found
}
