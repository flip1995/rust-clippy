// Copyright 2016 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

fn main() {
    let x = Some(0);

    match x {
        Some(y) | None => {} //~  ERROR variable `y` is not bound in all patterns
        _ => ()              //~| NOTE pattern doesn't bind `y`
                             //~| NOTE variable not in all patterns
    }
}
