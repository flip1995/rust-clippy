use clippy_utils::diagnostics::span_lint_and_then;
use clippy_utils::sugg::Sugg;
use clippy_utils::{
    get_enclosing_block, is_expr_path_def_path, is_integer_literal, is_path_diagnostic_item, path_to_local,
    path_to_local_id, paths, SpanlessEq,
};
use if_chain::if_chain;
use rustc_errors::Applicability;
use rustc_hir::intravisit::{walk_block, walk_expr, walk_stmt, Visitor};
use rustc_hir::{BindingAnnotation, Block, Expr, ExprKind, HirId, PatKind, Stmt, StmtKind};
use rustc_lint::{LateContext, LateLintPass};
use rustc_session::{declare_lint_pass, declare_tool_lint};
use rustc_span::symbol::sym;

declare_clippy_lint! {
    /// ### What it does
    /// Checks slow zero-filled vector initialization
    ///
    /// ### Why is this bad?
    /// These structures are non-idiomatic and less efficient than simply using
    /// `vec![0; len]`.
    ///
    /// ### Example
    /// ```rust
    /// # use core::iter::repeat;
    /// # let len = 4;
    /// let mut vec1 = Vec::with_capacity(len);
    /// vec1.resize(len, 0);
    ///
    /// let mut vec1 = Vec::with_capacity(len);
    /// vec1.resize(vec1.capacity(), 0);
    ///
    /// let mut vec2 = Vec::with_capacity(len);
    /// vec2.extend(repeat(0).take(len));
    /// ```
    ///
    /// Use instead:
    /// ```rust
    /// # let len = 4;
    /// let mut vec1 = vec![0; len];
    /// let mut vec2 = vec![0; len];
    /// ```
    #[clippy::version = "1.32.0"]
    pub SLOW_VECTOR_INITIALIZATION,
    perf,
    "slow vector initialization"
}

declare_lint_pass!(SlowVectorInit => [SLOW_VECTOR_INITIALIZATION]);

/// `VecAllocation` contains data regarding a vector allocated with `with_capacity` and then
/// assigned to a variable. For example, `let mut vec = Vec::with_capacity(0)` or
/// `vec = Vec::with_capacity(0)`
struct VecAllocation<'tcx> {
    /// HirId of the variable
    local_id: HirId,

    /// Reference to the expression which allocates the vector
    allocation_expr: &'tcx Expr<'tcx>,

    /// Reference to the expression used as argument on `with_capacity` call. This is used
    /// to only match slow zero-filling idioms of the same length than vector initialization.
    size_expr: InitializedSize<'tcx>,
}

/// Initializer for the creation of the vector.
///
/// When `Vec::with_capacity(size)` is found, the `size` expression will be in
/// `InitializedSize::Initialized`.
///
/// Otherwise, for `Vec::new()` calls, there is no allocation initializer yet, so
/// `InitializedSize::Uninitialized` is used.
/// Later, when a call to `.resize(size, 0)` or similar is found, it's set
/// to `InitializedSize::Initialized(size)`.
///
/// Since it will be set to `InitializedSize::Initialized(size)` when a slow initialization is
/// found, it is always safe to "unwrap" it at lint time.
enum InitializedSize<'tcx> {
    Initialized(&'tcx Expr<'tcx>),
    Uninitialized,
}

/// Type of slow initialization
enum InitializationType<'tcx> {
    /// Extend is a slow initialization with the form `vec.extend(repeat(0).take(..))`
    Extend(&'tcx Expr<'tcx>),

    /// Resize is a slow initialization with the form `vec.resize(.., 0)`
    Resize(&'tcx Expr<'tcx>),
}

impl<'tcx> LateLintPass<'tcx> for SlowVectorInit {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'_>) {
        // Matches initialization on reassignments. For example: `vec = Vec::with_capacity(100)`
        if_chain! {
            if let ExprKind::Assign(left, right, _) = expr.kind;
            if let Some(local_id) = path_to_local(left);
            if let Some(size_expr) = Self::as_vec_initializer(cx, right);

            then {
                let vi = VecAllocation {
                    local_id,
                    allocation_expr: right,
                    size_expr,
                };

                Self::search_initialization(cx, vi, expr.hir_id);
            }
        }
    }

    fn check_stmt(&mut self, cx: &LateContext<'tcx>, stmt: &'tcx Stmt<'_>) {
        // Matches statements which initializes vectors. For example: `let mut vec = Vec::with_capacity(10)`
        // or `Vec::new()`
        if_chain! {
            if let StmtKind::Local(local) = stmt.kind;
            if let PatKind::Binding(BindingAnnotation::MUT, local_id, _, None) = local.pat.kind;
            if let Some(init) = local.init;
            if let Some(size_expr) = Self::as_vec_initializer(cx, init);

            then {
                let vi = VecAllocation {
                    local_id,
                    allocation_expr: init,
                    size_expr,
                };

                Self::search_initialization(cx, vi, stmt.hir_id);
            }
        }
    }
}

impl SlowVectorInit {
    /// Looks for `Vec::with_capacity(size)` or `Vec::new()` calls and returns the initialized size,
    /// if any. More specifically, it returns:
    /// - `Some(InitializedSize::Initialized(size))` for `Vec::with_capacity(size)`
    /// - `Some(InitializedSize::Uninitialized)` for `Vec::new()`
    /// - `None` for other, unrelated kinds of expressions
    fn as_vec_initializer<'tcx>(cx: &LateContext<'_>, expr: &'tcx Expr<'tcx>) -> Option<InitializedSize<'tcx>> {
        if let ExprKind::Call(func, [len_expr]) = expr.kind
            && is_expr_path_def_path(cx, func, &paths::VEC_WITH_CAPACITY)
        {
            Some(InitializedSize::Initialized(len_expr))
        } else if matches!(expr.kind, ExprKind::Call(func, _) if is_expr_path_def_path(cx, func, &paths::VEC_NEW)) {
            Some(InitializedSize::Uninitialized)
        } else {
            None
        }
    }

    /// Search initialization for the given vector
    fn search_initialization<'tcx>(cx: &LateContext<'tcx>, vec_alloc: VecAllocation<'tcx>, parent_node: HirId) {
        let enclosing_body = get_enclosing_block(cx, parent_node);

        if enclosing_body.is_none() {
            return;
        }

        let mut v = VectorInitializationVisitor {
            cx,
            vec_alloc,
            slow_expression: None,
            initialization_found: false,
        };

        v.visit_block(enclosing_body.unwrap());

        if let Some(ref allocation_expr) = v.slow_expression {
            Self::lint_initialization(cx, allocation_expr, &v.vec_alloc);
        }
    }

    fn lint_initialization<'tcx>(
        cx: &LateContext<'tcx>,
        initialization: &InitializationType<'tcx>,
        vec_alloc: &VecAllocation<'_>,
    ) {
        match initialization {
            InitializationType::Extend(e) | InitializationType::Resize(e) => {
                Self::emit_lint(cx, e, vec_alloc, "slow zero-filling initialization");
            },
        };
    }

    fn emit_lint(cx: &LateContext<'_>, slow_fill: &Expr<'_>, vec_alloc: &VecAllocation<'_>, msg: &str) {
        let len_expr = Sugg::hir(
            cx,
            match vec_alloc.size_expr {
                InitializedSize::Initialized(expr) => expr,
                InitializedSize::Uninitialized => unreachable!("size expression must be set by this point"),
            },
            "len",
        );

        span_lint_and_then(cx, SLOW_VECTOR_INITIALIZATION, slow_fill.span, msg, |diag| {
            diag.span_suggestion(
                vec_alloc.allocation_expr.span,
                "consider replacing this with",
                format!("vec![0; {len_expr}]"),
                Applicability::Unspecified,
            );
        });
    }
}

/// `VectorInitializationVisitor` searches for unsafe or slow vector initializations for the given
/// vector.
struct VectorInitializationVisitor<'a, 'tcx> {
    cx: &'a LateContext<'tcx>,

    /// Contains the information.
    vec_alloc: VecAllocation<'tcx>,

    /// Contains the slow initialization expression, if one was found.
    slow_expression: Option<InitializationType<'tcx>>,

    /// `true` if the initialization of the vector has been found on the visited block.
    initialization_found: bool,
}

impl<'a, 'tcx> VectorInitializationVisitor<'a, 'tcx> {
    /// Checks if the given expression is extending a vector with `repeat(0).take(..)`
    fn search_slow_extend_filling(&mut self, expr: &'tcx Expr<'_>) {
        if_chain! {
            if self.initialization_found;
            if let ExprKind::MethodCall(path, self_arg, [extend_arg], _) = expr.kind;
            if path_to_local_id(self_arg, self.vec_alloc.local_id);
            if path.ident.name == sym!(extend);
            if self.is_repeat_take(extend_arg);

            then {
                self.slow_expression = Some(InitializationType::Extend(expr));
            }
        }
    }

    /// Checks if the given expression is resizing a vector with 0
    fn search_slow_resize_filling(&mut self, expr: &'tcx Expr<'tcx>) {
        if self.initialization_found
            && let ExprKind::MethodCall(path, self_arg, [len_arg, fill_arg], _) = expr.kind
            && path_to_local_id(self_arg, self.vec_alloc.local_id)
            && path.ident.name == sym!(resize)
            // Check that is filled with 0
            && is_integer_literal(fill_arg, 0)
        {
            let is_matching_resize = if let InitializedSize::Initialized(size_expr) = self.vec_alloc.size_expr {
                // If we have a size expression, check that it is equal to what's passed to `resize`
                SpanlessEq::new(self.cx).eq_expr(len_arg, size_expr)
                    || matches!(len_arg.kind, ExprKind::MethodCall(path, ..) if path.ident.as_str() == "capacity")
            } else {
                self.vec_alloc.size_expr = InitializedSize::Initialized(len_arg);
                true
            };

            if is_matching_resize {
                self.slow_expression = Some(InitializationType::Resize(expr));
            }
        }
    }

    /// Returns `true` if give expression is `repeat(0).take(...)`
    fn is_repeat_take(&mut self, expr: &'tcx Expr<'tcx>) -> bool {
        if_chain! {
            if let ExprKind::MethodCall(take_path, recv, [len_arg, ..], _) = expr.kind;
            if take_path.ident.name == sym!(take);
            // Check that take is applied to `repeat(0)`
            if self.is_repeat_zero(recv);
            then {
                if let InitializedSize::Initialized(size_expr) = self.vec_alloc.size_expr {
                    // Check that len expression is equals to `with_capacity` expression
                    return SpanlessEq::new(self.cx).eq_expr(len_arg, size_expr)
                        || matches!(len_arg.kind, ExprKind::MethodCall(path, ..) if path.ident.as_str() == "capacity")
                }

                self.vec_alloc.size_expr = InitializedSize::Initialized(len_arg);
                return true;
            }
        }

        false
    }

    /// Returns `true` if given expression is `repeat(0)`
    fn is_repeat_zero(&self, expr: &Expr<'_>) -> bool {
        if_chain! {
            if let ExprKind::Call(fn_expr, [repeat_arg]) = expr.kind;
            if is_path_diagnostic_item(self.cx, fn_expr, sym::iter_repeat);
            if is_integer_literal(repeat_arg, 0);
            then {
                true
            } else {
                false
            }
        }
    }
}

impl<'a, 'tcx> Visitor<'tcx> for VectorInitializationVisitor<'a, 'tcx> {
    fn visit_stmt(&mut self, stmt: &'tcx Stmt<'_>) {
        if self.initialization_found {
            match stmt.kind {
                StmtKind::Expr(expr) | StmtKind::Semi(expr) => {
                    self.search_slow_extend_filling(expr);
                    self.search_slow_resize_filling(expr);
                },
                _ => (),
            }

            self.initialization_found = false;
        } else {
            walk_stmt(self, stmt);
        }
    }

    fn visit_block(&mut self, block: &'tcx Block<'_>) {
        if self.initialization_found {
            if let Some(s) = block.stmts.get(0) {
                self.visit_stmt(s);
            }

            self.initialization_found = false;
        } else {
            walk_block(self, block);
        }
    }

    fn visit_expr(&mut self, expr: &'tcx Expr<'_>) {
        // Skip all the expressions previous to the vector initialization
        if self.vec_alloc.allocation_expr.hir_id == expr.hir_id {
            self.initialization_found = true;
        }

        walk_expr(self, expr);
    }
}
