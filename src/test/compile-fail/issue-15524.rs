// Copyright 2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

const N: isize = 1;

enum Foo {
    A = 1,
    //~^ NOTE first use
    //~| NOTE first use
    //~| NOTE first use
    B = 1, //~ ERROR discriminant value
    //~^ NOTE enum already
    C = 0,
    D, //~ ERROR discriminant value
    //~^ NOTE enum already

    E = N, //~ ERROR discriminant value
    //~^ NOTE enum already

}

fn main() {}
