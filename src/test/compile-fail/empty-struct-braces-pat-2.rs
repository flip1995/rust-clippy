// Copyright 2015 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// Can't use empty braced struct as enum pattern

// aux-build:empty-struct.rs

#![feature(relaxed_adts)]

extern crate empty_struct;
use empty_struct::*;

struct Empty1 {}

fn main() {
    let e1 = Empty1 {};
    let xe1 = XEmpty1 {};

    match e1 {
        Empty1() => () //~ ERROR unresolved tuple struct/variant `Empty1`
    }
    match xe1 {
        XEmpty1() => () //~ ERROR unresolved tuple struct/variant `XEmpty1`
    }
    match e1 {
        Empty1(..) => () //~ ERROR unresolved tuple struct/variant `Empty1`
    }
    match xe1 {
        XEmpty1(..) => () //~ ERROR unresolved tuple struct/variant `XEmpty1`
    }
}
