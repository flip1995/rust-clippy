// Copyright 2016 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// aux-build:extern-statics.rs

extern crate extern_statics;
use extern_statics::*;

extern {
    static mut B: u8;
}

fn main() {
    let b = B; //~ ERROR use of mutable static is unsafe
    let rb = &B; //~ ERROR use of mutable static is unsafe
    let xb = XB; //~ ERROR use of mutable static is unsafe
    let xrb = &XB; //~ ERROR use of mutable static is unsafe
}
