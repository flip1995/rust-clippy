// Copyright 2012 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

pub fn main() {
  let f = 1_usize as *const String;
  println!("{:?}", f as isize);
  println!("{:?}", f as usize);
  println!("{:?}", f as i8);
  println!("{:?}", f as i16);
  println!("{:?}", f as i32);
  println!("{:?}", f as i64);
  println!("{:?}", f as u8);
  println!("{:?}", f as u16);
  println!("{:?}", f as u32);
  println!("{:?}", f as u64);

  println!("{:?}", 1 as isize);
  println!("{:?}", 1 as usize);
  println!("{:?}", 1 as *const String);
  println!("{:?}", 1 as i8);
  println!("{:?}", 1 as i16);
  println!("{:?}", 1 as i32);
  println!("{:?}", 1 as i64);
  println!("{:?}", 1 as u8);
  println!("{:?}", 1 as u16);
  println!("{:?}", 1 as u32);
  println!("{:?}", 1 as u64);
  println!("{:?}", 1 as f32);
  println!("{:?}", 1 as f64);

  println!("{:?}", 1_usize as isize);
  println!("{:?}", 1_usize as usize);
  println!("{:?}", 1_usize as *const String);
  println!("{:?}", 1_usize as i8);
  println!("{:?}", 1_usize as i16);
  println!("{:?}", 1_usize as i32);
  println!("{:?}", 1_usize as i64);
  println!("{:?}", 1_usize as u8);
  println!("{:?}", 1_usize as u16);
  println!("{:?}", 1_usize as u32);
  println!("{:?}", 1_usize as u64);
  println!("{:?}", 1_usize as f32);
  println!("{:?}", 1_usize as f64);

  println!("{:?}", 1i8 as isize);
  println!("{:?}", 1i8 as usize);
  println!("{:?}", 1i8 as *const String);
  println!("{:?}", 1i8 as i8);
  println!("{:?}", 1i8 as i16);
  println!("{:?}", 1i8 as i32);
  println!("{:?}", 1i8 as i64);
  println!("{:?}", 1i8 as u8);
  println!("{:?}", 1i8 as u16);
  println!("{:?}", 1i8 as u32);
  println!("{:?}", 1i8 as u64);
  println!("{:?}", 1i8 as f32);
  println!("{:?}", 1i8 as f64);

  println!("{:?}", 1u8 as isize);
  println!("{:?}", 1u8 as usize);
  println!("{:?}", 1u8 as *const String);
  println!("{:?}", 1u8 as i8);
  println!("{:?}", 1u8 as i16);
  println!("{:?}", 1u8 as i32);
  println!("{:?}", 1u8 as i64);
  println!("{:?}", 1u8 as u8);
  println!("{:?}", 1u8 as u16);
  println!("{:?}", 1u8 as u32);
  println!("{:?}", 1u8 as u64);
  println!("{:?}", 1u8 as f32);
  println!("{:?}", 1u8 as f64);

  println!("{:?}", 1i16 as isize);
  println!("{:?}", 1i16 as usize);
  println!("{:?}", 1i16 as *const String);
  println!("{:?}", 1i16 as i8);
  println!("{:?}", 1i16 as i16);
  println!("{:?}", 1i16 as i32);
  println!("{:?}", 1i16 as i64);
  println!("{:?}", 1i16 as u8);
  println!("{:?}", 1i16 as u16);
  println!("{:?}", 1i16 as u32);
  println!("{:?}", 1i16 as u64);
  println!("{:?}", 1i16 as f32);
  println!("{:?}", 1i16 as f64);

  println!("{:?}", 1u16 as isize);
  println!("{:?}", 1u16 as usize);
  println!("{:?}", 1u16 as *const String);
  println!("{:?}", 1u16 as i8);
  println!("{:?}", 1u16 as i16);
  println!("{:?}", 1u16 as i32);
  println!("{:?}", 1u16 as i64);
  println!("{:?}", 1u16 as u8);
  println!("{:?}", 1u16 as u16);
  println!("{:?}", 1u16 as u32);
  println!("{:?}", 1u16 as u64);
  println!("{:?}", 1u16 as f32);
  println!("{:?}", 1u16 as f64);

  println!("{:?}", 1i32 as isize);
  println!("{:?}", 1i32 as usize);
  println!("{:?}", 1i32 as *const String);
  println!("{:?}", 1i32 as i8);
  println!("{:?}", 1i32 as i16);
  println!("{:?}", 1i32 as i32);
  println!("{:?}", 1i32 as i64);
  println!("{:?}", 1i32 as u8);
  println!("{:?}", 1i32 as u16);
  println!("{:?}", 1i32 as u32);
  println!("{:?}", 1i32 as u64);
  println!("{:?}", 1i32 as f32);
  println!("{:?}", 1i32 as f64);

  println!("{:?}", 1u32 as isize);
  println!("{:?}", 1u32 as usize);
  println!("{:?}", 1u32 as *const String);
  println!("{:?}", 1u32 as i8);
  println!("{:?}", 1u32 as i16);
  println!("{:?}", 1u32 as i32);
  println!("{:?}", 1u32 as i64);
  println!("{:?}", 1u32 as u8);
  println!("{:?}", 1u32 as u16);
  println!("{:?}", 1u32 as u32);
  println!("{:?}", 1u32 as u64);
  println!("{:?}", 1u32 as f32);
  println!("{:?}", 1u32 as f64);

  println!("{:?}", 1i64 as isize);
  println!("{:?}", 1i64 as usize);
  println!("{:?}", 1i64 as *const String);
  println!("{:?}", 1i64 as i8);
  println!("{:?}", 1i64 as i16);
  println!("{:?}", 1i64 as i32);
  println!("{:?}", 1i64 as i64);
  println!("{:?}", 1i64 as u8);
  println!("{:?}", 1i64 as u16);
  println!("{:?}", 1i64 as u32);
  println!("{:?}", 1i64 as u64);
  println!("{:?}", 1i64 as f32);
  println!("{:?}", 1i64 as f64);

  println!("{:?}", 1u64 as isize);
  println!("{:?}", 1u64 as usize);
  println!("{:?}", 1u64 as *const String);
  println!("{:?}", 1u64 as i8);
  println!("{:?}", 1u64 as i16);
  println!("{:?}", 1u64 as i32);
  println!("{:?}", 1u64 as i64);
  println!("{:?}", 1u64 as u8);
  println!("{:?}", 1u64 as u16);
  println!("{:?}", 1u64 as u32);
  println!("{:?}", 1u64 as u64);
  println!("{:?}", 1u64 as f32);
  println!("{:?}", 1u64 as f64);

  println!("{:?}", 1u64 as isize);
  println!("{:?}", 1u64 as usize);
  println!("{:?}", 1u64 as *const String);
  println!("{:?}", 1u64 as i8);
  println!("{:?}", 1u64 as i16);
  println!("{:?}", 1u64 as i32);
  println!("{:?}", 1u64 as i64);
  println!("{:?}", 1u64 as u8);
  println!("{:?}", 1u64 as u16);
  println!("{:?}", 1u64 as u32);
  println!("{:?}", 1u64 as u64);
  println!("{:?}", 1u64 as f32);
  println!("{:?}", 1u64 as f64);

  println!("{:?}", true as isize);
  println!("{:?}", true as usize);
  println!("{:?}", true as i8);
  println!("{:?}", true as i16);
  println!("{:?}", true as i32);
  println!("{:?}", true as i64);
  println!("{:?}", true as u8);
  println!("{:?}", true as u16);
  println!("{:?}", true as u32);
  println!("{:?}", true as u64);

  println!("{:?}", 1f32 as isize);
  println!("{:?}", 1f32 as usize);
  println!("{:?}", 1f32 as i8);
  println!("{:?}", 1f32 as i16);
  println!("{:?}", 1f32 as i32);
  println!("{:?}", 1f32 as i64);
  println!("{:?}", 1f32 as u8);
  println!("{:?}", 1f32 as u16);
  println!("{:?}", 1f32 as u32);
  println!("{:?}", 1f32 as u64);
  println!("{:?}", 1f32 as f32);
  println!("{:?}", 1f32 as f64);

  println!("{:?}", 1f64 as isize);
  println!("{:?}", 1f64 as usize);
  println!("{:?}", 1f64 as i8);
  println!("{:?}", 1f64 as i16);
  println!("{:?}", 1f64 as i32);
  println!("{:?}", 1f64 as i64);
  println!("{:?}", 1f64 as u8);
  println!("{:?}", 1f64 as u16);
  println!("{:?}", 1f64 as u32);
  println!("{:?}", 1f64 as u64);
  println!("{:?}", 1f64 as f32);
  println!("{:?}", 1f64 as f64);
}
