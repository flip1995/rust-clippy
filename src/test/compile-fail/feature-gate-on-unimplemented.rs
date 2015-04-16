// Copyright 2015 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// Test that `#[rustc_on_unimplemented]` is gated by `on_unimplemented` feature
// gate.

#[rustc_on_unimplemented = "test error `{Self}` with `{Bar}`"]
//~^ ERROR the `#[rustc_on_unimplemented]` attribute is an experimental feature
trait Foo<Bar>
{}

fn main() {}
