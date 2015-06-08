// Copyright 2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

fn main() {
    let foo = 1;

    // `foo` shouldn't be suggested, it is too dissimilar from `bar`.
    println!("Hello {}", bar);
    //~^ ERROR: unresolved name `bar`

    // But this is close enough.
    println!("Hello {}", fob);
    //~^ ERROR: unresolved name `fob`. Did you mean `foo`?
}
