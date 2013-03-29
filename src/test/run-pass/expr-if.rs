// Copyright 2012 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.




// -*- rust -*-

// Tests for if as expressions
fn test_if() { let rs: bool = if true { true } else { false }; assert!((rs)); }

fn test_else() {
    let rs: bool = if false { false } else { true };
    assert!((rs));
}

fn test_elseif1() {
    let rs: bool = if true { true } else if true { false } else { false };
    assert!((rs));
}

fn test_elseif2() {
    let rs: bool = if false { false } else if true { true } else { false };
    assert!((rs));
}

fn test_elseif3() {
    let rs: bool = if false { false } else if false { false } else { true };
    assert!((rs));
}

fn test_inferrence() {
    let rs = if true { true } else { false };
    assert!((rs));
}

fn test_if_as_if_condition() {
    let rs1 = if if false { false } else { true } { true } else { false };
    assert!((rs1));
    let rs2 = if if true { false } else { true } { false } else { true };
    assert!((rs2));
}

fn test_if_as_block_result() {
    let rs = if true { if false { false } else { true } } else { false };
    assert!((rs));
}

pub fn main() {
    test_if();
    test_else();
    test_elseif1();
    test_elseif2();
    test_elseif3();
    test_inferrence();
    test_if_as_if_condition();
    test_if_as_block_result();
}
