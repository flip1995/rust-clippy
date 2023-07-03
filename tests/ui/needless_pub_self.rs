//@run-rustfix
//@aux-build:proc_macros.rs:proc-macro
#![feature(custom_inner_attributes)]
#![allow(unused)]
#![warn(clippy::needless_pub_self)]
#![no_main]
#![rustfmt::skip] // rustfmt will remove `in`, understandable
                  // but very annoying for our purposes!

#[macro_use]
extern crate proc_macros;

pub(self) fn a() {}
pub(in self) fn b() {}

pub fn c() {}
mod a {
    pub(in super) fn d() {}
    pub(super) fn e() {}
    pub(self) fn f() {}
}

external! {
    pub(self) fn g() {}
    pub(in self) fn h() {}
}
with_span! {
    span
    pub(self) fn i() {}
    pub(in self) fn j() {}
}

// not really anything more to test. just a really simple lint overall
