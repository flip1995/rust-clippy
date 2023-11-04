use clippy_utils::diagnostics::{span_lint_hir, span_lint_hir_and_then};
use clippy_utils::mir::{visit_local_usage, LocalUsage, PossibleBorrowerMap};
use clippy_utils::source::snippet_opt;
use clippy_utils::ty::{has_drop, is_copy, is_type_diagnostic_item, is_type_lang_item, walk_ptrs_ty_depth};
use clippy_utils::{fn_has_unsatisfiable_preds, match_def_path, paths};
use if_chain::if_chain;
use rustc_errors::Applicability;
use rustc_hir::intravisit::FnKind;
use rustc_hir::{def_id, Body, FnDecl, LangItem};
use rustc_lint::{LateContext, LateLintPass};
use rustc_middle::mir;
use rustc_middle::ty::{self, Ty};
use rustc_session::{declare_lint_pass, declare_tool_lint};
use rustc_span::def_id::LocalDefId;
use rustc_span::{sym, BytePos, Span};

macro_rules! unwrap_or_continue {
    ($x:expr) => {
        match $x {
            Some(x) => x,
            None => continue,
        }
    };
}

declare_clippy_lint! {
    /// ### What it does
    /// Checks for a redundant `clone()` (and its relatives) which clones an owned
    /// value that is going to be dropped without further use.
    ///
    /// ### Why is this bad?
    /// It is not always possible for the compiler to eliminate useless
    /// allocations and deallocations generated by redundant `clone()`s.
    ///
    /// ### Known problems
    /// False-negatives: analysis performed by this lint is conservative and limited.
    ///
    /// ### Example
    /// ```no_run
    /// # use std::path::Path;
    /// # #[derive(Clone)]
    /// # struct Foo;
    /// # impl Foo {
    /// #     fn new() -> Self { Foo {} }
    /// # }
    /// # fn call(x: Foo) {}
    /// {
    ///     let x = Foo::new();
    ///     call(x.clone());
    ///     call(x.clone()); // this can just pass `x`
    /// }
    ///
    /// ["lorem", "ipsum"].join(" ").to_string();
    ///
    /// Path::new("/a/b").join("c").to_path_buf();
    /// ```
    #[clippy::version = "1.32.0"]
    pub REDUNDANT_CLONE,
    nursery,
    "`clone()` of an owned value that is going to be dropped immediately"
}

declare_lint_pass!(RedundantClone => [REDUNDANT_CLONE]);

impl<'tcx> LateLintPass<'tcx> for RedundantClone {
    #[expect(clippy::too_many_lines)]
    fn check_fn(
        &mut self,
        cx: &LateContext<'tcx>,
        _: FnKind<'tcx>,
        _: &'tcx FnDecl<'_>,
        _: &'tcx Body<'_>,
        _: Span,
        def_id: LocalDefId,
    ) {
        // Building MIR for `fn`s with unsatisfiable preds results in ICE.
        if fn_has_unsatisfiable_preds(cx, def_id.to_def_id()) {
            return;
        }

        let mir = cx.tcx.optimized_mir(def_id.to_def_id());

        let mut possible_borrower = PossibleBorrowerMap::new(cx, mir);

        for (bb, bbdata) in mir.basic_blocks.iter_enumerated() {
            let terminator = bbdata.terminator();

            if terminator.source_info.span.from_expansion() {
                continue;
            }

            // Give up on loops
            if terminator.successors().any(|s| s == bb) {
                continue;
            }

            let (fn_def_id, arg, arg_ty, clone_ret) =
                unwrap_or_continue!(is_call_with_ref_arg(cx, mir, &terminator.kind));

            let from_borrow = match_def_path(cx, fn_def_id, &paths::CLONE_TRAIT_METHOD)
                || cx.tcx.is_diagnostic_item(sym::to_owned_method, fn_def_id)
                || (cx.tcx.is_diagnostic_item(sym::to_string_method, fn_def_id)
                    && is_type_lang_item(cx, arg_ty, LangItem::String));

            let from_deref = !from_borrow
                && (match_def_path(cx, fn_def_id, &paths::PATH_TO_PATH_BUF)
                    || match_def_path(cx, fn_def_id, &paths::OS_STR_TO_OS_STRING));

            if !from_borrow && !from_deref {
                continue;
            }

            if let ty::Adt(def, _) = arg_ty.kind() {
                if def.is_manually_drop() {
                    continue;
                }
            }

            // `{ arg = &cloned; clone(move arg); }` or `{ arg = &cloned; to_path_buf(arg); }`
            let (cloned, cannot_move_out) = unwrap_or_continue!(find_stmt_assigns_to(cx, mir, arg, from_borrow, bb));

            let loc = mir::Location {
                block: bb,
                statement_index: bbdata.statements.len(),
            };

            // `Local` to be cloned, and a local of `clone` call's destination
            let (local, ret_local) = if from_borrow {
                // `res = clone(arg)` can be turned into `res = move arg;`
                // if `arg` is the only borrow of `cloned` at this point.

                if cannot_move_out || !possible_borrower.only_borrowers(&[arg], cloned, loc) {
                    continue;
                }

                (cloned, clone_ret)
            } else {
                // `arg` is a reference as it is `.deref()`ed in the previous block.
                // Look into the predecessor block and find out the source of deref.

                let ps = &mir.basic_blocks.predecessors()[bb];
                if ps.len() != 1 {
                    continue;
                }
                let pred_terminator = mir[ps[0]].terminator();

                // receiver of the `deref()` call
                let (pred_arg, deref_clone_ret) = if_chain! {
                    if let Some((pred_fn_def_id, pred_arg, pred_arg_ty, res)) =
                        is_call_with_ref_arg(cx, mir, &pred_terminator.kind);
                    if res == cloned;
                    if cx.tcx.is_diagnostic_item(sym::deref_method, pred_fn_def_id);
                    if is_type_diagnostic_item(cx, pred_arg_ty, sym::PathBuf)
                        || is_type_diagnostic_item(cx, pred_arg_ty, sym::OsString);
                    then {
                        (pred_arg, res)
                    } else {
                        continue;
                    }
                };

                let (local, cannot_move_out) =
                    unwrap_or_continue!(find_stmt_assigns_to(cx, mir, pred_arg, true, ps[0]));
                let loc = mir::Location {
                    block: bb,
                    statement_index: mir.basic_blocks[bb].statements.len(),
                };

                // This can be turned into `res = move local` if `arg` and `cloned` are not borrowed
                // at the last statement:
                //
                // ```
                // pred_arg = &local;
                // cloned = deref(pred_arg);
                // arg = &cloned;
                // StorageDead(pred_arg);
                // res = to_path_buf(cloned);
                // ```
                if cannot_move_out || !possible_borrower.only_borrowers(&[arg, cloned], local, loc) {
                    continue;
                }

                (local, deref_clone_ret)
            };

            let clone_usage = if local == ret_local {
                CloneUsage {
                    cloned_used: false,
                    cloned_consume_or_mutate_loc: None,
                    clone_consumed_or_mutated: true,
                }
            } else {
                let clone_usage = visit_clone_usage(local, ret_local, mir, bb);
                if clone_usage.cloned_used && clone_usage.clone_consumed_or_mutated {
                    // cloned value is used, and the clone is modified or moved
                    continue;
                } else if let Some(loc) = clone_usage.cloned_consume_or_mutate_loc {
                    // cloned value is mutated, and the clone is alive.
                    if possible_borrower.local_is_alive_at(ret_local, loc) {
                        continue;
                    }
                }
                clone_usage
            };

            let span = terminator.source_info.span;
            let scope = terminator.source_info.scope;
            let node = mir.source_scopes[scope]
                .local_data
                .as_ref()
                .assert_crate_local()
                .lint_root;

            if_chain! {
                if let Some(snip) = snippet_opt(cx, span);
                if let Some(dot) = snip.rfind('.');
                then {
                    let sugg_span = span.with_lo(
                        span.lo() + BytePos(u32::try_from(dot).unwrap())
                    );
                    let mut app = Applicability::MaybeIncorrect;

                    let call_snip = &snip[dot + 1..];
                    // Machine applicable when `call_snip` looks like `foobar()`
                    if let Some(call_snip) = call_snip.strip_suffix("()").map(str::trim) {
                        if call_snip.as_bytes().iter().all(|b| b.is_ascii_alphabetic() || *b == b'_') {
                            app = Applicability::MachineApplicable;
                        }
                    }

                    span_lint_hir_and_then(cx, REDUNDANT_CLONE, node, sugg_span, "redundant clone", |diag| {
                        diag.span_suggestion(
                            sugg_span,
                            "remove this",
                            "",
                            app,
                        );
                        if clone_usage.cloned_used {
                            diag.span_note(
                                span,
                                "cloned value is neither consumed nor mutated",
                            );
                        } else {
                            diag.span_note(
                                span.with_hi(span.lo() + BytePos(u32::try_from(dot).unwrap())),
                                "this value is dropped without further use",
                            );
                        }
                    });
                } else {
                    span_lint_hir(cx, REDUNDANT_CLONE, node, span, "redundant clone");
                }
            }
        }
    }
}

/// If `kind` is `y = func(x: &T)` where `T: !Copy`, returns `(DefId of func, x, T, y)`.
fn is_call_with_ref_arg<'tcx>(
    cx: &LateContext<'tcx>,
    mir: &'tcx mir::Body<'tcx>,
    kind: &'tcx mir::TerminatorKind<'tcx>,
) -> Option<(def_id::DefId, mir::Local, Ty<'tcx>, mir::Local)> {
    if_chain! {
        if let mir::TerminatorKind::Call { func, args, destination, .. } = kind;
        if args.len() == 1;
        if let mir::Operand::Move(mir::Place { local, .. }) = &args[0];
        if let ty::FnDef(def_id, _) = *func.ty(mir, cx.tcx).kind();
        if let (inner_ty, 1) = walk_ptrs_ty_depth(args[0].ty(mir, cx.tcx));
        if !is_copy(cx, inner_ty);
        then {
            Some((def_id, *local, inner_ty, destination.as_local()?))
        } else {
            None
        }
    }
}

type CannotMoveOut = bool;

/// Finds the first `to = (&)from`, and returns
/// ``Some((from, whether `from` cannot be moved out))``.
fn find_stmt_assigns_to<'tcx>(
    cx: &LateContext<'tcx>,
    mir: &mir::Body<'tcx>,
    to_local: mir::Local,
    by_ref: bool,
    bb: mir::BasicBlock,
) -> Option<(mir::Local, CannotMoveOut)> {
    let rvalue = mir.basic_blocks[bb].statements.iter().rev().find_map(|stmt| {
        if let mir::StatementKind::Assign(box (mir::Place { local, .. }, v)) = &stmt.kind {
            return if *local == to_local { Some(v) } else { None };
        }

        None
    })?;

    match (by_ref, rvalue) {
        (true, mir::Rvalue::Ref(_, _, place)) | (false, mir::Rvalue::Use(mir::Operand::Copy(place))) => {
            Some(base_local_and_movability(cx, mir, *place))
        },
        (false, mir::Rvalue::Ref(_, _, place)) => {
            if let [mir::ProjectionElem::Deref] = place.as_ref().projection {
                Some(base_local_and_movability(cx, mir, *place))
            } else {
                None
            }
        },
        _ => None,
    }
}

/// Extracts and returns the undermost base `Local` of given `place`. Returns `place` itself
/// if it is already a `Local`.
///
/// Also reports whether given `place` cannot be moved out.
fn base_local_and_movability<'tcx>(
    cx: &LateContext<'tcx>,
    mir: &mir::Body<'tcx>,
    place: mir::Place<'tcx>,
) -> (mir::Local, CannotMoveOut) {
    // Dereference. You cannot move things out from a borrowed value.
    let mut deref = false;
    // Accessing a field of an ADT that has `Drop`. Moving the field out will cause E0509.
    let mut field = false;
    // If projection is a slice index then clone can be removed only if the
    // underlying type implements Copy
    let mut slice = false;

    for (base, elem) in place.as_ref().iter_projections() {
        let base_ty = base.ty(&mir.local_decls, cx.tcx).ty;
        deref |= matches!(elem, mir::ProjectionElem::Deref);
        field |= matches!(elem, mir::ProjectionElem::Field(..)) && has_drop(cx, base_ty);
        slice |= matches!(elem, mir::ProjectionElem::Index(..)) && !is_copy(cx, base_ty);
    }

    (place.local, deref || field || slice)
}

#[derive(Default)]
struct CloneUsage {
    /// Whether the cloned value is used after the clone.
    cloned_used: bool,
    /// The first location where the cloned value is consumed or mutated, if any.
    cloned_consume_or_mutate_loc: Option<mir::Location>,
    /// Whether the clone value is mutated.
    clone_consumed_or_mutated: bool,
}

fn visit_clone_usage(cloned: mir::Local, clone: mir::Local, mir: &mir::Body<'_>, bb: mir::BasicBlock) -> CloneUsage {
    if let Some((
        LocalUsage {
            local_use_locs: cloned_use_locs,
            local_consume_or_mutate_locs: cloned_consume_or_mutate_locs,
        },
        LocalUsage {
            local_use_locs: _,
            local_consume_or_mutate_locs: clone_consume_or_mutate_locs,
        },
    )) = visit_local_usage(
        &[cloned, clone],
        mir,
        mir::Location {
            block: bb,
            statement_index: mir.basic_blocks[bb].statements.len(),
        },
    )
    .map(|mut vec| (vec.remove(0), vec.remove(0)))
    {
        CloneUsage {
            cloned_used: !cloned_use_locs.is_empty(),
            cloned_consume_or_mutate_loc: cloned_consume_or_mutate_locs.first().copied(),
            // Consider non-temporary clones consumed.
            // TODO: Actually check for mutation of non-temporaries.
            clone_consumed_or_mutated: mir.local_kind(clone) != mir::LocalKind::Temp
                || !clone_consume_or_mutate_locs.is_empty(),
        }
    } else {
        CloneUsage {
            cloned_used: true,
            cloned_consume_or_mutate_loc: None,
            clone_consumed_or_mutated: true,
        }
    }
}
