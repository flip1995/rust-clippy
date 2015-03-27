// Copyright 2012 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// pretty-expanded FIXME #23616

#![allow(unknown_features)]
#![feature(box_syntax)]

static tau: f64 = 2.0*3.14159265358979323;

struct Point {x: f64, y: f64}
struct Size {w: f64, h: f64}
enum shape {
    circle(Point, f64),
    rectangle(Point, Size)
}


fn compute_area(shape: &shape) -> f64 {
    match *shape {
        shape::circle(_, radius) => 0.5 * tau * radius * radius,
        shape::rectangle(_, ref size) => size.w * size.h
    }
}

impl shape {
    // self is in the implicit self region
    pub fn select<'r, T>(&self, threshold: f64, a: &'r T, b: &'r T)
                         -> &'r T {
        if compute_area(self) > threshold {a} else {b}
    }
}

fn select_based_on_unit_circle<'r, T>(
    threshold: f64, a: &'r T, b: &'r T) -> &'r T {

    let shape = &shape::circle(Point{x: 0.0, y: 0.0}, 1.0);
    shape.select(threshold, a, b)
}

#[derive(Clone)]
struct thing {
    x: A
}

#[derive(Clone)]
struct A {
    a: isize
}

fn thing(x: A) -> thing {
    thing {
        x: x
    }
}

impl thing {
    pub fn bar(self: Box<thing>) -> isize { self.x.a }
    pub fn quux(&self) -> isize { self.x.a }
    pub fn baz<'a>(&'a self) -> &'a A { &self.x }
    pub fn spam(self) -> isize { self.x.a }
}

trait Nus { fn f(&self); }
impl Nus for thing { fn f(&self) {} }

pub fn main() {
    let y: Box<_> = box thing(A {a: 10});
    assert_eq!(y.clone().bar(), 10);
    assert_eq!(y.quux(), 10);

    let z = thing(A {a: 11});
    assert_eq!(z.spam(), 11);
}
