// Copyright 2017 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![feature(conservative_impl_trait)]

pub fn g() -> impl Iterator<Item=u8> {
    Some(1u8).into_iter()
}

pub fn h() -> (impl Iterator<Item=u8>) {
    Some(1u8).into_iter()
}

pub fn i() -> impl Iterator<Item=u8> + 'static {
    Some(1u8).into_iter()
}

pub fn j() -> impl Iterator<Item=u8> + Clone {
    Some(1u8).into_iter()
}

pub fn k() -> [impl Clone; 2] {
    [123u32, 456u32]
}

pub fn l() -> (impl Clone, impl Default) {
    (789u32, -123i32)
}

pub fn m() -> &'static impl Clone {
    &1u8
}

pub fn n() -> *const impl Clone {
    &1u8
}

pub fn o() -> &'static [impl Clone] {
    b":)"
}

// issue #44731
pub fn test_44731_0() -> Box<impl Iterator<Item=u8>> {
    Box::new(g())
}

pub fn test_44731_1() -> Result<Box<impl Clone>, ()> {
    Ok(Box::new(j()))
}


pub fn test_44731_3() -> Box<Fn() -> impl Clone> {
    Box::new(|| 0u32)
}

pub fn test_44731_4() -> Box<Iterator<Item=impl Clone>> {
    Box::new(g())
}

// @has issue_43869/fn.g.html
// @has issue_43869/fn.h.html
// @has issue_43869/fn.i.html
// @has issue_43869/fn.j.html
// @has issue_43869/fn.k.html
// @has issue_43869/fn.l.html
// @has issue_43869/fn.m.html
// @has issue_43869/fn.n.html
// @has issue_43869/fn.o.html
// @has issue_43869/fn.test_44731_0.html
// @has issue_43869/fn.test_44731_1.html
// @has issue_43869/fn.test_44731_3.html
// @has issue_43869/fn.test_44731_4.html
