// Copyright 2012 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#[pkgid="rust_get_test_int"];
// NOTE: remove after the next snapshot
#[link(name = "rust_get_test_int")];

mod rustrt {
    use std::libc;

    extern {
        pub fn rust_get_test_int() -> libc::intptr_t;
    }
}

pub fn main() { }
