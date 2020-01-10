use rustc::ty::TyCtxt;
use rustc_errors::struct_span_err;
use rustc_hir as hir;
use rustc_hir::itemlikevisit::ItemLikeVisitor;
use rustc_span::symbol::sym;

use rustc_error_codes::*;

pub fn test_variance(tcx: TyCtxt<'_>) {
    tcx.hir().krate().visit_all_item_likes(&mut VarianceTest { tcx });
}

struct VarianceTest<'tcx> {
    tcx: TyCtxt<'tcx>,
}

impl ItemLikeVisitor<'tcx> for VarianceTest<'tcx> {
    fn visit_item(&mut self, item: &'tcx hir::Item<'tcx>) {
        let item_def_id = self.tcx.hir().local_def_id(item.hir_id);

        // For unit testing: check for a special "rustc_variance"
        // attribute and report an error with various results if found.
        if self.tcx.has_attr(item_def_id, sym::rustc_variance) {
            let variances_of = self.tcx.variances_of(item_def_id);
            struct_span_err!(self.tcx.sess, item.span, E0208, "{:?}", variances_of).emit();
        }
    }

    fn visit_trait_item(&mut self, _: &'tcx hir::TraitItem<'tcx>) {}
    fn visit_impl_item(&mut self, _: &'tcx hir::ImplItem<'tcx>) {}
}
