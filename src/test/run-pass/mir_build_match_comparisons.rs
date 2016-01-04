// Copyright 2015 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![feature(rustc_attrs)]

#[rustc_mir]
pub fn test1(x: i8) -> i32 {
  match x {
    1...10 => 0,
    _ => 1,
  }
}

const U: Option<i8> = Some(10);
const S: &'static str = "hello";

#[rustc_mir]
pub fn test2(x: i8) -> i32 {
  match Some(x) {
    U => 0,
    _ => 1,
  }
}

#[rustc_mir]
pub fn test3(x: &'static str) -> i32 {
  match x {
    S => 0,
    _ => 1,
  }
}

fn main() {
  assert_eq!(test1(0), 1);
  assert_eq!(test1(1), 0);
  assert_eq!(test1(2), 0);
  assert_eq!(test1(5), 0);
  assert_eq!(test1(9), 0);
  assert_eq!(test1(10), 0);
  assert_eq!(test1(11), 1);
  assert_eq!(test1(20), 1);
  assert_eq!(test2(10), 0);
  assert_eq!(test2(0), 1);
  assert_eq!(test2(20), 1);
  assert_eq!(test3("hello"), 0);
  assert_eq!(test3(""), 1);
  assert_eq!(test3("world"), 1);
}
