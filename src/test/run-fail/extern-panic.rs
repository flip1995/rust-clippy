// Copyright 2012-2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// ignore-test linked failure
// error-pattern:explicit failure
// Testing that runtime failure doesn't cause callbacks to abort abnormally.
// Instead the failure will be delivered after the callbacks return.

extern crate libc;
use std::task;

mod rustrt {
    extern crate libc;

    extern {
        pub fn rust_dbg_call(cb: *u8, data: libc::uintptr_t)
                             -> libc::uintptr_t;
    }
}

extern fn cb(data: libc::uintptr_t) -> libc::uintptr_t {
    if data == 1_usize {
        data
    } else {
        count(data - 1_usize) + count(data - 1_usize)
    }
}

fn count(n: uint) -> uint {
    unsafe {
        task::deschedule();
        rustrt::rust_dbg_call(cb, n)
    }
}

fn main() {
    for _ in 0..10_usize {
        task::spawn(move|| {
            let result = count(5_usize);
            println!("result = %?", result);
            panic!();
        });
    }
}
