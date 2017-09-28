// Copyright 2013 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use rustc::hir;
use rustc::hir::itemlikevisit::ItemLikeVisitor;
use rustc::ty::TyCtxt;

pub fn test_inferred_outlives<'a, 'tcx>(tcx: TyCtxt<'a, 'tcx, 'tcx>) {
    tcx.hir.krate().visit_all_item_likes(&mut OutlivesTest { tcx });
}

struct OutlivesTest<'a, 'tcx: 'a> {
    tcx: TyCtxt<'a, 'tcx, 'tcx>
}

impl<'a, 'tcx> ItemLikeVisitor<'tcx> for OutlivesTest<'a, 'tcx> {
    fn visit_item(&mut self, item: &'tcx hir::Item) {
        let item_def_id = self.tcx.hir.local_def_id(item.id);

        // For unit testing: check for a special "rustc_outlives"
        // attribute and report an error with various results if found.
        if self.tcx.has_attr(item_def_id, "rustc_outlives") {
            let inferred_outlives_of = self.tcx.inferred_outlives_of(item_def_id);
            span_err!(self.tcx.sess,
                      item.span,
                      E0628,
                      "{:?}",
                      inferred_outlives_of);
        }
    }

    fn visit_trait_item(&mut self, _: &'tcx hir::TraitItem) { }
    fn visit_impl_item(&mut self, _: &'tcx hir::ImplItem) { }
}
