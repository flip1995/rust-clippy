// Copyright 2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use core::cmp::Ordering::{Less, Greater, Equal};

#[test]
fn test_int_totalord() {
    assert_eq!(5.cmp(&10), Less);
    assert_eq!(10.cmp(&5), Greater);
    assert_eq!(5.cmp(&5), Equal);
    assert_eq!((-5).cmp(&12), Less);
    assert_eq!(12.cmp(&-5), Greater);
}

#[test]
fn test_mut_int_totalord() {
    assert_eq!((&mut 5).cmp(&&mut 10), Less);
    assert_eq!((&mut 10).cmp(&&mut 5), Greater);
    assert_eq!((&mut 5).cmp(&&mut 5), Equal);
    assert_eq!((&mut -5).cmp(&&mut 12), Less);
    assert_eq!((&mut 12).cmp(&&mut -5), Greater);
}

#[test]
fn test_ordering_reverse() {
    assert_eq!(Less.reverse(), Greater);
    assert_eq!(Equal.reverse(), Equal);
    assert_eq!(Greater.reverse(), Less);
}

#[test]
fn test_ordering_order() {
    assert!(Less < Equal);
    assert_eq!(Greater.cmp(&Less), Greater);
}

#[test]
fn test_ordering_or() {
    assert_eq!(Equal.or(Less), Less);
    assert_eq!(Equal.or(Equal), Equal);
    assert_eq!(Equal.or(Greater), Greater);
    assert_eq!(Less.or(Less), Less);
    assert_eq!(Less.or(Equal), Less);
    assert_eq!(Less.or(Greater), Less);
    assert_eq!(Greater.or(Less), Greater);
    assert_eq!(Greater.or(Equal), Greater);
    assert_eq!(Greater.or(Greater), Greater);
}

#[test]
fn test_ordering_or_else() {
    assert_eq!(Equal.or_else(|| Less), Less);
    assert_eq!(Equal.or_else(|| Equal), Equal);
    assert_eq!(Equal.or_else(|| Greater), Greater);
    assert_eq!(Less.or_else(|| Less), Less);
    assert_eq!(Less.or_else(|| Equal), Less);
    assert_eq!(Less.or_else(|| Greater), Less);
    assert_eq!(Greater.or_else(|| Less), Greater);
    assert_eq!(Greater.or_else(|| Equal), Greater);
    assert_eq!(Greater.or_else(|| Greater), Greater);
}

#[test]
fn test_user_defined_eq() {
    // Our type.
    struct SketchyNum {
        num : isize
    }

    // Our implementation of `PartialEq` to support `==` and `!=`.
    impl PartialEq for SketchyNum {
        // Our custom eq allows numbers which are near each other to be equal! :D
        fn eq(&self, other: &SketchyNum) -> bool {
            (self.num - other.num).abs() < 5
        }
    }

    // Now these binary operators will work when applied!
    assert!(SketchyNum {num: 37} == SketchyNum {num: 34});
    assert!(SketchyNum {num: 25} != SketchyNum {num: 57});
}
