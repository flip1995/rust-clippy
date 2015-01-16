// Copyright 2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![allow(non_snake_case)]

register_diagnostics! {
    E0023,
    E0024,
    E0025,
    E0026,
    E0027,
    E0029,
    E0030,
    E0031,
    E0033,
    E0034,
    E0035,
    E0036,
    E0038,
    E0040, // explicit use of destructor method
    E0044,
    E0045,
    E0046,
    E0049,
    E0050,
    E0053,
    E0054,
    E0055,
    E0057,
    E0059,
    E0060,
    E0061,
    E0062,
    E0063,
    E0066,
    E0067,
    E0068,
    E0069,
    E0070,
    E0071,
    E0072,
    E0073,
    E0074,
    E0075,
    E0076,
    E0077,
    E0079,
    E0080,
    E0081,
    E0082,
    E0083,
    E0084,
    E0085,
    E0086,
    E0087,
    E0088,
    E0089,
    E0090,
    E0091,
    E0092,
    E0093,
    E0094,
    E0101,
    E0102,
    E0103,
    E0104,
    E0106,
    E0107,
    E0116,
    E0117,
    E0118,
    E0119,
    E0120,
    E0121,
    E0122,
    E0124,
    E0127,
    E0128,
    E0129,
    E0130,
    E0131,
    E0132,
    E0141,
    E0159,
    E0163,
    E0164,
    E0166,
    E0167,
    E0168,
    E0172,
    E0173, // manual implementations of unboxed closure traits are experimental
    E0174, // explicit use of unboxed closure methods are experimental
    E0178,
    E0182,
    E0183,
    E0184
}

__build_diagnostic_array! { DIAGNOSTICS }

