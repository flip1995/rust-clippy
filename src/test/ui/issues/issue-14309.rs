#![deny(improper_ctypes)]
#![allow(dead_code)]

struct A {
    x: i32
}

#[repr(C, packed)]
struct B {
    x: i32,
    y: A
}

#[repr(C)]
struct C {
    x: i32
}

type A2 = A;
type B2 = B;
type C2 = C;

#[repr(C)]
struct D {
    x: C,
    y: A
}

extern "C" {
    fn foo(x: A); //~ ERROR type `A`, which is not FFI-safe
    fn bar(x: B); //~ ERROR type `A`
    fn baz(x: C);
    fn qux(x: A2); //~ ERROR type `A`
    fn quux(x: B2); //~ ERROR type `A`
    fn corge(x: C2);
    fn fred(x: D); //~ ERROR type `A`
}

fn main() { }
