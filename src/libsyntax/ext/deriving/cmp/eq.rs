// Copyright 2013 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use core::prelude::*;

use ast::{meta_item, item, expr};
use codemap::span;
use ext::base::ExtCtxt;
use ext::build::AstBuilder;
use ext::deriving::generic::*;

pub fn expand_deriving_eq(cx: @ExtCtxt,
                          span: span,
                          mitem: @meta_item,
                          in_items: ~[@item]) -> ~[@item] {
    // structures are equal if all fields are equal, and non equal, if
    // any fields are not equal or if the enum variants are different
    fn cs_eq(cx: @ExtCtxt, span: span, substr: &Substructure) -> @expr {
        cs_and(|cx, span, _, _| cx.expr_bool(span, false),
                                 cx, span, substr)
    }
    fn cs_ne(cx: @ExtCtxt, span: span, substr: &Substructure) -> @expr {
        cs_or(|cx, span, _, _| cx.expr_bool(span, true),
              cx, span, substr)
    }

    macro_rules! md (
        ($name:expr, $f:ident) => {
            MethodDef {
                name: $name,
                generics: LifetimeBounds::empty(),
                explicit_self: borrowed_explicit_self(),
                args: ~[borrowed_self()],
                ret_ty: Literal(Path::new(~["bool"])),
                const_nonmatching: true,
                combine_substructure: $f
            },
        }
    );

    let trait_def = TraitDef {
        path: Path::new(~["std", "cmp", "Eq"]),
        additional_bounds: ~[],
        generics: LifetimeBounds::empty(),
        methods: ~[
            md!("eq", cs_eq),
            md!("ne", cs_ne)
        ]
    };

    expand_deriving_generic(cx, span, mitem, in_items,
                            &trait_def)
}
