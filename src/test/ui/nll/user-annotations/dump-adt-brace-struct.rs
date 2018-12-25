// Unit test for the "user substitutions" that are annotated on each
// node.

// compile-flags:-Zverbose

#![allow(warnings)]
#![feature(nll)]
#![feature(rustc_attrs)]

struct SomeStruct<T> { t: T }

#[rustc_dump_user_substs]
fn main() {
    SomeStruct { t: 22 }; // Nothing given, no annotation.

    SomeStruct::<_> { t: 22 }; // Nothing interesting given, no annotation.

    SomeStruct::<u32> { t: 22 }; //~ ERROR [u32]
}
