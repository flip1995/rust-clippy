// Copyright 2016 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// aux-build:issue-26606-macro.rs
// ignore-cross-compile
// build-aux-docs

// @has issue_26606_macro/macro.make_item.html
#[macro_use]
extern crate issue_26606_macro;

// @has issue_26606/constant.FOO.html
// @!has - '//a/@href' '../src/'
make_item!(FOO);
