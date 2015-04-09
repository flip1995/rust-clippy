// Copyright 2015 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// Make sure the type inference for the new range expression work as
// good as the old one. Check out issue #21672, #21595 and #21649 for
// more details.


fn main() {
    let xs = (0..8).map(|i| i == 1u64).collect::<Vec<_>>();
    assert_eq!(xs[1], true);
    let xs = (0..8).map(|i| 1u64 == i).collect::<Vec<_>>();
    assert_eq!(xs[1], true);
    let xs: Vec<u8> = (0..10).collect();
    assert_eq!(xs.len(), 10);

    for x in 0..10 { x % 2; }
    for x in 0..100 { x as f32; }

    let array = [true, false];
    for i in 0..1 { array[i]; }
}
