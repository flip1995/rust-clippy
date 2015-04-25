// Copyright 2013 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
#![feature(no_std)]
#![no_std]


mod sub1 {
    fn foo() {} // implementation 1
}

mod sub2 {
    fn foo() {} // implementation 2
}

use sub1::foo; //~ note previous import of `foo` here
use sub2::foo; //~ error a value named `foo` has already been imported in this module [e0252]

fn main() {}
