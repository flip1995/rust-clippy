use clippy_utils::diagnostics::{multispan_sugg_with_applicability, span_lint_and_then};
use rustc_errors::Applicability;
use rustc_hir::{Block, Expr, ExprKind, Stmt, StmtKind};
use rustc_lint::{LateContext, LateLintPass};
use rustc_session::{declare_lint_pass, declare_tool_lint};
use rustc_span::Span;

declare_clippy_lint! {
    /// ### What it does
    ///
    /// For () returning expressions, check that the semicolon is inside the block.
    ///
    /// ### Why is this bad?
    ///
    /// For consistency it's best to have the semicolon inside/outside the block. Either way is fine and this lint suggests inside the block.
    /// Take a look at `semicolon_outside_block` for the other alternative.
    ///
    /// ### Example
    ///
    /// ```rust
    /// # fn f(_: u32) {}
    /// # let x = 0;
    /// unsafe { f(x) };
    /// ```
    /// Use instead:
    /// ```rust
    /// # fn f(_: u32) {}
    /// # let x = 0;
    /// unsafe { f(x); }
    /// ```
    #[clippy::version = "1.66.0"]
    pub SEMICOLON_INSIDE_BLOCK,
    restriction,
    "add a semicolon inside the block"
}
declare_clippy_lint! {
    /// ### What it does
    ///
    /// For () returning expressions, check that the semicolon is outside the block.
    ///
    /// ### Why is this bad?
    ///
    /// For consistency it's best to have the semicolon inside/outside the block. Either way is fine and this lint suggests outside the block.
    /// Take a look at `semicolon_inside_block` for the other alternative.
    ///
    /// ### Example
    ///
    /// ```rust
    /// # fn f(_: u32) {}
    /// # let x = 0;
    /// unsafe { f(x); }
    /// ```
    /// Use instead:
    /// ```rust
    /// # fn f(_: u32) {}
    /// # let x = 0;
    /// unsafe { f(x) };
    /// ```
    #[clippy::version = "1.66.0"]
    pub SEMICOLON_OUTSIDE_BLOCK,
    restriction,
    "add a semicolon outside the block"
}
declare_lint_pass!(SemicolonBlock => [SEMICOLON_INSIDE_BLOCK, SEMICOLON_OUTSIDE_BLOCK]);

impl LateLintPass<'_> for SemicolonBlock {
    fn check_stmt(&mut self, cx: &LateContext<'_>, stmt: &Stmt<'_>) {
        match stmt.kind {
            StmtKind::Expr(Expr {
                kind:
                    ExprKind::Block(
                        block @ Block {
                            expr: None,
                            stmts:
                                &[
                                    ..,
                                    Stmt {
                                        kind: StmtKind::Semi(expr),
                                        span,
                                        ..
                                    },
                                ],
                            ..
                        },
                        _,
                    ),
                ..
            }) if !block.span.from_expansion() => semicolon_outside_block(cx, block, expr, span),
            StmtKind::Semi(Expr {
                kind: ExprKind::Block(block @ Block { expr: Some(tail), .. }, _),
                ..
            }) if !block.span.from_expansion() => semicolon_inside_block(cx, block, tail, stmt.span),
            _ => (),
        }
    }
}

fn semicolon_inside_block(cx: &LateContext<'_>, block: &Block<'_>, tail: &Expr<'_>, semi_span: Span) {
    let insert_span = tail.span.with_lo(tail.span.hi());
    let remove_span = semi_span.with_lo(block.span.hi());

    span_lint_and_then(
        cx,
        SEMICOLON_INSIDE_BLOCK,
        semi_span,
        "consider moving the `;` inside the block for consistent formatting",
        |diag| {
            multispan_sugg_with_applicability(
                diag,
                "put the `;` here",
                Applicability::MachineApplicable,
                [(remove_span, String::new()), (insert_span, ";".to_owned())],
            );
        },
    );
}

fn semicolon_outside_block(cx: &LateContext<'_>, block: &Block<'_>, tail_stmt_expr: &Expr<'_>, semi_span: Span) {
    let insert_span = block.span.with_lo(block.span.hi());
    let remove_span = semi_span.with_lo(tail_stmt_expr.span.source_callsite().hi());

    span_lint_and_then(
        cx,
        SEMICOLON_OUTSIDE_BLOCK,
        block.span,
        "consider moving the `;` outside the block for consistent formatting",
        |diag| {
            multispan_sugg_with_applicability(
                diag,
                "put the `;` here",
                Applicability::MachineApplicable,
                [(remove_span, String::new()), (insert_span, ";".to_owned())],
            );
        },
    );
}
