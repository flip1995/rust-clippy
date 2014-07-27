// Copyright 2012 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.


use std::gc::{Gc, GC};

fn borrow<'r,T>(x: &'r T) -> &'r T {x}

struct Rec { f: Gc<int> }

pub fn main() {
    let rec = box(GC) Rec {f: box(GC) 22};
    while *borrow(&*rec.f) == 23 {}
}
