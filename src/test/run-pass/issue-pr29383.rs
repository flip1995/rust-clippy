// Copyright 2015 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![allow(match_of_unit_variant_via_paren_dotdot)]

enum E {
    A,
    B,
}

fn main() {
    match None {
        None => {}
        Some(E::A(..)) => {}
        Some(E::B(..)) => {}
    }
}
