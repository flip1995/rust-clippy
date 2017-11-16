// Copyright 2017 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// compile-flags: -Z lower_128bit_ops -C debug_assertions=yes

#![feature(i128_type)]
#![feature(lang_items)]

#[lang="i128_div"]
fn i128_div(_x: i128, _y: i128) -> i128 { 3 }
#[lang="u128_div"]
fn u128_div(_x: i128, _y: i128) -> i128 { 4 }
#[lang="i128_rem"]
fn i128_rem(_x: i128, _y: i128) -> i128 { 5 }
#[lang="u128_rem"]
fn u128_rem(_x: i128, _y: i128) -> i128 { 6 }

#[lang="i128_addo"]
fn i128_addo(_x: i128, _y: i128) -> (i128, bool) { (0, false) }
#[lang="u128_addo"]
fn u128_addo(_x: i128, _y: i128) -> (i128, bool) { (1, false) }
#[lang="i128_subo"]
fn i128_subo(_x: i128, _y: i128) -> (i128, bool) { (2, false) }
#[lang="u128_subo"]
fn u128_subo(_x: i128, _y: i128) -> (i128, bool) { (3, false) }
#[lang="i128_mulo"]
fn i128_mulo(_x: i128, _y: i128) -> (i128, bool) { (4, false) }
#[lang="u128_mulo"]
fn u128_mulo(_x: i128, _y: i128) -> (i128, bool) { (5, false) }
#[lang="i128_shlo"]
fn i128_shlo(_x: i128, _y: u32) -> (i128, bool) { (6, false) }
#[lang="i128_shro"]
fn i128_shro(_x: i128, _y: u32) -> (i128, bool) { (7, false) }
#[lang="u128_shro"]
fn u128_shro(_x: i128, _y: u32) -> (i128, bool) { (8, false) }


fn test_signed(mut x: i128) -> i128 {
    x += 1;
    x -= 2;
    x *= 3;
    x /= 4;
    x %= 5;
    x <<= 6;
    x >>= 7;
    x
}

fn test_unsigned(mut x: u128) -> u128 {
    x += 1;
    x -= 2;
    x *= 3;
    x /= 4;
    x %= 5;
    x <<= 6;
    x >>= 7;
    x
}

fn main() {
    test_signed(-200);
    test_unsigned(200);
}

// END RUST SOURCE

// START rustc.test_signed.Lower128Bit.after.mir
//     _2 = const i128_addo(_1, const 1i128) -> bb10;
//     ...
//     _3 = const i128_subo(_1, const 2i128) -> bb11;
//     ...
//     _4 = const i128_mulo(_1, const 3i128) -> bb12;
//     ...
//     _1 = const i128_div(_1, const 4i128) -> bb13;
//     ...
//     _1 = const i128_rem(_1, const 5i128) -> bb15;
//     ...
//     _14 = const i128_shro(_1, const 7i32) -> bb16;
//     ...
//     _13 = const i128_shlo(_1, const 6i32) -> bb14;
// END rustc.test_signed.Lower128Bit.after.mir

// START rustc.test_unsigned.Lower128Bit.after.mir
//     _2 = const u128_addo(_1, const 1u128) -> bb8;
//     ...
//     _3 = const u128_subo(_1, const 2u128) -> bb9;
//     ...
//     _4 = const u128_mulo(_1, const 3u128) -> bb10;
//     ...
//     _1 = const u128_div(_1, const 4u128) -> bb11;
//     ...
//     _1 = const u128_rem(_1, const 5u128) -> bb13;
//     ...
//     _8 = const u128_shro(_1, const 7i32) -> bb14;
//     ...
//     _7 = const i128_shlo(_1, const 6i32) -> bb12;
// END rustc.test_unsigned.Lower128Bit.after.mir
