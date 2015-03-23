// Copyright 2012 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// pretty-expanded FIXME #23616

enum Flopsy {
    Bunny = 2
}

const BAR:uint = Flopsy::Bunny as uint;
const BAR2:uint = BAR;

pub fn main() {
    let _v = [0;  Flopsy::Bunny as uint];
    let _v = [0;  BAR];
    let _v = [0;  BAR2];
    const BAR3:uint = BAR2;
    let _v = [0;  BAR3];
}
