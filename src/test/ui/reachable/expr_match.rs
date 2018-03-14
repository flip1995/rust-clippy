// Copyright 2016 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![allow(unused_variables)]
#![allow(unused_assignments)]
#![allow(dead_code)]
#![deny(unreachable_code)]

fn a() {
    // The match is considered unreachable here, because the `return`
    // diverges:
    match {return} { } //~ ERROR unreachable
}

fn b() {
    match () { () => return }
    println!("I am dead");
}

fn c() {
    match () { () if false => return, () => () }
    println!("I am not dead");
}

fn d() {
    match () { () if false => return, () => return }
    println!("I am dead");
}

fn e() {
    // Here the compiler fails to figure out that the `println` is dead.
    match () { () if return => (), () => return }
    println!("I am dead");
}

fn f() {
    match Some(()) { None => (), Some(()) => return }
    println!("I am not dead");
}

fn g() {
    match Some(()) { None => return, Some(()) => () }
    println!("I am not dead");
}

fn main() { }
