// run-pass
#![allow(unused_imports)]
// aux-build:issue13507.rs

extern crate issue13507;
use issue13507::testtypes;

use std::any::TypeId;

pub fn type_ids() -> Vec<TypeId> {
    use issue13507::testtypes::*;
    vec![
        TypeId::of::<FooBool>(),
        TypeId::of::<FooInt>(),
        TypeId::of::<FooUint>(),
        TypeId::of::<FooFloat>(),
        TypeId::of::<FooStr>(),
        TypeId::of::<FooArray>(),
        TypeId::of::<FooSlice>(),
        TypeId::of::<FooBox>(),
        TypeId::of::<FooPtr>(),
        TypeId::of::<FooRef>(),
        TypeId::of::<FooFnPtr>(),
        TypeId::of::<FooNil>(),
        TypeId::of::<FooTuple>(),
        TypeId::of::<FooTrait>(),
        TypeId::of::<FooStruct>(),
        TypeId::of::<FooEnum>()
    ]
}

pub fn main() {
    let othercrate = issue13507::testtypes::type_ids();
    let thiscrate = type_ids();
    assert_eq!(thiscrate, othercrate);
}
