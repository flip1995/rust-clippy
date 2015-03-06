// Copyright 2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![feature(old_io)]

use std::old_io::Reader;

enum Wrapper<'a> {
    WrapReader(&'a (Reader + 'a))
}

trait Wrap<'a> {
    fn wrap(self) -> Wrapper<'a>;
}

impl<'a, R: Reader> Wrap<'a> for &'a mut R {
    fn wrap(self) -> Wrapper<'a> {
        Wrapper::WrapReader(self as &'a mut Reader)
    }
}

pub fn main() {}
