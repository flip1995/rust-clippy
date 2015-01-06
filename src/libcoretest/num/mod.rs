// Copyright 2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use core::cmp::PartialEq;
use core::fmt::Show;
use core::num::{NumCast, cast};
use core::ops::{Add, Sub, Mul, Div, Rem};
use core::marker::Copy;

#[cfg_attr(stage0, macro_escape)]
#[cfg_attr(not(stage0), macro_use)]
mod int_macros;

mod i8;
mod i16;
mod i32;
mod i64;
mod int;

#[cfg_attr(stage0, macro_escape)]
#[cfg_attr(not(stage0), macro_use)]
mod uint_macros;

mod u8;
mod u16;
mod u32;
mod u64;
mod uint;

/// Helper function for testing numeric operations
pub fn test_num<T>(ten: T, two: T) where
    T: PartialEq + NumCast
     + Add<Output=T> + Sub<Output=T>
     + Mul<Output=T> + Div<Output=T>
     + Rem<Output=T> + Show
     + Copy
{
    assert_eq!(ten.add(two),  cast(12i).unwrap());
    assert_eq!(ten.sub(two),  cast(8i).unwrap());
    assert_eq!(ten.mul(two),  cast(20i).unwrap());
    assert_eq!(ten.div(two),  cast(5i).unwrap());
    assert_eq!(ten.rem(two),  cast(0i).unwrap());

    assert_eq!(ten.add(two),  ten + two);
    assert_eq!(ten.sub(two),  ten - two);
    assert_eq!(ten.mul(two),  ten * two);
    assert_eq!(ten.div(two),  ten / two);
    assert_eq!(ten.rem(two),  ten % two);
}

#[cfg(test)]
mod test {
    use core::option::Option;
    use core::option::Option::{Some, None};
    use core::num::Float;
    use core::num::from_str_radix;

    #[test]
    fn from_str_issue7588() {
        let u : Option<u8> = from_str_radix("1000", 10);
        assert_eq!(u, None);
        let s : Option<i16> = from_str_radix("80000", 10);
        assert_eq!(s, None);
        let f : Option<f32> = from_str_radix("10000000000000000000000000000000000000000", 10);
        assert_eq!(f, Some(Float::infinity()));
        let fe : Option<f32> = from_str_radix("1e40", 10);
        assert_eq!(fe, Some(Float::infinity()));
    }

    #[test]
    fn test_from_str_radix_float() {
        let x1 : Option<f64> = from_str_radix("-123.456", 10);
        assert_eq!(x1, Some(-123.456));
        let x2 : Option<f32> = from_str_radix("123.456", 10);
        assert_eq!(x2, Some(123.456));
        let x3 : Option<f32> = from_str_radix("-0.0", 10);
        assert_eq!(x3, Some(-0.0));
        let x4 : Option<f32> = from_str_radix("0.0", 10);
        assert_eq!(x4, Some(0.0));
        let x4 : Option<f32> = from_str_radix("1.0", 10);
        assert_eq!(x4, Some(1.0));
        let x5 : Option<f32> = from_str_radix("-1.0", 10);
        assert_eq!(x5, Some(-1.0));
    }

    #[test]
    fn test_int_from_str_overflow() {
        let mut i8_val: i8 = 127_i8;
        assert_eq!("127".parse::<i8>(), Some(i8_val));
        assert_eq!("128".parse::<i8>(), None);

        i8_val += 1 as i8;
        assert_eq!("-128".parse::<i8>(), Some(i8_val));
        assert_eq!("-129".parse::<i8>(), None);

        let mut i16_val: i16 = 32_767_i16;
        assert_eq!("32767".parse::<i16>(), Some(i16_val));
        assert_eq!("32768".parse::<i16>(), None);

        i16_val += 1 as i16;
        assert_eq!("-32768".parse::<i16>(), Some(i16_val));
        assert_eq!("-32769".parse::<i16>(), None);

        let mut i32_val: i32 = 2_147_483_647_i32;
        assert_eq!("2147483647".parse::<i32>(), Some(i32_val));
        assert_eq!("2147483648".parse::<i32>(), None);

        i32_val += 1 as i32;
        assert_eq!("-2147483648".parse::<i32>(), Some(i32_val));
        assert_eq!("-2147483649".parse::<i32>(), None);

        let mut i64_val: i64 = 9_223_372_036_854_775_807_i64;
        assert_eq!("9223372036854775807".parse::<i64>(), Some(i64_val));
        assert_eq!("9223372036854775808".parse::<i64>(), None);

        i64_val += 1 as i64;
        assert_eq!("-9223372036854775808".parse::<i64>(), Some(i64_val));
        assert_eq!("-9223372036854775809".parse::<i64>(), None);
    }
}
