// Copyright 2012 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// Test case for issue #2843.
//

proto! streamp (
    open:send<T: Owned> {
        data(T) -> open<T>
    }
)

fn rendezvous() {
    let (c, s) = streamp::init();
    let streams: ~[streamp::client::open<int>] = ~[move c];

    error!("%?", streams[0]);
}

pub fn main() {
    //os::getenv("FOO");
    rendezvous();
}
