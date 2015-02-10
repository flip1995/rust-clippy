// Copyright 2015 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! The I/O Prelude
//!
//! The purpose of this module is to alleviate imports of many common I/O traits
//! by adding a glob import to the top of I/O heavy modules:
//!
//! ```
//! use std::io::prelude::*;
//! ```
//!
//! This module contains reexports of many core I/O traits such as `Read`,
//! `Write`, `ReadExt`, and `WriteExt`. Structures and functions are not
//! contained in this module.

pub use super::{Read, ReadExt, Write, WriteExt, BufRead, BufReadExt};
pub use fs::PathExt;

// FIXME: pub use as `Seek` when the name isn't in the actual prelude any more
pub use super::Seek as NewSeek;
