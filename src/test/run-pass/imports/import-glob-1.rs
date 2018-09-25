// Copyright 2015 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// run-pass
#![allow(dead_code)]
#![allow(unused_imports)]
// This should resolve fine. Prior to fix, the last import
// was being tried too early, and marked as unrsolved before
// the glob import had a chance to be resolved.

mod bar {
    pub use self::middle::*;

    mod middle {
        pub use self::baz::Baz;

        mod baz {
            pub enum Baz {
                Baz1,
                Baz2
            }
        }
    }
}

mod foo {
    use bar::Baz::{Baz1, Baz2};
}

fn main() {}
