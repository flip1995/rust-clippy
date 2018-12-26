// run-pass
#![allow(unused_must_use)]
// ignore-emscripten no threads support
#![feature(rustc_attrs)]

use std::thread;

trait Int {
    fn zero() -> Self;
    fn one() -> Self;
}
macro_rules! doit {
    ($($t:ident)*) => ($(impl Int for $t {
        fn zero() -> $t { 0 }
        fn one() -> $t { 1 }
    })*)
}
doit! { i8 i16 i32 i64 isize }

macro_rules! check {
    ($($e:expr),*) => {
        $(assert!(thread::spawn({
            move|| { $e; }
        }).join().is_err());)*
    }
}

fn main() {
    check![
        isize::min_value() / -isize::one(),
        i8::min_value() / -i8::one(),
        i16::min_value() / -i16::one(),
        i32::min_value() / -i32::one(),
        i64::min_value() / -i64::one(),
        1isize / isize::zero(),
        1i8 / i8::zero(),
        1i16 / i16::zero(),
        1i32 / i32::zero(),
        1i64 / i64::zero(),
        isize::min_value() % -isize::one(),
        i8::min_value() % -i8::one(),
        i16::min_value() % -i16::one(),
        i32::min_value() % -i32::one(),
        i64::min_value() % -i64::one(),
        1isize % isize::zero(),
        1i8 % i8::zero(),
        1i16 % i16::zero(),
        1i32 % i32::zero(),
        1i64 % i64::zero()
    ];
}
