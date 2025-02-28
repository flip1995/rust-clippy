#![warn(clippy::unit_arg)]
#![allow(unused_must_use, unused_variables)]
#![allow(clippy::no_effect, clippy::uninlined_format_args)]

use std::fmt::Debug;

fn foo<T: Debug>(t: T) {
    println!("{:?}", t);
}

fn foo3<T1: Debug, T2: Debug, T3: Debug>(t1: T1, t2: T2, t3: T3) {
    println!("{:?}, {:?}, {:?}", t1, t2, t3);
}

fn bad() {
    foo({});
    //~^ unit_arg
    foo3({}, 2, 2);
    //~^ unit_arg
    taking_two_units({}, foo(0));
    //~^ unit_arg
    taking_three_units({}, foo(0), foo(1));
    //~^ unit_arg
}

fn taking_two_units(a: (), b: ()) {}
fn taking_three_units(a: (), b: (), c: ()) {}

fn main() {
    bad();
}
