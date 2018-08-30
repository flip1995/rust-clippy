// Copyright 2016 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// run-pass
// ignore-wasm32-bare compiled with panic=abort by default
// compile-flags: -C debug_assertions=yes

use std::panic;

fn main() {
    let r = panic::catch_unwind(|| {
        let mut it = u8::max_value()..;
        it.next().unwrap(); // 255
        it.next().unwrap();
    });
    assert!(r.is_err());

    let r = panic::catch_unwind(|| {
        let mut it = i8::max_value()..;
        it.next().unwrap(); // 127
        it.next().unwrap();
    });
    assert!(r.is_err());
}
