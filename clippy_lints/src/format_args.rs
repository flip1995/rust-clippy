use clippy_utils::diagnostics::{span_lint_and_sugg, span_lint_and_then};
use clippy_utils::higher::{FormatArgsArg, FormatArgsExpn, FormatExpn};
use clippy_utils::source::snippet_opt;
use clippy_utils::ty::implements_trait;
use clippy_utils::{is_diag_trait_item, match_def_path, paths};
use if_chain::if_chain;
use rustc_errors::Applicability;
use rustc_hir::{Expr, ExprKind};
use rustc_lint::{LateContext, LateLintPass};
use rustc_middle::ty::adjustment::{Adjust, Adjustment};
use rustc_middle::ty::Ty;
use rustc_session::{declare_lint_pass, declare_tool_lint};
use rustc_span::{sym, BytePos, ExpnData, ExpnKind, Span, Symbol};

declare_clippy_lint! {
    /// ### What it does
    /// Detects `format!` within the arguments of another macro that does
    /// formatting such as `format!` itself, `write!` or `println!`. Suggests
    /// inlining the `format!` call.
    ///
    /// ### Why is this bad?
    /// The recommended code is both shorter and avoids a temporary allocation.
    ///
    /// ### Example
    /// ```rust
    /// # use std::panic::Location;
    /// println!("error: {}", format!("something failed at {}", Location::caller()));
    /// ```
    /// Use instead:
    /// ```rust
    /// # use std::panic::Location;
    /// println!("error: something failed at {}", Location::caller());
    /// ```
    pub FORMAT_IN_FORMAT_ARGS,
    perf,
    "`format!` used in a macro that does formatting"
}

declare_clippy_lint! {
    /// ### What it does
    /// Checks for [`ToString::to_string`](https://doc.rust-lang.org/std/string/trait.ToString.html#tymethod.to_string)
    /// applied to a type that implements [`Display`](https://doc.rust-lang.org/std/fmt/trait.Display.html)
    /// in a macro that does formatting.
    ///
    /// ### Why is this bad?
    /// Since the type implements `Display`, the use of `to_string` is
    /// unnecessary.
    ///
    /// ### Example
    /// ```rust
    /// # use std::panic::Location;
    /// println!("error: something failed at {}", Location::caller().to_string());
    /// ```
    /// Use instead:
    /// ```rust
    /// # use std::panic::Location;
    /// println!("error: something failed at {}", Location::caller());
    /// ```
    pub TO_STRING_IN_FORMAT_ARGS,
    perf,
    "`to_string` applied to a type that implements `Display` in format args"
}

declare_lint_pass!(FormatArgs => [FORMAT_IN_FORMAT_ARGS, TO_STRING_IN_FORMAT_ARGS]);

const FORMAT_MACRO_PATHS: &[&[&str]] = &[
    &paths::FORMAT_ARGS_MACRO,
    &paths::ASSERT_EQ_MACRO,
    &paths::ASSERT_MACRO,
    &paths::ASSERT_NE_MACRO,
    &paths::EPRINT_MACRO,
    &paths::EPRINTLN_MACRO,
    &paths::PRINT_MACRO,
    &paths::PRINTLN_MACRO,
    &paths::WRITE_MACRO,
    &paths::WRITELN_MACRO,
];

const FORMAT_MACRO_DIAG_ITEMS: &[Symbol] = &[sym::format_macro, sym::std_panic_macro];

impl<'tcx> LateLintPass<'tcx> for FormatArgs {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) {
        if_chain! {
            if let Some(format_args) = FormatArgsExpn::parse(expr);
            let expr_expn_data = expr.span.ctxt().outer_expn_data();
            let outermost_expn_data = outermost_expn_data(expr_expn_data);
            if let Some(macro_def_id) = outermost_expn_data.macro_def_id;
            if FORMAT_MACRO_PATHS
                .iter()
                .any(|path| match_def_path(cx, macro_def_id, path))
                || FORMAT_MACRO_DIAG_ITEMS
                    .iter()
                    .any(|diag_item| cx.tcx.is_diagnostic_item(*diag_item, macro_def_id));
            if let ExpnKind::Macro(_, name) = outermost_expn_data.kind;
            if let Some(args) = format_args.args();
            then {
                for (i, arg) in args.iter().enumerate() {
                    if !arg.is_display() {
                        continue;
                    }
                    if arg.has_string_formatting() {
                        continue;
                    }
                    if is_aliased(&args, i) {
                        continue;
                    }
                    check_format_in_format_args(cx, outermost_expn_data.call_site, name, arg);
                    check_to_string_in_format_args(cx, name, arg);
                }
            }
        }
    }
}

fn outermost_expn_data(expn_data: ExpnData) -> ExpnData {
    if expn_data.call_site.from_expansion() {
        outermost_expn_data(expn_data.call_site.ctxt().outer_expn_data())
    } else {
        expn_data
    }
}

fn check_format_in_format_args(cx: &LateContext<'_>, call_site: Span, name: Symbol, arg: &FormatArgsArg<'_>) {
    if_chain! {
        if FormatExpn::parse(arg.value).is_some();
        if !arg.value.span.ctxt().outer_expn_data().call_site.from_expansion();
        then {
            span_lint_and_then(
                cx,
                FORMAT_IN_FORMAT_ARGS,
                trim_semicolon(cx, call_site),
                &format!("`format!` in `{}!` args", name),
                |diag| {
                    diag.help(&format!(
                        "combine the `format!(..)` arguments with the outer `{}!(..)` call",
                        name
                    ));
                    diag.help("or consider changing `format!` to `format_args!`");
                },
            );
        }
    }
}

fn check_to_string_in_format_args<'tcx>(cx: &LateContext<'tcx>, name: Symbol, arg: &FormatArgsArg<'tcx>) {
    let value = arg.value;
    if_chain! {
        if !value.span.from_expansion();
        if let ExprKind::MethodCall(_, _, [receiver], _) = value.kind;
        if let Some(method_def_id) = cx.typeck_results().type_dependent_def_id(value.hir_id);
        if is_diag_trait_item(cx, method_def_id, sym::ToString);
        let receiver_ty = cx.typeck_results().expr_ty(receiver);
        if let Some(display_trait_id) = cx.tcx.get_diagnostic_item(sym::Display);
        if let Some(receiver_snippet) = snippet_opt(cx, receiver.span);
        then {
            let (n_needed_derefs, target) = count_needed_derefs(
                receiver_ty,
                cx.typeck_results().expr_adjustments(receiver).iter(),
            );
            if implements_trait(cx, target, display_trait_id, &[]) {
                if n_needed_derefs == 0 {
                    span_lint_and_sugg(
                        cx,
                        TO_STRING_IN_FORMAT_ARGS,
                        value.span.with_lo(receiver.span.hi()),
                        &format!("`to_string` applied to a type that implements `Display` in `{}!` args", name),
                        "remove this",
                        String::new(),
                        Applicability::MachineApplicable,
                    );
                } else {
                    span_lint_and_sugg(
                        cx,
                        TO_STRING_IN_FORMAT_ARGS,
                        value.span,
                        &format!("`to_string` applied to a type that implements `Display` in `{}!` args", name),
                        "use this",
                        format!("{:*>width$}{}", "", receiver_snippet, width = n_needed_derefs),
                        Applicability::MachineApplicable,
                    );
                }
            }
        }
    }
}

// Returns true if `args[i]` "refers to" or "is referred to by" another argument.
fn is_aliased(args: &[FormatArgsArg<'_>], i: usize) -> bool {
    let value = args[i].value;
    args.iter()
        .enumerate()
        .any(|(j, arg)| i != j && std::ptr::eq(value, arg.value))
}

fn trim_semicolon(cx: &LateContext<'_>, span: Span) -> Span {
    snippet_opt(cx, span).map_or(span, |snippet| {
        let snippet = snippet.trim_end_matches(';');
        span.with_hi(span.lo() + BytePos(u32::try_from(snippet.len()).unwrap()))
    })
}

fn count_needed_derefs<'tcx, I>(mut ty: Ty<'tcx>, mut iter: I) -> (usize, Ty<'tcx>)
where
    I: Iterator<Item = &'tcx Adjustment<'tcx>>,
{
    let mut n_total = 0;
    let mut n_needed = 0;
    loop {
        if let Some(Adjustment {
            kind: Adjust::Deref(overloaded_deref),
            target,
        }) = iter.next()
        {
            n_total += 1;
            if overloaded_deref.is_some() {
                n_needed = n_total;
            }
            ty = target;
        } else {
            return (n_needed, ty);
        }
    }
}
