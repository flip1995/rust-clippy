// compile-pass
#![allow(dead_code)]
#![feature(slice_patterns)]

fn check(list: &[u8]) {
    match list {
        &[] => {},
        &[_u1, _u2, ref _next..] => {},
        &[_u1] => {},
    }
}

fn main() {}
