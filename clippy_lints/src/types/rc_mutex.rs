use clippy_utils::diagnostics::span_lint_and_help;
use clippy_utils::{path_def_id, qpath_generic_tys};
use rustc_hir::def_id::DefId;
use rustc_hir::{self as hir, QPath};
use rustc_lint::LateContext;
use rustc_span::symbol::sym;

use super::RC_MUTEX;

pub(super) fn check(cx: &LateContext<'_>, hir_ty: &hir::Ty<'_>, qpath: &QPath<'_>, def_id: DefId) -> bool {
    if cx.tcx.is_diagnostic_item(sym::Rc, def_id)
        && let Some(arg) = qpath_generic_tys(qpath).next()
        && let Some(id) = path_def_id(cx, arg)
        && cx.tcx.is_diagnostic_item(sym::Mutex, id)
    {
        span_lint_and_help(
            cx,
            RC_MUTEX,
            hir_ty.span,
            "usage of `Rc<Mutex<_>>`",
            None,
            "consider using `Rc<RefCell<_>>` or `Arc<Mutex<_>>` instead",
        );
        return true;
    }

    false
}
