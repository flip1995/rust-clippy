// Copyright 2018 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// compile-flags: -C no-prepopulate-passes
// `#[no_mangle]`d functions always have external linkage, i.e. no `internal` in their `define`s

#![crate_type = "lib"]
#![no_std]

// CHECK: define void @a()
#[no_mangle]
fn a() {}

// CHECK: define void @b()
#[no_mangle]
pub fn b() {}

mod private {
    // CHECK: define void @c()
    #[no_mangle]
    fn c() {}

    // CHECK: define void @d()
    #[no_mangle]
    pub fn d() {}
}

const HIDDEN: () = {
    // CHECK: define void @e()
    #[no_mangle]
    fn e() {}

    // CHECK: define void @f()
    #[no_mangle]
    pub fn f() {}
};

// The surrounding item should not accidentally become external
// CHECK: define internal{{.*}} void @_ZN22external_no_mangle_fns1x
#[inline(never)]
fn x() {
    // CHECK: define void @g()
    #[no_mangle]
    fn g() {
        x();
    }

    // CHECK: define void @h()
    #[no_mangle]
    pub fn h() {}

    // side effect to keep `x` around
    unsafe {
        core::ptr::read_volatile(&42);
    }
}
