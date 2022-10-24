use clippy_utils::diagnostics::span_lint_and_then;
use clippy_utils::higher::IfLetOrMatch;
use clippy_utils::source::snippet;
use clippy_utils::visitors::is_local_used;
use clippy_utils::{
    is_res_lang_ctor, is_unit_expr, path_to_local, peel_blocks_with_stmt, peel_ref_operators, SpanlessEq,
};
use if_chain::if_chain;
use rustc_errors::MultiSpan;
use rustc_hir::LangItem::OptionNone;
use rustc_hir::{Arm, Expr, Guard, HirId, Let, Pat, PatKind};
use rustc_lint::LateContext;
use rustc_span::Span;

use super::COLLAPSIBLE_MATCH;

pub(super) fn check_match<'tcx>(cx: &LateContext<'tcx>, arms: &'tcx [Arm<'_>]) {
    if let Some(els_arm) = arms.iter().rfind(|arm| arm_is_wild_like(cx, arm)) {
        for arm in arms {
            check_arm(cx, true, arm.pat, arm.body, arm.guard.as_ref(), Some(els_arm.body));
        }
    }
}

pub(super) fn check_if_let<'tcx>(
    cx: &LateContext<'tcx>,
    pat: &'tcx Pat<'_>,
    body: &'tcx Expr<'_>,
    else_expr: Option<&'tcx Expr<'_>>,
) {
    check_arm(cx, false, pat, body, None, else_expr);
}

fn check_arm<'tcx>(
    cx: &LateContext<'tcx>,
    outer_is_match: bool,
    outer_pat: &'tcx Pat<'tcx>,
    outer_then_body: &'tcx Expr<'tcx>,
    outer_guard: Option<&'tcx Guard<'tcx>>,
    outer_else_body: Option<&'tcx Expr<'tcx>>,
) {
    let inner_expr = peel_blocks_with_stmt(outer_then_body);
    if_chain! {
        if let Some(inner) = IfLetOrMatch::parse(cx, inner_expr);
        if let Some((inner_scrutinee, inner_then_pat, inner_else_body)) = match inner {
            IfLetOrMatch::IfLet(scrutinee, pat, _, els) => Some((scrutinee, pat, els)),
            IfLetOrMatch::Match(scrutinee, arms, ..) => if_chain! {
                // if there are more than two arms, collapsing would be non-trivial
                if arms.len() == 2 && arms.iter().all(|a| a.guard.is_none());
                // one of the arms must be "wild-like"
                if let Some(wild_idx) = arms.iter().rposition(|a| arm_is_wild_like(cx, a));
                then {
                    let (then, els) = (&arms[1 - wild_idx], &arms[wild_idx]);
                    Some((scrutinee, then.pat, Some(els.body)))
                } else {
                    None
                }
            },
        };
        if outer_pat.span.ctxt() == inner_scrutinee.span.ctxt();
        // match expression must be a local binding
        // match <local> { .. }
        if let Some(binding_id) = path_to_local(peel_ref_operators(cx, inner_scrutinee));
        if !pat_contains_or(inner_then_pat);
        // the binding must come from the pattern of the containing match arm
        // ..<local>.. => match <local> { .. }
        if let (Some(binding_span), is_innermost_parent_pat_struct)
            = find_pat_binding_and_is_innermost_parent_pat_struct(outer_pat, binding_id);
        // the "else" branches must be equal
        if match (outer_else_body, inner_else_body) {
            (None, None) => true,
            (None, Some(e)) | (Some(e), None) => is_unit_expr(e),
            (Some(a), Some(b)) => SpanlessEq::new(cx).eq_expr(a, b),
        };
        // the binding must not be used in the if guard
        if outer_guard.map_or(
            true,
            |(Guard::If(e) | Guard::IfLet(Let { init: e, .. }))| !is_local_used(cx, *e, binding_id)
        );
        // ...or anywhere in the inner expression
        if match inner {
            IfLetOrMatch::IfLet(_, _, body, els) => {
                !is_local_used(cx, body, binding_id) && els.map_or(true, |e| !is_local_used(cx, e, binding_id))
            },
            IfLetOrMatch::Match(_, arms, ..) => !arms.iter().any(|arm| is_local_used(cx, arm, binding_id)),
        };
        then {
            let msg = format!(
                "this `{}` can be collapsed into the outer `{}`",
                if matches!(inner, IfLetOrMatch::Match(..)) { "match" } else { "if let" },
                if outer_is_match { "match" } else { "if let" },
            );
            // collapsing patterns need an explicit field name in struct pattern matching
            // ex: Struct {x: Some(1)}
            let replace_msg = if is_innermost_parent_pat_struct {
                format!(", prefixed by {}:", snippet(cx, binding_span, "their field name"))
            } else {
                String::new()
            };
            span_lint_and_then(
                cx,
                COLLAPSIBLE_MATCH,
                inner_expr.span,
                &msg,
                |diag| {
                    let mut help_span = MultiSpan::from_spans(vec![binding_span, inner_then_pat.span]);
                    help_span.push_span_label(binding_span, "replace this binding");
                    help_span.push_span_label(inner_then_pat.span, format!("with this pattern{replace_msg}"));
                    diag.span_help(help_span, "the outer pattern can be modified to include the inner pattern");
                },
            );
        }
    }
}

/// A "wild-like" arm has a wild (`_`) or `None` pattern and no guard. Such arms can be "collapsed"
/// into a single wild arm without any significant loss in semantics or readability.
fn arm_is_wild_like(cx: &LateContext<'_>, arm: &Arm<'_>) -> bool {
    if arm.guard.is_some() {
        return false;
    }
    match arm.pat.kind {
        PatKind::Binding(..) | PatKind::Wild => true,
        PatKind::Path(ref qpath) => is_res_lang_ctor(cx, cx.qpath_res(qpath, arm.pat.hir_id), OptionNone),
        _ => false,
    }
}

fn find_pat_binding_and_is_innermost_parent_pat_struct(pat: &Pat<'_>, hir_id: HirId) -> (Option<Span>, bool) {
    let mut span = None;
    let mut is_innermost_parent_pat_struct = false;
    pat.walk_short(|p| match &p.kind {
        // ignore OR patterns
        PatKind::Or(_) => false,
        PatKind::Binding(_bm, _, _ident, _) => {
            let found = p.hir_id == hir_id;
            if found {
                span = Some(p.span);
            }
            !found
        },
        _ => {
            is_innermost_parent_pat_struct = matches!(p.kind, PatKind::Struct(..));
            true
        },
    });
    (span, is_innermost_parent_pat_struct)
}

fn pat_contains_or(pat: &Pat<'_>) -> bool {
    let mut result = false;
    pat.walk(|p| {
        let is_or = matches!(p.kind, PatKind::Or(_));
        result |= is_or;
        !is_or
    });
    result
}
