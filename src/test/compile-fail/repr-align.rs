// Copyright 2017 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
#![allow(dead_code)]
#![feature(attr_literals)]

#[repr(align(16.0))] //~ ERROR: align representation must be a u16 power of two
struct A(i32);

#[repr(align(15))] //~ ERROR: align representation must be a u16 power of two
struct B(i32);

#[repr(align(65536))] //~ ERROR: align representation must be a u16 power of tw
struct C(i32);

fn main() {}
