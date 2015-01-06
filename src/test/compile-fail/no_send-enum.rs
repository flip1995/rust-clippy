// Copyright 2013 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::markers;

enum Foo {
    A(markers::NoSend)
}

fn bar<T: Send>(_: T) {}

fn main() {
    let x = Foo::A(markers::NoSend);
    bar(x);
    //~^ ERROR `core::markers::Send` is not implemented
}
