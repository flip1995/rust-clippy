// Copyright 2012 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

pub fn main() {

    let mut y: int = 42;
    let mut z: int = 42;
    let mut x: int;
    while z < 50 {
        z += 1;
        while false { x = move y; y = z; }
        log(debug, y);
    }
    assert (y == 42 && z == 50);
}
