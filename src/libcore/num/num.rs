// Copyright 2012-2013 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! An interface for numeric types
use cmp::{Eq, Ord};
#[cfg(stage0)]
use ops::{Add, Sub, Mul, Neg};
#[cfg(stage0)]
use Quot = ops::Div;
#[cfg(stage0)]
use Rem = ops::Modulo;
#[cfg(not(stage0))]
use ops::{Add, Sub, Mul, Quot, Rem, Neg};
use option::Option;
use kinds::Copy;

pub mod strconv;

pub trait Num: Eq + Zero + One
             + Neg<Self>
             + Add<Self,Self>
             + Sub<Self,Self>
             + Mul<Self,Self>
             + Quot<Self,Self>
             + Rem<Self,Self> {}

pub trait IntConvertible {
    fn to_int(&self) -> int;
    fn from_int(n: int) -> Self;
}

pub trait Zero {
    // FIXME (#5527): These should be associated constants
    fn zero() -> Self;
}

pub trait One {
    // FIXME (#5527): These should be associated constants
    fn one() -> Self;
}

pub trait Signed: Num
                + Neg<Self> {
    fn abs(&self) -> Self;
    fn signum(&self) -> Self;
    fn is_positive(&self) -> bool;
    fn is_negative(&self) -> bool;
}

pub trait Unsigned: Num {}

// This should be moved into the default implementation for Signed::abs
pub fn abs<T:Ord + Zero + Neg<T>>(v: T) -> T {
    if v < Zero::zero() { v.neg() } else { v }
}

pub trait Integer: Num
                 + Ord
                 + Quot<Self,Self>
                 + Rem<Self,Self> {
    fn div(&self, other: &Self) -> Self;
    fn modulo(&self, other: &Self) -> Self;
    fn div_mod(&self, other: &Self) -> (Self,Self);
    fn quot_rem(&self, other: &Self) -> (Self,Self);

    fn gcd(&self, other: &Self) -> Self;
    fn lcm(&self, other: &Self) -> Self;
    fn divisible_by(&self, other: &Self) -> bool;
    fn is_even(&self) -> bool;
    fn is_odd(&self) -> bool;
}

pub trait Fractional: Num
                    + Ord
                    + Quot<Self,Self> {
    fn recip(&self) -> Self;
}

pub trait Real: Signed
              + Fractional {
    // FIXME (#5527): usages of `int` should be replaced with an associated
    // integer type once these are implemented

    // Common Constants
    // FIXME (#5527): These should be associated constants
    fn pi() -> Self;
    fn two_pi() -> Self;
    fn frac_pi_2() -> Self;
    fn frac_pi_3() -> Self;
    fn frac_pi_4() -> Self;
    fn frac_pi_6() -> Self;
    fn frac_pi_8() -> Self;
    fn frac_1_pi() -> Self;
    fn frac_2_pi() -> Self;
    fn frac_2_sqrtpi() -> Self;
    fn sqrt2() -> Self;
    fn frac_1_sqrt2() -> Self;
    fn e() -> Self;
    fn log2_e() -> Self;
    fn log10_e() -> Self;
    fn log_2() -> Self;
    fn log_10() -> Self;

    // Rounding operations
    fn floor(&self) -> Self;
    fn ceil(&self) -> Self;
    fn round(&self) -> Self;
    fn trunc(&self) -> Self;
    fn fract(&self) -> Self;

    // Exponential functions
    fn pow(&self, n: Self) -> Self;
    fn exp(&self) -> Self;
    fn exp2(&self) -> Self;
    fn expm1(&self) -> Self;
    fn ldexp(&self, n: int) -> Self;
    fn log(&self) -> Self;
    fn log2(&self) -> Self;
    fn log10(&self) -> Self;
    fn log_radix(&self) -> Self;
    fn ilog_radix(&self) -> int;
    fn sqrt(&self) -> Self;
    fn rsqrt(&self) -> Self;
    fn cbrt(&self) -> Self;

    // Angular conversions
    fn to_degrees(&self) -> Self;
    fn to_radians(&self) -> Self;

    // Triganomic functions
    fn hypot(&self, other: Self) -> Self;
    fn sin(&self) -> Self;
    fn cos(&self) -> Self;
    fn tan(&self) -> Self;

    // Inverse triganomic functions
    fn asin(&self) -> Self;
    fn acos(&self) -> Self;
    fn atan(&self) -> Self;
    fn atan2(&self, other: Self) -> Self;

    // Hyperbolic triganomic functions
    fn sinh(&self) -> Self;
    fn cosh(&self) -> Self;
    fn tanh(&self) -> Self;
}

/// Methods that are harder to implement and not commonly used.
pub trait RealExt: Real {
    // FIXME (#5527): usages of `int` should be replaced with an associated
    // integer type once these are implemented

    // Gamma functions
    fn lgamma(&self) -> (int, Self);
    fn tgamma(&self) -> Self;

    // Bessel functions
    fn j0(&self) -> Self;
    fn j1(&self) -> Self;
    fn jn(&self, n: int) -> Self;
    fn y0(&self) -> Self;
    fn y1(&self) -> Self;
    fn yn(&self, n: int) -> Self;
}

/**
 * Cast from one machine scalar to another
 *
 * # Example
 *
 * ~~~
 * let twenty: f32 = num::cast(0x14);
 * assert_eq!(twenty, 20f32);
 * ~~~
 */
#[inline(always)]
pub fn cast<T:NumCast,U:NumCast>(n: T) -> U {
    NumCast::from(n)
}

/**
 * An interface for casting between machine scalars
 */
pub trait NumCast {
    fn from<T:NumCast>(n: T) -> Self;

    fn to_u8(&self) -> u8;
    fn to_u16(&self) -> u16;
    fn to_u32(&self) -> u32;
    fn to_u64(&self) -> u64;
    fn to_uint(&self) -> uint;

    fn to_i8(&self) -> i8;
    fn to_i16(&self) -> i16;
    fn to_i32(&self) -> i32;
    fn to_i64(&self) -> i64;
    fn to_int(&self) -> int;

    fn to_f32(&self) -> f32;
    fn to_f64(&self) -> f64;
    fn to_float(&self) -> float;
}

macro_rules! impl_num_cast(
    ($T:ty, $conv:ident) => (
        impl NumCast for $T {
            #[inline(always)]
            fn from<N:NumCast>(n: N) -> $T {
                // `$conv` could be generated using `concat_idents!`, but that
                // macro seems to be broken at the moment
                n.$conv()
            }

            #[inline(always)] fn to_u8(&self)    -> u8    { *self as u8    }
            #[inline(always)] fn to_u16(&self)   -> u16   { *self as u16   }
            #[inline(always)] fn to_u32(&self)   -> u32   { *self as u32   }
            #[inline(always)] fn to_u64(&self)   -> u64   { *self as u64   }
            #[inline(always)] fn to_uint(&self)  -> uint  { *self as uint  }

            #[inline(always)] fn to_i8(&self)    -> i8    { *self as i8    }
            #[inline(always)] fn to_i16(&self)   -> i16   { *self as i16   }
            #[inline(always)] fn to_i32(&self)   -> i32   { *self as i32   }
            #[inline(always)] fn to_i64(&self)   -> i64   { *self as i64   }
            #[inline(always)] fn to_int(&self)   -> int   { *self as int   }

            #[inline(always)] fn to_f32(&self)   -> f32   { *self as f32   }
            #[inline(always)] fn to_f64(&self)   -> f64   { *self as f64   }
            #[inline(always)] fn to_float(&self) -> float { *self as float }
        }
    )
)

impl_num_cast!(u8,    to_u8)
impl_num_cast!(u16,   to_u16)
impl_num_cast!(u32,   to_u32)
impl_num_cast!(u64,   to_u64)
impl_num_cast!(uint,  to_uint)
impl_num_cast!(i8,    to_i8)
impl_num_cast!(i16,   to_i16)
impl_num_cast!(i32,   to_i32)
impl_num_cast!(i64,   to_i64)
impl_num_cast!(int,   to_int)
impl_num_cast!(f32,   to_f32)
impl_num_cast!(f64,   to_f64)
impl_num_cast!(float, to_float)

pub trait ToStrRadix {
    pub fn to_str_radix(&self, radix: uint) -> ~str;
}

pub trait FromStrRadix {
    pub fn from_str_radix(str: &str, radix: uint) -> Option<Self>;
}

// Generic math functions:

/**
 * Calculates a power to a given radix, optimized for uint `pow` and `radix`.
 *
 * Returns `radix^pow` as `T`.
 *
 * Note:
 * Also returns `1` for `0^0`, despite that technically being an
 * undefined number. The reason for this is twofold:
 * - If code written to use this function cares about that special case, it's
 *   probably going to catch it before making the call.
 * - If code written to use this function doesn't care about it, it's
 *   probably assuming that `x^0` always equals `1`.
 */
pub fn pow_with_uint<T:NumCast+One+Zero+Copy+Quot<T,T>+Mul<T,T>>(
    radix: uint, pow: uint) -> T {
    let _0: T = Zero::zero();
    let _1: T = One::one();

    if pow   == 0u { return _1; }
    if radix == 0u { return _0; }
    let mut my_pow     = pow;
    let mut total      = _1;
    let mut multiplier = cast(radix as int);
    while (my_pow > 0u) {
        if my_pow % 2u == 1u {
            total *= multiplier;
        }
        my_pow     /= 2u;
        multiplier *= multiplier;
    }
    total
}

/// Helper function for testing numeric operations
#[cfg(stage0,test)]
pub fn test_num<T:Num + NumCast>(ten: T, two: T) {
    assert_eq!(ten.add(&two),    cast(12));
    assert_eq!(ten.sub(&two),    cast(8));
    assert_eq!(ten.mul(&two),    cast(20));
    assert_eq!(ten.div(&two),    cast(5));
    assert_eq!(ten.modulo(&two), cast(0));

    assert_eq!(ten.add(&two),    ten + two);
    assert_eq!(ten.sub(&two),    ten - two);
    assert_eq!(ten.mul(&two),    ten * two);
    assert_eq!(ten.div(&two),    ten / two);
    assert_eq!(ten.modulo(&two), ten % two);
}
#[cfg(stage1,test)]
#[cfg(stage2,test)]
#[cfg(stage3,test)]
pub fn test_num<T:Num + NumCast>(ten: T, two: T) {
    assert_eq!(ten.add(&two),  cast(12));
    assert_eq!(ten.sub(&two),  cast(8));
    assert_eq!(ten.mul(&two),  cast(20));
    assert_eq!(ten.quot(&two), cast(5));
    assert_eq!(ten.rem(&two),  cast(0));

    assert_eq!(ten.add(&two),  ten + two);
    assert_eq!(ten.sub(&two),  ten - two);
    assert_eq!(ten.mul(&two),  ten * two);
    assert_eq!(ten.quot(&two), ten / two);
    assert_eq!(ten.rem(&two),  ten % two);
}

macro_rules! test_cast_20(
    ($_20:expr) => ({
        let _20 = $_20;

        assert_eq!(20u,   _20.to_uint());
        assert_eq!(20u8,  _20.to_u8());
        assert_eq!(20u16, _20.to_u16());
        assert_eq!(20u32, _20.to_u32());
        assert_eq!(20u64, _20.to_u64());
        assert_eq!(20i,   _20.to_int());
        assert_eq!(20i8,  _20.to_i8());
        assert_eq!(20i16, _20.to_i16());
        assert_eq!(20i32, _20.to_i32());
        assert_eq!(20i64, _20.to_i64());
        assert_eq!(20f,   _20.to_float());
        assert_eq!(20f32, _20.to_f32());
        assert_eq!(20f64, _20.to_f64());

        assert_eq!(_20, NumCast::from(20u));
        assert_eq!(_20, NumCast::from(20u8));
        assert_eq!(_20, NumCast::from(20u16));
        assert_eq!(_20, NumCast::from(20u32));
        assert_eq!(_20, NumCast::from(20u64));
        assert_eq!(_20, NumCast::from(20i));
        assert_eq!(_20, NumCast::from(20i8));
        assert_eq!(_20, NumCast::from(20i16));
        assert_eq!(_20, NumCast::from(20i32));
        assert_eq!(_20, NumCast::from(20i64));
        assert_eq!(_20, NumCast::from(20f));
        assert_eq!(_20, NumCast::from(20f32));
        assert_eq!(_20, NumCast::from(20f64));

        assert_eq!(_20, cast(20u));
        assert_eq!(_20, cast(20u8));
        assert_eq!(_20, cast(20u16));
        assert_eq!(_20, cast(20u32));
        assert_eq!(_20, cast(20u64));
        assert_eq!(_20, cast(20i));
        assert_eq!(_20, cast(20i8));
        assert_eq!(_20, cast(20i16));
        assert_eq!(_20, cast(20i32));
        assert_eq!(_20, cast(20i64));
        assert_eq!(_20, cast(20f));
        assert_eq!(_20, cast(20f32));
        assert_eq!(_20, cast(20f64));
    })
)

#[test] fn test_u8_cast()    { test_cast_20!(20u8)  }
#[test] fn test_u16_cast()   { test_cast_20!(20u16) }
#[test] fn test_u32_cast()   { test_cast_20!(20u32) }
#[test] fn test_u64_cast()   { test_cast_20!(20u64) }
#[test] fn test_uint_cast()  { test_cast_20!(20u)   }
#[test] fn test_i8_cast()    { test_cast_20!(20i8)  }
#[test] fn test_i16_cast()   { test_cast_20!(20i16) }
#[test] fn test_i32_cast()   { test_cast_20!(20i32) }
#[test] fn test_i64_cast()   { test_cast_20!(20i64) }
#[test] fn test_int_cast()   { test_cast_20!(20i)   }
#[test] fn test_f32_cast()   { test_cast_20!(20f32) }
#[test] fn test_f64_cast()   { test_cast_20!(20f64) }
#[test] fn test_float_cast() { test_cast_20!(20f)   }
