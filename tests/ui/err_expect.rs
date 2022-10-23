// run-rustfix

#![feature(custom_inner_attributes)]
#![allow(unused)]

struct MyTypeNonDebug;

#[derive(Debug)]
struct MyTypeDebug;

fn main() {
    let test_debug: Result<MyTypeDebug, u32> = Ok(MyTypeDebug);
    test_debug.err().expect("Testing debug type");

    let test_non_debug: Result<MyTypeNonDebug, u32> = Ok(MyTypeNonDebug);
    test_non_debug.err().expect("Testing non debug type");
}

fn msrv_1_16() {
    #![clippy::msrv = "1.16"]

    let x: Result<u32, &str> = Ok(16);
    x.err().expect("16");
}

fn msrv_1_17() {
    #![clippy::msrv = "1.17"]

    let x: Result<u32, &str> = Ok(17);
    x.err().expect("17");
}
