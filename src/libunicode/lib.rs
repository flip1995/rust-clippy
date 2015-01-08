// Copyright 2012-2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! # The Unicode Library
//!
//! Unicode-intensive functions for `char` and `str` types.
//!
//! This crate provides a collection of Unicode-related functionality,
//! including decompositions, conversions, etc., and provides traits
//! implementing these functions for the `char` and `str` types.
//!
//! The functionality included here is only that which is necessary to
//! provide for basic string-related manipulations. This crate does not
//! (yet) aim to provide a full set of Unicode tables.

#![crate_name = "unicode"]
#![experimental]
#![staged_api]
#![crate_type = "rlib"]
#![doc(html_logo_url = "http://www.rust-lang.org/logos/rust-logo-128x128-blk-v2.png",
       html_favicon_url = "http://www.rust-lang.org/favicon.ico",
       html_root_url = "http://doc.rust-lang.org/nightly/",
       html_playground_url = "http://play.rust-lang.org/")]
#![no_std]
#![feature(slicing_syntax)]
#![allow(unknown_features)] #![feature(int_uint)]

extern crate core;

// regex module
pub use tables::regex;

mod normalize;
mod tables;
mod u_char;
mod u_str;

// re-export char so that std et al see it correctly
/// Character manipulation (`char` type, Unicode Scalar Value)
///
/// This module provides the `CharExt` trait, as well as its
/// implementation for the primitive `char` type, in order to allow
/// basic character manipulation.
///
/// A `char` actually represents a
/// *[Unicode Scalar Value](http://www.unicode.org/glossary/#unicode_scalar_value)*,
/// as it can contain any Unicode code point except high-surrogate and
/// low-surrogate code points.
///
/// As such, only values in the ranges \[0x0,0xD7FF\] and \[0xE000,0x10FFFF\]
/// (inclusive) are allowed. A `char` can always be safely cast to a `u32`;
/// however the converse is not always true due to the above range limits
/// and, as such, should be performed via the `from_u32` function..
#[stable]
pub mod char {
    pub use core::char::{MAX, from_u32, from_digit};

    pub use normalize::{decompose_canonical, decompose_compatible, compose};

    pub use tables::normalization::canonical_combining_class;
    pub use tables::UNICODE_VERSION;

    pub use u_char::CharExt;
}

pub mod str {
    pub use u_str::{UnicodeStr, Words, Graphemes, GraphemeIndices};
    pub use u_str::{utf8_char_width, is_utf16, Utf16Items, Utf16Item};
    pub use u_str::{utf16_items, Utf16Encoder};
}

// this lets us use #[derive(..)]
mod std {
    pub use core::clone;
    pub use core::cmp;
    pub use core::fmt;
}
