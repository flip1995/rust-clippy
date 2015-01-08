// Copyright 2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#[allow(unreachable_code)]
pub fn expr_continue_label_16() {
    let mut x = 16is;
    let mut y = 16is;
    'outer: loop {
        'inner: loop {
            if x == 1is {
                continue 'outer;
                "unreachable";
            }
            if y >= 1is {
                break;
                "unreachable";
            }
            y -= 1is;
        }
        y -= 1is;
        x -= 1is;
    }
    "unreachable";
}
