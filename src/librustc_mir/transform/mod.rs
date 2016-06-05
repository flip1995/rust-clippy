// Copyright 2015 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

pub mod remove_dead_blocks;
pub mod simplify_cfg;
pub mod erase_regions;
pub mod no_landing_pads;
pub mod type_check;
pub mod add_call_guards;
pub mod promote_consts;
pub mod qualify_consts;
pub mod dump_mir;
