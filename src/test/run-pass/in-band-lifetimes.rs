// Copyright 2017 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![allow(warnings)]
#![feature(in_band_lifetimes, universal_impl_trait)]

fn foo(x: &'x u8) -> &'x u8 { x }
fn foo2(x: &'a u8, y: &u8) -> &'a u8 { x }

fn check_in_band_can_be_late_bound() {
    let _: for<'x> fn(&'x u8, &u8) -> &'x u8 = foo2;
}

struct ForInherentNoParams;

impl ForInherentNoParams {
    fn foo(x: &'a u32, y: &u32) -> &'a u32 { x }
}

struct X<'a>(&'a u8);

impl<'a> X<'a> {
    fn inner(&self) -> &'a u8 {
        self.0
    }

    fn same_lifetime_as_parameter(&mut self, x: &'a u8) {
        self.0 = x;
    }
}

impl X<'b> {
    fn inner_2(&self) -> &'b u8 {
        self.0
    }

    fn reference_already_introduced_in_band_from_method_with_explicit_binders<'a>(
        &'b self, x: &'a u32
    ) {}
}

struct Y<T>(T);

impl Y<&'a u8> {
    fn inner(&self) -> &'a u8 {
        self.0
    }
}

trait MyTrait<'a> {
    fn my_lifetime(&self) -> &'a u8;
    fn any_lifetime() -> &'b u8;
    fn borrowed_lifetime(&'b self) -> &'b u8;
    fn default_impl(&self, x: &'b u32, y: &u32) -> &'b u32 { x }
    fn in_band_def_explicit_impl(&self, x: &'b u8);
}

impl MyTrait<'a> for Y<&'a u8> {
    fn my_lifetime(&self) -> &'a u8 { self.0 }
    fn any_lifetime() -> &'b u8 { &0 }
    fn borrowed_lifetime(&'b self) -> &'b u8 { &*self.0 }
    fn in_band_def_explicit_impl<'b>(&self, x: &'b u8) {}
}

fn test_hrtb_defined_lifetime_where<F>(_: F) where for<'a> F: Fn(&'a u8) {}
fn test_hrtb_defined_lifetime_polytraitref<F>(_: F) where F: for<'a> Fn(&'a u8) {}

fn reference_in_band_from_locals(x: &'test u32) -> &'test u32 {
    let y: &'test u32 = x;
    y
}

fn in_generics_in_band<T: MyTrait<'a>>(x: &T) {}
fn where_clause_in_band<T>(x: &T) where T: MyTrait<'a> {}
fn impl_trait_in_band(x: &impl MyTrait<'a>) {}

fn main() {}
