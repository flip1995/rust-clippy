use rustc::lint::*;
use rustc_front::hir::*;
use rustc_front::util as ast_util;

use utils::{is_exp_equal, span_lint};

/// **What it does:** This lint checks for equal operands to comparison, logical and bitwise,
/// difference and division binary operators (`==`, `>`, etc., `&&`, `||`, `&`, `|`, `^`, `-` and
/// `/`).
///
/// **Why is this bad?** This is usually just a typo or a copy and paste error.
///
/// **Known problems:** False negatives: We had some false positives regarding calls (notably [racer](https://github.com/phildawes/racer) had one instance of `x.pop() && x.pop()`), so we removed matching any function or method calls. We may introduce a whitelist of known pure functions in the future.
///
/// **Example:** `x + 1 == x + 1`
declare_lint! {
    pub EQ_OP,
    Warn,
    "equal operands on both sides of a comparison or bitwise combination (e.g. `x == x`)"
}

#[derive(Copy,Clone)]
pub struct EqOp;

impl LintPass for EqOp {
    fn get_lints(&self) -> LintArray {
        lint_array!(EQ_OP)
    }
}

impl LateLintPass for EqOp {
    fn check_expr(&mut self, cx: &LateContext, e: &Expr) {
        if let ExprBinary(ref op, ref left, ref right) = e.node {
            if is_valid_operator(op) && is_exp_equal(cx, left, right, true) {
                span_lint(cx,
                          EQ_OP,
                          e.span,
                          &format!("equal expressions as operands to `{}`", ast_util::binop_to_string(op.node)));
            }
        }
    }
}


fn is_valid_operator(op: &BinOp) -> bool {
    match op.node {
        BiSub |
        BiDiv |
        BiEq |
        BiLt |
        BiLe |
        BiGt |
        BiGe |
        BiNe |
        BiAnd |
        BiOr |
        BiBitXor |
        BiBitAnd |
        BiBitOr => true,
        _ => false,
    }
}
