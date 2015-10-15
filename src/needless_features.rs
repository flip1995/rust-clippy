//! Checks for usage of nightly features that have simple stable equivalents
//!
//! This lint is **warn** by default

use rustc::lint::*;
use rustc_front::hir::*;

use utils::{span_lint};

declare_lint! {
    pub AS_SLICE,
    Warn,
    "as_slice is not stable and can be replaced by & v[..]\
see https://github.com/rust-lang/rust/issues/27729"
}

declare_lint! {
    pub AS_MUT_SLICE,
    Warn,
    "as_mut_slice is not stable and can be replaced by &mut v[..]\
see https://github.com/rust-lang/rust/issues/27729"
}


#[derive(Copy,Clone)]
pub struct NeedlessFeaturesPass;

impl LintPass for NeedlessFeaturesPass {
    fn get_lints(&self) -> LintArray {
        lint_array!(AS_SLICE,AS_MUT_SLICE)
    }
}

impl LateLintPass for NeedlessFeaturesPass {
    fn check_expr(&mut self, cx: &LateContext, expr: &Expr) {
        if let ExprMethodCall(ref name, _, _) = expr.node {
            if name.node.as_str() == "as_slice" {
                span_lint(cx, AS_SLICE, expr.span,
                          "used as_slice() from the 'convert' nightly feature. Use &[..] \
                           instead");
            }
            if name.node.as_str() == "as_mut_slice" {
                span_lint(cx, AS_MUT_SLICE, expr.span,
                          "used as_mut_slice() from the 'convert' nightly feature. Use &mut [..] \
                           instead");
            }
        }
    }
}