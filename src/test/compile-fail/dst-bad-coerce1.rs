// Copyright 2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// Attempt to change the type as well as unsizing.

struct Fat<Sized? T> {
    ptr: T
}

struct Foo;
trait Bar {}

pub fn main() {
    // With a vec of ints.
    let f1 = Fat { ptr: [1, 2, 3] };
    let f2: &Fat<[int; 3]> = &f1;
    let f3: &Fat<[uint]> = f2;
    //~^ ERROR mismatched types: expected `&Fat<[uint]>`, found `&Fat<[int; 3]>`

    // With a trait.
    let f1 = Fat { ptr: Foo };
    let f2: &Fat<Foo> = &f1;
    let f3: &Fat<Bar> = f2;
    //~^ ERROR the trait `Bar` is not implemented for the type `Foo`
}
