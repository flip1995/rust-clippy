// Copyright 2012-2015 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// This should not cause an ICE

enum Foo { //~ NOTE variant `Baz` not found here
    Bar(u8)
}
fn main(){
    foo(|| {
        match Foo::Bar(1) {
            Foo::Baz(..) => (),
            //~^ ERROR no variant named `Baz` found for type `Foo`
            //~| NOTE variant not found in `Foo`
            _ => (),
        }
    });
}

fn foo<F>(f: F) where F: FnMut() {
    f();
}
