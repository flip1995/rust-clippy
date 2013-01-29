// Copyright 2012 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#[forbid(deprecated_mode)];
//! Additional general-purpose comparison functionality.

use core::f32;
use core::f64;
use core::float;

const fuzzy_epsilon: float = 1.0e-6;

pub trait FuzzyEq {
    pure fn fuzzy_eq(&self, other: &self) -> bool;
}

impl float: FuzzyEq {
    pure fn fuzzy_eq(&self, other: &float) -> bool {
        float::abs(*self - *other) < fuzzy_epsilon
    }
}

impl f32: FuzzyEq {
    pure fn fuzzy_eq(&self, other: &f32) -> bool {
        f32::abs(*self - *other) < (fuzzy_epsilon as f32)
    }
}

impl f64: FuzzyEq {
    pure fn fuzzy_eq(&self, other: &f64) -> bool {
        f64::abs(*self - *other) < (fuzzy_epsilon as f64)
    }
}

#[test]
fn test_fuzzy_equals() {
    assert (&1.0).fuzzy_eq(&1.0);
    assert (&1.0f32).fuzzy_eq(&1.0f32);
    assert (&1.0f64).fuzzy_eq(&1.0f64);
}
