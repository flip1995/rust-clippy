// Copyright 2012 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.


#![feature(str_words)]

fn main() {
    let foo = "hello".to_string();
    let foo: Vec<&str> = foo.words().collect();
    let invalid_string = &foo[0];
    assert_eq!(*invalid_string, "hello");
}
