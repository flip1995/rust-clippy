// Copyright 2012 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Operations and constants for `i16`

mod inst {
    use num::Primitive;

    pub type T = i16;
    pub static bits: uint = ::u16::bits;

    impl Primitive for i16 {
        #[inline(always)]
        fn bits() -> uint { 16 }

        #[inline(always)]
        fn bytes() -> uint { Primitive::bits::<i16>() / 8 }
    }
}
