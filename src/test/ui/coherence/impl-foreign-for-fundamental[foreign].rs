#![feature(re_rebalance_coherence)]

// compile-flags:--crate-name=test
// aux-build:coherence_lib.rs

extern crate coherence_lib as lib;
use lib::*;
use std::rc::Rc;

struct Local;

impl Remote for Box<i32> {
    //~^ ERROR only traits defined in the current crate
    // | can be implemented for arbitrary types [E0117]
}
impl<T> Remote for Box<Rc<T>> {
    //~^ ERROR only traits defined in the current crate
    // | can be implemented for arbitrary types [E0117]
}

fn main() {}
