// Copyright 2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// Test that Copy bounds inherited by trait are checked.

use std::any::Any;
use std::any::AnyRefExt;

trait Foo : Copy {
}

impl<T:Copy> Foo for T {
}

fn take_param<T:Foo>(foo: &T) { }

fn a() {
    let x = box 3i;
    take_param(&x); //~ ERROR `core::kinds::Copy` is not implemented
}

fn b() {
    let x = box 3i;
    let y = &x;
    let z = &x as &Foo; //~ ERROR `core::kinds::Copy` is not implemented
}

fn main() { }
