// Copyright 2012 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.


trait double {
    fn double(self: Box<Self>) -> uint;
}

impl double for Box<uint> {
    fn double(self: Box<Box<uint>>) -> uint { **self * 2u }
}

pub fn main() {
    let x = box box box box box 3u;
    assert_eq!(x.double(), 6u);
}
