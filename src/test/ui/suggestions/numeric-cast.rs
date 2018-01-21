// Copyright 2018 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.


fn foo<N>(_x: N) {}

fn main() {
    let x_usize: usize = 1;
    let x_u64: u64 = 2;
    let x_u32: u32 = 3;
    let x_u16: u16 = 4;
    let x_u8: u8 = 5;
    let x_isize: isize = 6;
    let x_i64: i64 = 7;
    let x_i32: i32 = 8;
    let x_i16: i16 = 9;
    let x_i8: i8 = 10;
    let x_f64: f64 = 11.0;
    let x_f32: f32 = 12.0;

    foo::<usize>(x_usize);
    foo::<usize>(x_u64);
    //~^ ERROR mismatched types
    foo::<usize>(x_u32);
    //~^ ERROR mismatched types
    foo::<usize>(x_u16);
    //~^ ERROR mismatched types
    foo::<usize>(x_u8);
    //~^ ERROR mismatched types
    foo::<usize>(x_isize);
    //~^ ERROR mismatched types
    foo::<usize>(x_i64);
    //~^ ERROR mismatched types
    foo::<usize>(x_i32);
    //~^ ERROR mismatched types
    foo::<usize>(x_i16);
    //~^ ERROR mismatched types
    foo::<usize>(x_i8);
    //~^ ERROR mismatched types
    foo::<usize>(x_f64);
    //~^ ERROR mismatched types
    foo::<usize>(x_f32);
    //~^ ERROR mismatched types

    foo::<isize>(x_usize);
    //~^ ERROR mismatched types
    foo::<isize>(x_u64);
    //~^ ERROR mismatched types
    foo::<isize>(x_u32);
    //~^ ERROR mismatched types
    foo::<isize>(x_u16);
    //~^ ERROR mismatched types
    foo::<isize>(x_u8);
    //~^ ERROR mismatched types
    foo::<isize>(x_isize);
    foo::<isize>(x_i64);
    //~^ ERROR mismatched types
    foo::<isize>(x_i32);
    //~^ ERROR mismatched types
    foo::<isize>(x_i16);
    //~^ ERROR mismatched types
    foo::<isize>(x_i8);
    //~^ ERROR mismatched types
    foo::<isize>(x_f64);
    //~^ ERROR mismatched types
    foo::<isize>(x_f32);
    //~^ ERROR mismatched types

    foo::<u64>(x_usize);
    //~^ ERROR mismatched types
    foo::<u64>(x_u64);
    foo::<u64>(x_u32);
    //~^ ERROR mismatched types
    foo::<u64>(x_u16);
    //~^ ERROR mismatched types
    foo::<u64>(x_u8);
    //~^ ERROR mismatched types
    foo::<u64>(x_isize);
    //~^ ERROR mismatched types
    foo::<u64>(x_i64);
    //~^ ERROR mismatched types
    foo::<u64>(x_i32);
    //~^ ERROR mismatched types
    foo::<u64>(x_i16);
    //~^ ERROR mismatched types
    foo::<u64>(x_i8);
    //~^ ERROR mismatched types
    foo::<u64>(x_f64);
    //~^ ERROR mismatched types
    foo::<u64>(x_f32);
    //~^ ERROR mismatched types

    foo::<i64>(x_usize);
    //~^ ERROR mismatched types
    foo::<i64>(x_u64);
    //~^ ERROR mismatched types
    foo::<i64>(x_u32);
    //~^ ERROR mismatched types
    foo::<i64>(x_u16);
    //~^ ERROR mismatched types
    foo::<i64>(x_u8);
    //~^ ERROR mismatched types
    foo::<i64>(x_isize);
    //~^ ERROR mismatched types
    foo::<i64>(x_i64);
    foo::<i64>(x_i32);
    //~^ ERROR mismatched types
    foo::<i64>(x_i16);
    //~^ ERROR mismatched types
    foo::<i64>(x_i8);
    //~^ ERROR mismatched types
    foo::<i64>(x_f64);
    //~^ ERROR mismatched types
    foo::<i64>(x_f32);
    //~^ ERROR mismatched types

    foo::<u32>(x_usize);
    //~^ ERROR mismatched types
    foo::<u32>(x_u64);
    //~^ ERROR mismatched types
    foo::<u32>(x_u32);
    foo::<u32>(x_u16);
    //~^ ERROR mismatched types
    foo::<u32>(x_u8);
    //~^ ERROR mismatched types
    foo::<u32>(x_isize);
    //~^ ERROR mismatched types
    foo::<u32>(x_i64);
    //~^ ERROR mismatched types
    foo::<u32>(x_i32);
    //~^ ERROR mismatched types
    foo::<u32>(x_i16);
    //~^ ERROR mismatched types
    foo::<u32>(x_i8);
    //~^ ERROR mismatched types
    foo::<u32>(x_f64);
    //~^ ERROR mismatched types
    foo::<u32>(x_f32);
    //~^ ERROR mismatched types

    foo::<i32>(x_usize);
    //~^ ERROR mismatched types
    foo::<i32>(x_u64);
    //~^ ERROR mismatched types
    foo::<i32>(x_u32);
    //~^ ERROR mismatched types
    foo::<i32>(x_u16);
    //~^ ERROR mismatched types
    foo::<i32>(x_u8);
    //~^ ERROR mismatched types
    foo::<i32>(x_isize);
    //~^ ERROR mismatched types
    foo::<i32>(x_i64);
    //~^ ERROR mismatched types
    foo::<i32>(x_i32);
    foo::<i32>(x_i16);
    //~^ ERROR mismatched types
    foo::<i32>(x_i8);
    //~^ ERROR mismatched types
    foo::<i32>(x_f64);
    //~^ ERROR mismatched types
    foo::<i32>(x_f32);
    //~^ ERROR mismatched types

    foo::<u16>(x_usize);
    //~^ ERROR mismatched types
    foo::<u16>(x_u64);
    //~^ ERROR mismatched types
    foo::<u16>(x_u32);
    //~^ ERROR mismatched types
    foo::<u16>(x_u16);
    foo::<u16>(x_u8);
    //~^ ERROR mismatched types
    foo::<u16>(x_isize);
    //~^ ERROR mismatched types
    foo::<u16>(x_i64);
    //~^ ERROR mismatched types
    foo::<u16>(x_i32);
    //~^ ERROR mismatched types
    foo::<u16>(x_i16);
    //~^ ERROR mismatched types
    foo::<u16>(x_i8);
    //~^ ERROR mismatched types
    foo::<u16>(x_f64);
    //~^ ERROR mismatched types
    foo::<u16>(x_f32);
    //~^ ERROR mismatched types

    foo::<i16>(x_usize);
    //~^ ERROR mismatched types
    foo::<i16>(x_u64);
    //~^ ERROR mismatched types
    foo::<i16>(x_u32);
    //~^ ERROR mismatched types
    foo::<i16>(x_u16);
    //~^ ERROR mismatched types
    foo::<i16>(x_u8);
    //~^ ERROR mismatched types
    foo::<i16>(x_isize);
    //~^ ERROR mismatched types
    foo::<i16>(x_i64);
    //~^ ERROR mismatched types
    foo::<i16>(x_i32);
    //~^ ERROR mismatched types
    foo::<i16>(x_i16);
    foo::<i16>(x_i8);
    //~^ ERROR mismatched types
    foo::<i16>(x_f64);
    //~^ ERROR mismatched types
    foo::<i16>(x_f32);
    //~^ ERROR mismatched types

    foo::<u8>(x_usize);
    //~^ ERROR mismatched types
    foo::<u8>(x_u64);
    //~^ ERROR mismatched types
    foo::<u8>(x_u32);
    //~^ ERROR mismatched types
    foo::<u8>(x_u16);
    //~^ ERROR mismatched types
    foo::<u8>(x_u8);
    foo::<u8>(x_isize);
    //~^ ERROR mismatched types
    foo::<u8>(x_i64);
    //~^ ERROR mismatched types
    foo::<u8>(x_i32);
    //~^ ERROR mismatched types
    foo::<u8>(x_i16);
    //~^ ERROR mismatched types
    foo::<u8>(x_i8);
    //~^ ERROR mismatched types
    foo::<u8>(x_f64);
    //~^ ERROR mismatched types
    foo::<u8>(x_f32);
    //~^ ERROR mismatched types

    foo::<i8>(x_usize);
    //~^ ERROR mismatched types
    foo::<i8>(x_u64);
    //~^ ERROR mismatched types
    foo::<i8>(x_u32);
    //~^ ERROR mismatched types
    foo::<i8>(x_u16);
    //~^ ERROR mismatched types
    foo::<i8>(x_u8);
    //~^ ERROR mismatched types
    foo::<i8>(x_isize);
    //~^ ERROR mismatched types
    foo::<i8>(x_i64);
    //~^ ERROR mismatched types
    foo::<i8>(x_i32);
    //~^ ERROR mismatched types
    foo::<i8>(x_i16);
    //~^ ERROR mismatched types
    foo::<i8>(x_i8);
    foo::<i8>(x_f64);
    //~^ ERROR mismatched types
    foo::<i8>(x_f32);
    //~^ ERROR mismatched types

    foo::<f64>(x_usize);
    //~^ ERROR mismatched types
    foo::<f64>(x_u64);
    //~^ ERROR mismatched types
    foo::<f64>(x_u32);
    //~^ ERROR mismatched types
    foo::<f64>(x_u16);
    //~^ ERROR mismatched types
    foo::<f64>(x_u8);
    //~^ ERROR mismatched types
    foo::<f64>(x_isize);
    //~^ ERROR mismatched types
    foo::<f64>(x_i64);
    //~^ ERROR mismatched types
    foo::<f64>(x_i32);
    //~^ ERROR mismatched types
    foo::<f64>(x_i16);
    //~^ ERROR mismatched types
    foo::<f64>(x_i8);
    //~^ ERROR mismatched types
    foo::<f64>(x_f64);
    foo::<f64>(x_f32);
    //~^ ERROR mismatched types

    foo::<f32>(x_usize);
    //~^ ERROR mismatched types
    foo::<f32>(x_u64);
    //~^ ERROR mismatched types
    foo::<f32>(x_u32);
    //~^ ERROR mismatched types
    foo::<f32>(x_u16);
    //~^ ERROR mismatched types
    foo::<f32>(x_u8);
    //~^ ERROR mismatched types
    foo::<f32>(x_isize);
    //~^ ERROR mismatched types
    foo::<f32>(x_i64);
    //~^ ERROR mismatched types
    foo::<f32>(x_i32);
    //~^ ERROR mismatched types
    foo::<f32>(x_i16);
    //~^ ERROR mismatched types
    foo::<f32>(x_i8);
    //~^ ERROR mismatched types
    foo::<f32>(x_f64);
    //~^ ERROR mismatched types
    foo::<f32>(x_f32);
}
