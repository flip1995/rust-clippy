// Copyright 2018 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// This test checks that instantiating an uninhabited type via `mem::{uninitialized,zeroed}` results
// in a runtime panic.

#![feature(never_type)]

use std::{mem, panic};

struct Foo {
    x: u8,
    y: !,
}

fn main() {
    unsafe {
        panic::catch_unwind(|| mem::uninitialized::<!>()).is_err();
        panic::catch_unwind(|| mem::zeroed::<!>()).is_err();

        panic::catch_unwind(|| mem::uninitialized::<Foo>()).is_err();
        panic::catch_unwind(|| mem::zeroed::<Foo>()).is_err();
    }
}
