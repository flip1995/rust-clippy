use clippy_utils::diagnostics::span_lint_and_help;
use clippy_utils::is_from_proc_macro;
use clippy_utils::ty::needs_ordered_drop;
use rustc_hir::def::Res;
use rustc_hir::{
    BindingAnnotation, ByRef, Expr, ExprKind, HirId, Local, Node, Pat, PatKind, QPath,
};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_middle::lint::{in_external_macro, is_from_async_await};
use rustc_session::{declare_lint_pass, declare_tool_lint};
use rustc_span::symbol::Ident;

declare_clippy_lint! {
    /// ### What it does
    /// Checks for redundant redefinitions of local bindings.
    ///
    /// ### Why is this bad?
    /// Redundant redefinitions of local bindings do not change behavior and are likely to be unintended.
    ///
    /// Note that although these bindings do not affect your code's meaning, they _may_ affect `rustc`'s stack allocation.
    ///
    /// ### Example
    /// ```rust
    /// let a = 0;
    /// let a = a;
    ///
    /// fn foo(b: i32) {
    ///    let b = b;
    /// }
    /// ```
    /// Use instead:
    /// ```rust
    /// let a = 0;
    /// // no redefinition with the same name
    ///
    /// fn foo(b: i32) {
    ///   // no redefinition with the same name
    /// }
    /// ```
    #[clippy::version = "1.72.0"]
    pub REDUNDANT_LOCALS,
    correctness,
    "redundant redefinition of a local binding"
}
declare_lint_pass!(RedundantLocals => [REDUNDANT_LOCALS]);

impl<'tcx> LateLintPass<'tcx> for RedundantLocals {
    fn check_local(&mut self, cx: &LateContext<'tcx>, local: &'tcx Local<'tcx>) {
        if_chain! {
            // the pattern is a single by-value binding
            if let PatKind::Binding(BindingAnnotation(ByRef::No, mutability), _, ident, None) = local.pat.kind;
            // the binding is not type-ascribed
            if local.ty.is_none();
            // the expression is a resolved path
            if let Some(expr) = local.init;
            if let ExprKind::Path(qpath @ QPath::Resolved(None, path)) = expr.kind;
            // the path is a single segment equal to the local's name
            if let [last_segment] = path.segments;
            if last_segment.ident == ident;
            // resolve the path to its defining binding pattern
            if let Res::Local(binding_id) = cx.qpath_res(&qpath, expr.hir_id);
            if let Node::Pat(binding_pat) = cx.tcx.hir().get(binding_id);
            // the previous binding has the same mutability
            if find_binding(binding_pat, ident).unwrap().1 == mutability;
            // the local does not affect the code's drop behavior
            if !affects_drop_behavior(cx, binding_id, local.hir_id, expr);
            // the local is user-controlled
            if !in_external_macro(cx.sess(), local.span);
            if !is_from_proc_macro(cx, expr);
            // Async function parameters are lowered into the closure body, so we can't lint them.
            // see `lower_maybe_async_body` in `rust_ast_lowering`
            if !is_from_async_await(local.span);
            then {
                span_lint_and_help(
                    cx,
                    REDUNDANT_LOCALS,
                    vec![binding_pat.span, local.span],
                    "redundant redefinition of a binding",
                    None,
                    &format!("remove the redefinition of `{ident}`"),
                );
            }
        }
    }
}

/// Find the annotation of a binding introduced by a pattern, or `None` if it's not introduced.
fn find_binding(pat: &Pat<'_>, name: Ident) -> Option<BindingAnnotation> {
    let mut ret = None;

    pat.each_binding_or_first(&mut |annotation, _, _, ident| {
        if ident == name {
            ret = Some(annotation);
        }
    });

    ret
}

/// Check if a rebinding of a local affects the code's drop behavior.
fn affects_drop_behavior<'tcx>(
    cx: &LateContext<'tcx>,
    bind: HirId,
    rebind: HirId,
    rebind_expr: &Expr<'tcx>,
) -> bool {
    let hir = cx.tcx.hir();

    // the rebinding is in a different scope than the original binding
    // and the type of the binding cares about drop order
    hir.get_enclosing_scope(bind) != hir.get_enclosing_scope(rebind)
        && needs_ordered_drop(cx, cx.typeck_results().expr_ty(rebind_expr))
}
