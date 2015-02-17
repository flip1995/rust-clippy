// Copyright 2015 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// Regression test for #15477. This test should pass, vs reporting an
// error as it does now, but at least this test shows it doesn't
// segfault.

trait Chromosome<X: Chromosome> {
    //~^ ERROR cyclic reference detected
}

fn main() { }
