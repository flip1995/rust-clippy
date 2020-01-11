//! Code related to match expressions. These are sufficiently complex to
//! warrant their own module and submodules. :) This main module includes the
//! high-level algorithm, the submodules contain the details.
//!
//! This also includes code for pattern bindings in `let` statements and
//! function parameters.

use crate::build::scope::DropKind;
use crate::build::ForGuard::{self, OutsideGuard, RefWithinGuard};
use crate::build::{BlockAnd, BlockAndExtension, Builder};
use crate::build::{GuardFrame, GuardFrameLocal, LocalsForNode};
use crate::hair::{self, *};
use rustc::middle::region;
use rustc::mir::*;
use rustc::ty::layout::VariantIdx;
use rustc::ty::{self, CanonicalUserTypeAnnotation, Ty};
use rustc_data_structures::fx::{FxHashMap, FxHashSet};
use rustc_hir::HirId;
use rustc_index::bit_set::BitSet;
use rustc_span::Span;
use smallvec::{smallvec, SmallVec};
use syntax::ast::Name;

// helper functions, broken out by category:
mod simplify;
mod test;
mod util;

use std::borrow::Borrow;
use std::convert::TryFrom;
use std::mem;

impl<'a, 'tcx> Builder<'a, 'tcx> {
    /// Generates MIR for a `match` expression.
    ///
    /// The MIR that we generate for a match looks like this.
    ///
    /// ```text
    /// [ 0. Pre-match ]
    ///        |
    /// [ 1. Evaluate Scrutinee (expression being matched on) ]
    /// [ (fake read of scrutinee) ]
    ///        |
    /// [ 2. Decision tree -- check discriminants ] <--------+
    ///        |                                             |
    ///        | (once a specific arm is chosen)             |
    ///        |                                             |
    /// [pre_binding_block]                           [otherwise_block]
    ///        |                                             |
    /// [ 3. Create "guard bindings" for arm ]               |
    /// [ (create fake borrows) ]                            |
    ///        |                                             |
    /// [ 4. Execute guard code ]                            |
    /// [ (read fake borrows) ] --(guard is false)-----------+
    ///        |
    ///        | (guard results in true)
    ///        |
    /// [ 5. Create real bindings and execute arm ]
    ///        |
    /// [ Exit match ]
    /// ```
    ///
    /// All of the different arms have been stacked on top of each other to
    /// simplify the diagram. For an arm with no guard the blocks marked 3 and
    /// 4 and the fake borrows are omitted.
    ///
    /// We generate MIR in the following steps:
    ///
    /// 1. Evaluate the scrutinee and add the fake read of it ([Builder::lower_scrutinee]).
    /// 2. Create the decision tree ([Builder::lower_match_tree]).
    /// 3. Determine the fake borrows that are needed from the places that were
    ///    matched against and create the required temporaries for them
    ///    ([Builder::calculate_fake_borrows]).
    /// 4. Create everything else: the guards and the arms ([Builder::lower_match_arms]).
    ///
    /// ## False edges
    ///
    /// We don't want to have the exact structure of the decision tree be
    /// visible through borrow checking. False edges ensure that the CFG as
    /// seen by borrow checking doesn't encode this. False edges are added:
    ///
    /// * From each prebinding block to the next prebinding block.
    /// * From each otherwise block to the next prebinding block.
    crate fn match_expr(
        &mut self,
        destination: &Place<'tcx>,
        span: Span,
        mut block: BasicBlock,
        scrutinee: ExprRef<'tcx>,
        arms: Vec<Arm<'tcx>>,
    ) -> BlockAnd<()> {
        let scrutinee_span = scrutinee.span();
        let scrutinee_place =
            unpack!(block = self.lower_scrutinee(block, scrutinee, scrutinee_span,));

        let mut arm_candidates = self.create_match_candidates(scrutinee_place, &arms);

        let match_has_guard = arms.iter().any(|arm| arm.guard.is_some());
        let mut candidates =
            arm_candidates.iter_mut().map(|(_, candidate)| candidate).collect::<Vec<_>>();

        let fake_borrow_temps =
            self.lower_match_tree(block, scrutinee_span, match_has_guard, &mut candidates);

        self.lower_match_arms(
            destination,
            scrutinee_place,
            scrutinee_span,
            arm_candidates,
            self.source_info(span),
            fake_borrow_temps,
        )
    }

    /// Evaluate the scrutinee and add the fake read of it.
    fn lower_scrutinee(
        &mut self,
        mut block: BasicBlock,
        scrutinee: ExprRef<'tcx>,
        scrutinee_span: Span,
    ) -> BlockAnd<Place<'tcx>> {
        let scrutinee_place = unpack!(block = self.as_place(block, scrutinee));
        // Matching on a `scrutinee_place` with an uninhabited type doesn't
        // generate any memory reads by itself, and so if the place "expression"
        // contains unsafe operations like raw pointer dereferences or union
        // field projections, we wouldn't know to require an `unsafe` block
        // around a `match` equivalent to `std::intrinsics::unreachable()`.
        // See issue #47412 for this hole being discovered in the wild.
        //
        // HACK(eddyb) Work around the above issue by adding a dummy inspection
        // of `scrutinee_place`, specifically by applying `ReadForMatch`.
        //
        // NOTE: ReadForMatch also checks that the scrutinee is initialized.
        // This is currently needed to not allow matching on an uninitialized,
        // uninhabited value. If we get never patterns, those will check that
        // the place is initialized, and so this read would only be used to
        // check safety.
        let cause_matched_place = FakeReadCause::ForMatchedPlace;
        let source_info = self.source_info(scrutinee_span);
        self.cfg.push_fake_read(block, source_info, cause_matched_place, scrutinee_place);

        block.and(scrutinee_place)
    }

    /// Create the initial `Candidate`s for a `match` expression.
    fn create_match_candidates<'pat>(
        &mut self,
        scrutinee: Place<'tcx>,
        arms: &'pat [Arm<'tcx>],
    ) -> Vec<(&'pat Arm<'tcx>, Candidate<'pat, 'tcx>)> {
        // Assemble a list of candidates: there is one candidate per pattern,
        // which means there may be more than one candidate *per arm*.
        arms.iter()
            .map(|arm| {
                let arm_has_guard = arm.guard.is_some();
                let arm_candidate = Candidate::new(scrutinee, &arm.pattern, arm_has_guard);
                (arm, arm_candidate)
            })
            .collect()
    }

    /// Create the decision tree for the match expression, starting from `block`.
    ///
    /// Modifies `candidates` to store the bindings and type ascriptions for
    /// that candidate.
    ///
    /// Returns the places that need fake borrows because we bind or test them.
    fn lower_match_tree<'pat>(
        &mut self,
        block: BasicBlock,
        scrutinee_span: Span,
        match_has_guard: bool,
        candidates: &mut [&mut Candidate<'pat, 'tcx>],
    ) -> Vec<(Place<'tcx>, Local)> {
        // The set of places that we are creating fake borrows of. If there are
        // no match guards then we don't need any fake borrows, so don't track
        // them.
        let mut fake_borrows = if match_has_guard { Some(FxHashSet::default()) } else { None };

        let mut otherwise = None;

        // This will generate code to test scrutinee_place and
        // branch to the appropriate arm block
        self.match_candidates(scrutinee_span, block, &mut otherwise, candidates, &mut fake_borrows);

        if let Some(otherwise_block) = otherwise {
            // See the doc comment on `match_candidates` for why we may have an
            // otherwise block. Match checking will ensure this is actually
            // unreachable.
            let source_info = self.source_info(scrutinee_span);
            self.cfg.terminate(otherwise_block, source_info, TerminatorKind::Unreachable);
        }

        // Link each leaf candidate to the `pre_binding_block` of the next one.
        let mut previous_candidate: Option<&mut Candidate<'_, '_>> = None;

        for candidate in candidates {
            candidate.visit_leaves(|leaf_candidate| {
                if let Some(ref mut prev) = previous_candidate {
                    prev.next_candidate_pre_binding_block = leaf_candidate.pre_binding_block;
                }
                previous_candidate = Some(leaf_candidate);
            });
        }

        if let Some(ref borrows) = fake_borrows {
            self.calculate_fake_borrows(borrows, scrutinee_span)
        } else {
            Vec::new()
        }
    }

    /// Lower the bindings, guards and arm bodies of a `match` expression.
    ///
    /// The decision tree should have already been created
    /// (by [Builder::lower_match_tree]).
    ///
    /// `outer_source_info` is the SourceInfo for the whole match.
    fn lower_match_arms(
        &mut self,
        destination: &Place<'tcx>,
        scrutinee_place: Place<'tcx>,
        scrutinee_span: Span,
        arm_candidates: Vec<(&'_ Arm<'tcx>, Candidate<'_, 'tcx>)>,
        outer_source_info: SourceInfo,
        fake_borrow_temps: Vec<(Place<'tcx>, Local)>,
    ) -> BlockAnd<()> {
        let match_scope = self.scopes.topmost();

        let arm_end_blocks: Vec<_> = arm_candidates
            .into_iter()
            .map(|(arm, candidate)| {
                debug!("lowering arm {:?}\ncanidate = {:?}", arm, candidate);

                let arm_source_info = self.source_info(arm.span);
                let arm_scope = (arm.scope, arm_source_info);
                self.in_scope(arm_scope, arm.lint_level, |this| {
                    let body = this.hir.mirror(arm.body.clone());
                    let scope = this.declare_bindings(
                        None,
                        arm.span,
                        &arm.pattern,
                        ArmHasGuard(arm.guard.is_some()),
                        Some((Some(&scrutinee_place), scrutinee_span)),
                    );

                    let arm_block = this.bind_pattern(
                        outer_source_info,
                        candidate,
                        arm.guard.as_ref().map(|g| (g, match_scope)),
                        &fake_borrow_temps,
                        scrutinee_span,
                        Some(arm.scope),
                    );

                    if let Some(source_scope) = scope {
                        this.source_scope = source_scope;
                    }

                    this.into(destination, arm_block, body)
                })
            })
            .collect();

        // all the arm blocks will rejoin here
        let end_block = self.cfg.start_new_block();

        for arm_block in arm_end_blocks {
            self.cfg.goto(unpack!(arm_block), outer_source_info, end_block);
        }

        self.source_scope = outer_source_info.scope;

        end_block.unit()
    }

    /// Binds the variables and ascribes types for a given `match` arm.
    ///
    /// Also check if the guard matches, if it's provided.
    fn bind_pattern(
        &mut self,
        outer_source_info: SourceInfo,
        candidate: Candidate<'_, 'tcx>,
        guard: Option<(&Guard<'tcx>, region::Scope)>,
        fake_borrow_temps: &Vec<(Place<'tcx>, Local)>,
        scrutinee_span: Span,
        arm_scope: Option<region::Scope>,
    ) -> BasicBlock {
        if candidate.subcandidates.is_empty() {
            // Avoid generating another `BasicBlock` when we only have one
            // candidate.
            self.bind_and_guard_matched_candidate(
                candidate,
                &[],
                guard,
                fake_borrow_temps,
                scrutinee_span,
                true,
            )
        } else {
            let target_block = self.cfg.start_new_block();
            let mut schedule_drops = true;
            // We keep a stack of all of the bindings and type asciptions
            // from the the parent candidates that we visit, that also need to
            // be bound for each candidate.
            traverse_candidate(
                candidate,
                &mut Vec::new(),
                &mut |leaf_candidate, parent_bindings| {
                    if let Some(arm_scope) = arm_scope {
                        // Avoid scheduling drops multiple times by unscheduling drops.
                        self.clear_top_scope(arm_scope);
                    }
                    let binding_end = self.bind_and_guard_matched_candidate(
                        leaf_candidate,
                        parent_bindings,
                        guard,
                        &fake_borrow_temps,
                        scrutinee_span,
                        schedule_drops,
                    );
                    if arm_scope.is_none() {
                        // If we aren't in a match, then our bindings may not be
                        // the only thing in the top scope, so only schedule
                        // them to drop for the first pattern instead.
                        schedule_drops = false;
                    }
                    self.cfg.goto(binding_end, outer_source_info, target_block);
                },
                |inner_candidate, parent_bindings| {
                    parent_bindings.push((inner_candidate.bindings, inner_candidate.ascriptions));
                    inner_candidate.subcandidates.into_iter()
                },
                |parent_bindings| {
                    parent_bindings.pop();
                },
            );

            target_block
        }
    }

    pub(super) fn expr_into_pattern(
        &mut self,
        mut block: BasicBlock,
        irrefutable_pat: Pat<'tcx>,
        initializer: ExprRef<'tcx>,
    ) -> BlockAnd<()> {
        match *irrefutable_pat.kind {
            // Optimize the case of `let x = ...` to write directly into `x`
            PatKind::Binding { mode: BindingMode::ByValue, var, subpattern: None, .. } => {
                let place =
                    self.storage_live_binding(block, var, irrefutable_pat.span, OutsideGuard);
                unpack!(block = self.into(&place, block, initializer));

                // Inject a fake read, see comments on `FakeReadCause::ForLet`.
                let source_info = self.source_info(irrefutable_pat.span);
                self.cfg.push_fake_read(block, source_info, FakeReadCause::ForLet, place);

                self.schedule_drop_for_binding(var, irrefutable_pat.span, OutsideGuard);
                block.unit()
            }

            // Optimize the case of `let x: T = ...` to write directly
            // into `x` and then require that `T == typeof(x)`.
            //
            // Weirdly, this is needed to prevent the
            // `intrinsic-move-val.rs` test case from crashing. That
            // test works with uninitialized values in a rather
            // dubious way, so it may be that the test is kind of
            // broken.
            PatKind::AscribeUserType {
                subpattern:
                    Pat {
                        kind:
                            box PatKind::Binding {
                                mode: BindingMode::ByValue,
                                var,
                                subpattern: None,
                                ..
                            },
                        ..
                    },
                ascription:
                    hair::pattern::Ascription { user_ty: pat_ascription_ty, variance: _, user_ty_span },
            } => {
                let place =
                    self.storage_live_binding(block, var, irrefutable_pat.span, OutsideGuard);
                unpack!(block = self.into(&place, block, initializer));

                // Inject a fake read, see comments on `FakeReadCause::ForLet`.
                let pattern_source_info = self.source_info(irrefutable_pat.span);
                let cause_let = FakeReadCause::ForLet;
                self.cfg.push_fake_read(block, pattern_source_info, cause_let, place);

                let ty_source_info = self.source_info(user_ty_span);
                let user_ty = pat_ascription_ty.user_ty(
                    &mut self.canonical_user_type_annotations,
                    place.ty(&self.local_decls, self.hir.tcx()).ty,
                    ty_source_info.span,
                );
                self.cfg.push(
                    block,
                    Statement {
                        source_info: ty_source_info,
                        kind: StatementKind::AscribeUserType(
                            box (place, user_ty),
                            // We always use invariant as the variance here. This is because the
                            // variance field from the ascription refers to the variance to use
                            // when applying the type to the value being matched, but this
                            // ascription applies rather to the type of the binding. e.g., in this
                            // example:
                            //
                            // ```
                            // let x: T = <expr>
                            // ```
                            //
                            // We are creating an ascription that defines the type of `x` to be
                            // exactly `T` (i.e., with invariance). The variance field, in
                            // contrast, is intended to be used to relate `T` to the type of
                            // `<expr>`.
                            ty::Variance::Invariant,
                        ),
                    },
                );

                self.schedule_drop_for_binding(var, irrefutable_pat.span, OutsideGuard);
                block.unit()
            }

            _ => {
                let place = unpack!(block = self.as_place(block, initializer));
                self.place_into_pattern(block, irrefutable_pat, place, true)
            }
        }
    }

    crate fn place_into_pattern(
        &mut self,
        block: BasicBlock,
        irrefutable_pat: Pat<'tcx>,
        initializer: Place<'tcx>,
        set_match_place: bool,
    ) -> BlockAnd<()> {
        let mut candidate = Candidate::new(initializer, &irrefutable_pat, false);

        let fake_borrow_temps =
            self.lower_match_tree(block, irrefutable_pat.span, false, &mut [&mut candidate]);

        // For matches and function arguments, the place that is being matched
        // can be set when creating the variables. But the place for
        // let PATTERN = ... might not even exist until we do the assignment.
        // so we set it here instead.
        if set_match_place {
            let mut candidate_ref = &candidate;
            while let Some(next) = {
                for binding in &candidate_ref.bindings {
                    let local = self.var_local_id(binding.var_id, OutsideGuard);

                    if let LocalInfo::User(ClearCrossCrate::Set(BindingForm::Var(
                        VarBindingForm { opt_match_place: Some((ref mut match_place, _)), .. },
                    ))) = self.local_decls[local].local_info
                    {
                        *match_place = Some(initializer);
                    } else {
                        bug!("Let binding to non-user variable.")
                    }
                }
                // All of the subcandidates should bind the same locals, so we
                // only visit the first one.
                candidate_ref.subcandidates.get(0)
            } {
                candidate_ref = next;
            }
        }

        self.bind_pattern(
            self.source_info(irrefutable_pat.span),
            candidate,
            None,
            &fake_borrow_temps,
            irrefutable_pat.span,
            None,
        )
        .unit()
    }

    /// Declares the bindings of the given patterns and returns the visibility
    /// scope for the bindings in these patterns, if such a scope had to be
    /// created. NOTE: Declaring the bindings should always be done in their
    /// drop scope.
    crate fn declare_bindings(
        &mut self,
        mut visibility_scope: Option<SourceScope>,
        scope_span: Span,
        pattern: &Pat<'tcx>,
        has_guard: ArmHasGuard,
        opt_match_place: Option<(Option<&Place<'tcx>>, Span)>,
    ) -> Option<SourceScope> {
        debug!("declare_bindings: pattern={:?}", pattern);
        self.visit_bindings(
            &pattern,
            UserTypeProjections::none(),
            &mut |this, mutability, name, mode, var, span, ty, user_ty| {
                if visibility_scope.is_none() {
                    visibility_scope =
                        Some(this.new_source_scope(scope_span, LintLevel::Inherited, None));
                }
                let source_info = SourceInfo { span, scope: this.source_scope };
                let visibility_scope = visibility_scope.unwrap();
                this.declare_binding(
                    source_info,
                    visibility_scope,
                    mutability,
                    name,
                    mode,
                    var,
                    ty,
                    user_ty,
                    has_guard,
                    opt_match_place.map(|(x, y)| (x.cloned(), y)),
                    pattern.span,
                );
            },
        );
        visibility_scope
    }

    crate fn storage_live_binding(
        &mut self,
        block: BasicBlock,
        var: HirId,
        span: Span,
        for_guard: ForGuard,
    ) -> Place<'tcx> {
        let local_id = self.var_local_id(var, for_guard);
        let source_info = self.source_info(span);
        self.cfg.push(block, Statement { source_info, kind: StatementKind::StorageLive(local_id) });
        let region_scope = self.hir.region_scope_tree.var_scope(var.local_id);
        self.schedule_drop(span, region_scope, local_id, DropKind::Storage);
        Place::from(local_id)
    }

    crate fn schedule_drop_for_binding(&mut self, var: HirId, span: Span, for_guard: ForGuard) {
        let local_id = self.var_local_id(var, for_guard);
        let region_scope = self.hir.region_scope_tree.var_scope(var.local_id);
        self.schedule_drop(span, region_scope, local_id, DropKind::Value);
    }

    pub(super) fn visit_bindings(
        &mut self,
        pattern: &Pat<'tcx>,
        pattern_user_ty: UserTypeProjections,
        f: &mut impl FnMut(
            &mut Self,
            Mutability,
            Name,
            BindingMode,
            HirId,
            Span,
            Ty<'tcx>,
            UserTypeProjections,
        ),
    ) {
        debug!("visit_bindings: pattern={:?} pattern_user_ty={:?}", pattern, pattern_user_ty);
        match *pattern.kind {
            PatKind::Binding { mutability, name, mode, var, ty, ref subpattern, .. } => {
                f(self, mutability, name, mode, var, pattern.span, ty, pattern_user_ty.clone());
                if let Some(subpattern) = subpattern.as_ref() {
                    self.visit_bindings(subpattern, pattern_user_ty, f);
                }
            }

            PatKind::Array { ref prefix, ref slice, ref suffix }
            | PatKind::Slice { ref prefix, ref slice, ref suffix } => {
                let from = u32::try_from(prefix.len()).unwrap();
                let to = u32::try_from(suffix.len()).unwrap();
                for subpattern in prefix {
                    self.visit_bindings(subpattern, pattern_user_ty.clone().index(), f);
                }
                for subpattern in slice {
                    self.visit_bindings(subpattern, pattern_user_ty.clone().subslice(from, to), f);
                }
                for subpattern in suffix {
                    self.visit_bindings(subpattern, pattern_user_ty.clone().index(), f);
                }
            }

            PatKind::Constant { .. } | PatKind::Range { .. } | PatKind::Wild => {}

            PatKind::Deref { ref subpattern } => {
                self.visit_bindings(subpattern, pattern_user_ty.deref(), f);
            }

            PatKind::AscribeUserType {
                ref subpattern,
                ascription: hair::pattern::Ascription { ref user_ty, user_ty_span, variance: _ },
            } => {
                // This corresponds to something like
                //
                // ```
                // let A::<'a>(_): A<'static> = ...;
                // ```
                //
                // Note that the variance doesn't apply here, as we are tracking the effect
                // of `user_ty` on any bindings contained with subpattern.
                let annotation = CanonicalUserTypeAnnotation {
                    span: user_ty_span,
                    user_ty: user_ty.user_ty,
                    inferred_ty: subpattern.ty,
                };
                let projection = UserTypeProjection {
                    base: self.canonical_user_type_annotations.push(annotation),
                    projs: Vec::new(),
                };
                let subpattern_user_ty = pattern_user_ty.push_projection(&projection, user_ty_span);
                self.visit_bindings(subpattern, subpattern_user_ty, f)
            }

            PatKind::Leaf { ref subpatterns } => {
                for subpattern in subpatterns {
                    let subpattern_user_ty = pattern_user_ty.clone().leaf(subpattern.field);
                    debug!("visit_bindings: subpattern_user_ty={:?}", subpattern_user_ty);
                    self.visit_bindings(&subpattern.pattern, subpattern_user_ty, f);
                }
            }

            PatKind::Variant { adt_def, substs: _, variant_index, ref subpatterns } => {
                for subpattern in subpatterns {
                    let subpattern_user_ty =
                        pattern_user_ty.clone().variant(adt_def, variant_index, subpattern.field);
                    self.visit_bindings(&subpattern.pattern, subpattern_user_ty, f);
                }
            }
            PatKind::Or { ref pats } => {
                self.visit_bindings(&pats[0], pattern_user_ty.clone(), f);
            }
        }
    }
}

#[derive(Debug)]
struct Candidate<'pat, 'tcx> {
    /// `Span` of the original pattern that gave rise to this candidate
    span: Span,

    /// This `Candidate` has a guard.
    has_guard: bool,

    /// All of these must be satisfied...
    match_pairs: SmallVec<[MatchPair<'pat, 'tcx>; 1]>,

    /// ...these bindings established...
    bindings: Vec<Binding<'tcx>>,

    /// ...and these types asserted...
    ascriptions: Vec<Ascription<'tcx>>,

    /// ... and if this is non-empty, one of these subcandidates also has to match ...
    subcandidates: Vec<Candidate<'pat, 'tcx>>,

    /// ...and the guard must be evaluated, if false branch to Block...
    otherwise_block: Option<BasicBlock>,

    /// ...and the blocks for add false edges between candidates
    pre_binding_block: Option<BasicBlock>,
    next_candidate_pre_binding_block: Option<BasicBlock>,
}

impl<'tcx, 'pat> Candidate<'pat, 'tcx> {
    fn new(place: Place<'tcx>, pattern: &'pat Pat<'tcx>, has_guard: bool) -> Self {
        Candidate {
            span: pattern.span,
            has_guard,
            match_pairs: smallvec![MatchPair { place, pattern }],
            bindings: Vec::new(),
            ascriptions: Vec::new(),
            subcandidates: Vec::new(),
            otherwise_block: None,
            pre_binding_block: None,
            next_candidate_pre_binding_block: None,
        }
    }

    /// Visit the leaf candidates (those with no subcandidates) contained in
    /// this candidate.
    fn visit_leaves<'a>(&'a mut self, mut visit_leaf: impl FnMut(&'a mut Self)) {
        traverse_candidate(
            self,
            &mut (),
            &mut move |c, _| visit_leaf(c),
            move |c, _| c.subcandidates.iter_mut(),
            |_| {},
        );
    }
}

/// A depth-first traversal of the `Candidate` and all of its recursive
/// subcandidates.
fn traverse_candidate<'pat, 'tcx: 'pat, C, T, I>(
    candidate: C,
    context: &mut T,
    visit_leaf: &mut impl FnMut(C, &mut T),
    get_children: impl Copy + Fn(C, &mut T) -> I,
    complete_children: impl Copy + Fn(&mut T),
) where
    C: Borrow<Candidate<'pat, 'tcx>>,
    I: Iterator<Item = C>,
{
    if candidate.borrow().subcandidates.is_empty() {
        visit_leaf(candidate, context)
    } else {
        for child in get_children(candidate, context) {
            traverse_candidate(child, context, visit_leaf, get_children, complete_children);
        }
        complete_children(context)
    }
}

#[derive(Clone, Debug)]
struct Binding<'tcx> {
    span: Span,
    source: Place<'tcx>,
    name: Name,
    var_id: HirId,
    var_ty: Ty<'tcx>,
    mutability: Mutability,
    binding_mode: BindingMode,
}

/// Indicates that the type of `source` must be a subtype of the
/// user-given type `user_ty`; this is basically a no-op but can
/// influence region inference.
#[derive(Clone, Debug)]
struct Ascription<'tcx> {
    span: Span,
    source: Place<'tcx>,
    user_ty: PatTyProj<'tcx>,
    variance: ty::Variance,
}

#[derive(Clone, Debug)]
crate struct MatchPair<'pat, 'tcx> {
    // this place...
    place: Place<'tcx>,

    // ... must match this pattern.
    pattern: &'pat Pat<'tcx>,
}

#[derive(Clone, Debug, PartialEq)]
enum TestKind<'tcx> {
    /// Test the branches of enum.
    Switch {
        /// The enum being tested
        adt_def: &'tcx ty::AdtDef,
        /// The set of variants that we should create a branch for. We also
        /// create an additional "otherwise" case.
        variants: BitSet<VariantIdx>,
    },

    /// Test what value an `integer`, `bool` or `char` has.
    SwitchInt {
        /// The type of the value that we're testing.
        switch_ty: Ty<'tcx>,
        /// The (ordered) set of values that we test for.
        ///
        /// For integers and `char`s we create a branch to each of the values in
        /// `options`, as well as an "otherwise" branch for all other values, even
        /// in the (rare) case that options is exhaustive.
        ///
        /// For `bool` we always generate two edges, one for `true` and one for
        /// `false`.
        options: Vec<u128>,
        /// Reverse map used to ensure that the values in `options` are unique.
        indices: FxHashMap<&'tcx ty::Const<'tcx>, usize>,
    },

    /// Test for equality with value, possibly after an unsizing coercion to
    /// `ty`,
    Eq {
        value: &'tcx ty::Const<'tcx>,
        // Integer types are handled by `SwitchInt`, and constants with ADT
        // types are converted back into patterns, so this can only be `&str`,
        // `&[T]`, `f32` or `f64`.
        ty: Ty<'tcx>,
    },

    /// Test whether the value falls within an inclusive or exclusive range
    Range(PatRange<'tcx>),

    /// Test length of the slice is equal to len
    Len { len: u64, op: BinOp },
}

#[derive(Debug)]
crate struct Test<'tcx> {
    span: Span,
    kind: TestKind<'tcx>,
}

/// ArmHasGuard is isomorphic to a boolean flag. It indicates whether
/// a match arm has a guard expression attached to it.
#[derive(Copy, Clone, Debug)]
crate struct ArmHasGuard(crate bool);

///////////////////////////////////////////////////////////////////////////
// Main matching algorithm

impl<'a, 'tcx> Builder<'a, 'tcx> {
    /// The main match algorithm. It begins with a set of candidates
    /// `candidates` and has the job of generating code to determine
    /// which of these candidates, if any, is the correct one. The
    /// candidates are sorted such that the first item in the list
    /// has the highest priority. When a candidate is found to match
    /// the value, we will set and generate a branch to the appropriate
    /// prebinding block.
    ///
    /// If we find that *NONE* of the candidates apply, we branch to the
    /// `otherwise_block`, setting it to `Some` if required. In principle, this
    /// means that the input list was not exhaustive, though at present we
    /// sometimes are not smart enough to recognize all exhaustive inputs.
    ///
    /// It might be surprising that the input can be inexhaustive.
    /// Indeed, initially, it is not, because all matches are
    /// exhaustive in Rust. But during processing we sometimes divide
    /// up the list of candidates and recurse with a non-exhaustive
    /// list. This is important to keep the size of the generated code
    /// under control. See `test_candidates` for more details.
    ///
    /// If `fake_borrows` is Some, then places which need fake borrows
    /// will be added to it.
    ///
    /// For an example of a case where we set `otherwise_block`, even for an
    /// exhaustive match consider:
    ///
    /// match x {
    ///     (true, true) => (),
    ///     (_, false) => (),
    ///     (false, true) => (),
    /// }
    ///
    /// For this match, we check if `x.0` matches `true` (for the first
    /// arm). If that's false, we check `x.1`. If it's `true` we check if
    /// `x.0` matches `false` (for the third arm). In the (impossible at
    /// runtime) case when `x.0` is now `true`, we branch to
    /// `otherwise_block`.
    fn match_candidates<'pat>(
        &mut self,
        span: Span,
        start_block: BasicBlock,
        otherwise_block: &mut Option<BasicBlock>,
        candidates: &mut [&mut Candidate<'pat, 'tcx>],
        fake_borrows: &mut Option<FxHashSet<Place<'tcx>>>,
    ) {
        debug!(
            "matched_candidate(span={:?}, candidates={:?}, start_block={:?}, otherwise_block={:?})",
            span, candidates, start_block, otherwise_block,
        );

        // Start by simplifying candidates. Once this process is complete, all
        // the match pairs which remain require some form of test, whether it
        // be a switch or pattern comparison.
        let mut split_or_candidate = false;
        for candidate in &mut *candidates {
            split_or_candidate |= self.simplify_candidate(candidate);
        }

        if split_or_candidate {
            // At least one of the candidates has been split into subcandidates.
            // We need to change the candidate list to include those.
            let mut new_candidates = Vec::new();

            for candidate in candidates {
                candidate.visit_leaves(|leaf_candidate| new_candidates.push(leaf_candidate));
            }
            self.match_simplified_candidates(
                span,
                start_block,
                otherwise_block,
                &mut *new_candidates,
                fake_borrows,
            );
        } else {
            self.match_simplified_candidates(
                span,
                start_block,
                otherwise_block,
                candidates,
                fake_borrows,
            );
        };
    }

    fn match_simplified_candidates(
        &mut self,
        span: Span,
        start_block: BasicBlock,
        otherwise_block: &mut Option<BasicBlock>,
        candidates: &mut [&mut Candidate<'_, 'tcx>],
        fake_borrows: &mut Option<FxHashSet<Place<'tcx>>>,
    ) {
        // The candidates are sorted by priority. Check to see whether the
        // higher priority candidates (and hence at the front of the slice)
        // have satisfied all their match pairs.
        let fully_matched = candidates.iter().take_while(|c| c.match_pairs.is_empty()).count();
        debug!("match_candidates: {:?} candidates fully matched", fully_matched);
        let (matched_candidates, unmatched_candidates) = candidates.split_at_mut(fully_matched);

        let block = if !matched_candidates.is_empty() {
            let otherwise_block =
                self.select_matched_candidates(matched_candidates, start_block, fake_borrows);

            if let Some(last_otherwise_block) = otherwise_block {
                last_otherwise_block
            } else {
                // Any remaining candidates are unreachable.
                if unmatched_candidates.is_empty() {
                    return;
                }
                self.cfg.start_new_block()
            }
        } else {
            start_block
        };

        // If there are no candidates that still need testing, we're
        // done. Since all matches are exhaustive, execution should
        // never reach this point.
        if unmatched_candidates.is_empty() {
            let source_info = self.source_info(span);
            if let Some(otherwise) = *otherwise_block {
                self.cfg.goto(block, source_info, otherwise);
            } else {
                *otherwise_block = Some(block);
            }
            return;
        }

        // Test for the remaining candidates.
        self.test_candidates_with_or(
            span,
            unmatched_candidates,
            block,
            otherwise_block,
            fake_borrows,
        );
    }

    /// Link up matched candidates. For example, if we have something like
    /// this:
    ///
    /// ...
    /// Some(x) if cond => ...
    /// Some(x) => ...
    /// Some(x) if cond => ...
    /// ...
    ///
    /// We generate real edges from:
    /// * `start_block` to the `prebinding_block` of the first pattern,
    /// * the otherwise block of the first pattern to the second pattern,
    /// * the otherwise block of the third pattern to the a block with an
    ///   Unreachable terminator.
    ///
    /// As well as that we add fake edges from the otherwise blocks to the
    /// prebinding block of the next candidate in the original set of
    /// candidates.
    fn select_matched_candidates(
        &mut self,
        matched_candidates: &mut [&mut Candidate<'_, 'tcx>],
        start_block: BasicBlock,
        fake_borrows: &mut Option<FxHashSet<Place<'tcx>>>,
    ) -> Option<BasicBlock> {
        debug_assert!(
            !matched_candidates.is_empty(),
            "select_matched_candidates called with no candidates",
        );
        debug_assert!(
            matched_candidates.iter().all(|c| c.subcandidates.is_empty()),
            "subcandidates should be empty in select_matched_candidates",
        );

        // Insert a borrows of prefixes of places that are bound and are
        // behind a dereference projection.
        //
        // These borrows are taken to avoid situations like the following:
        //
        // match x[10] {
        //     _ if { x = &[0]; false } => (),
        //     y => (), // Out of bounds array access!
        // }
        //
        // match *x {
        //     // y is bound by reference in the guard and then by copy in the
        //     // arm, so y is 2 in the arm!
        //     y if { y == 1 && (x = &2) == () } => y,
        //     _ => 3,
        // }
        if let Some(fake_borrows) = fake_borrows {
            for Binding { source, .. } in
                matched_candidates.iter().flat_map(|candidate| &candidate.bindings)
            {
                if let Some(i) =
                    source.projection.iter().rposition(|elem| *elem == ProjectionElem::Deref)
                {
                    let proj_base = &source.projection[..i];

                    fake_borrows.insert(Place {
                        local: source.local,
                        projection: self.hir.tcx().intern_place_elems(proj_base),
                    });
                }
            }
        }

        let fully_matched_with_guard = matched_candidates
            .iter()
            .position(|c| !c.has_guard)
            .unwrap_or(matched_candidates.len() - 1);

        let (reachable_candidates, unreachable_candidates) =
            matched_candidates.split_at_mut(fully_matched_with_guard + 1);

        let mut next_prebinding = start_block;

        for candidate in reachable_candidates.iter_mut() {
            assert!(candidate.otherwise_block.is_none());
            assert!(candidate.pre_binding_block.is_none());
            candidate.pre_binding_block = Some(next_prebinding);
            if candidate.has_guard {
                // Create the otherwise block for this candidate, which is the
                // pre-binding block for the next candidate.
                next_prebinding = self.cfg.start_new_block();
                candidate.otherwise_block = Some(next_prebinding);
            }
        }

        debug!(
            "match_candidates: add pre_binding_blocks for unreachable {:?}",
            unreachable_candidates,
        );
        for candidate in unreachable_candidates {
            assert!(candidate.pre_binding_block.is_none());
            candidate.pre_binding_block = Some(self.cfg.start_new_block());
        }

        reachable_candidates.last_mut().unwrap().otherwise_block
    }

    /// Tests a candidate where there are only or-patterns left to test, or
    /// forwards to [Builder::test_candidates].
    ///
    /// Given a pattern `(P | Q, R | S)` we (in principle) generate a CFG like
    /// so
    ///
    /// ```text
    /// [ start ]
    ///      |
    /// [ match P, Q ]
    ///      |
    ///      +----------------------------------------+------------------------------------+
    ///      |                                        |                                    |
    /// [ P matches ]                           [ Q matches ]                        [ otherwise ]
    ///      |                                        |                                    |
    /// [ match R, S ]                          [ match R, S ]                             |
    ///      |                                        |                                    |
    ///      +--------------+------------+            +--------------+------------+        |
    ///      |              |            |            |              |            |        |
    /// [ R matches ] [ S matches ] [otherwise ] [ R matches ] [ S matches ] [otherwise ]  |
    ///      |              |            |            |              |            |        |
    ///      +--------------+------------|------------+--------------+            |        |
    ///      |                           |                                        |        |
    ///      |                           +----------------------------------------+--------+
    ///      |                           |
    /// [ Success ]                 [ Failure ]
    /// ```
    ///
    /// In practice there are some complications:
    ///
    /// * If there's a guard, then the otherwise branch of the first match on
    ///   `R | S` goes to a test for whether `Q` matches.
    /// * If neither `P` or `Q` has any bindings or type ascriptions and there
    ///   isn't a match guard, then we create a smaller CFG like:
    ///
    /// ```text
    ///     ...
    ///      +---------------+------------+
    ///      |               |            |
    /// [ P matches ] [ Q matches ] [ otherwise ]
    ///      |               |            |
    ///      +---------------+            |
    ///      |                           ...
    /// [ match R, S ]
    ///      |
    ///     ...
    /// ```
    fn test_candidates_with_or(
        &mut self,
        span: Span,
        candidates: &mut [&mut Candidate<'_, 'tcx>],
        block: BasicBlock,
        otherwise_block: &mut Option<BasicBlock>,
        fake_borrows: &mut Option<FxHashSet<Place<'tcx>>>,
    ) {
        let (first_candidate, remaining_candidates) = candidates.split_first_mut().unwrap();

        // All of the or-patterns have been sorted to the end, so if the first
        // pattern is an or-pattern we only have or-patterns.
        match *first_candidate.match_pairs[0].pattern.kind {
            PatKind::Or { .. } => (),
            _ => {
                self.test_candidates(span, candidates, block, otherwise_block, fake_borrows);
                return;
            }
        }

        let match_pairs = mem::take(&mut first_candidate.match_pairs);
        first_candidate.pre_binding_block = Some(block);

        let mut otherwise = None;
        for match_pair in match_pairs {
            if let PatKind::Or { ref pats } = *match_pair.pattern.kind {
                let or_span = match_pair.pattern.span;
                let place = match_pair.place;

                first_candidate.visit_leaves(|leaf_candidate| {
                    self.test_or_pattern(
                        leaf_candidate,
                        &mut otherwise,
                        pats,
                        or_span,
                        place,
                        fake_borrows,
                    );
                });
            } else {
                bug!("Or-patterns should have been sorted to the end");
            }
        }

        let remainder_start = otherwise.unwrap_or_else(|| self.cfg.start_new_block());

        self.match_candidates(
            span,
            remainder_start,
            otherwise_block,
            remaining_candidates,
            fake_borrows,
        )
    }

    fn test_or_pattern<'pat>(
        &mut self,
        candidate: &mut Candidate<'pat, 'tcx>,
        otherwise: &mut Option<BasicBlock>,
        pats: &'pat [Pat<'tcx>],
        or_span: Span,
        place: Place<'tcx>,
        fake_borrows: &mut Option<FxHashSet<Place<'tcx>>>,
    ) {
        debug!("test_or_pattern:\ncandidate={:#?}\npats={:#?}", candidate, pats);
        let mut or_candidates: Vec<_> =
            pats.iter().map(|pat| Candidate::new(place, pat, candidate.has_guard)).collect();
        let mut or_candidate_refs: Vec<_> = or_candidates.iter_mut().collect();
        let otherwise = if candidate.otherwise_block.is_some() {
            &mut candidate.otherwise_block
        } else {
            otherwise
        };
        self.match_candidates(
            or_span,
            candidate.pre_binding_block.unwrap(),
            otherwise,
            &mut or_candidate_refs,
            fake_borrows,
        );
        candidate.subcandidates = or_candidates;
        self.merge_trivial_subcandidates(candidate, self.source_info(or_span));
    }

    /// Try to merge all of the subcandidates of the given candidate into one.
    /// This avoids exponentially large CFGs in cases like `(1 | 2, 3 | 4, ...)`.
    fn merge_trivial_subcandidates(
        &mut self,
        candidate: &mut Candidate<'_, 'tcx>,
        source_info: SourceInfo,
    ) {
        if candidate.subcandidates.is_empty() || candidate.has_guard {
            // FIXME(or_patterns; matthewjasper) Don't give up if we have a guard.
            return;
        }

        let mut can_merge = true;

        // Not `Iterator::all` because we don't want to short-circuit.
        for subcandidate in &mut candidate.subcandidates {
            self.merge_trivial_subcandidates(subcandidate, source_info);

            // FIXME(or_patterns; matthewjasper) Try to be more aggressive here.
            can_merge &= subcandidate.subcandidates.is_empty()
                && subcandidate.bindings.is_empty()
                && subcandidate.ascriptions.is_empty();
        }

        if can_merge {
            let any_matches = self.cfg.start_new_block();
            for subcandidate in mem::take(&mut candidate.subcandidates) {
                let or_block = subcandidate.pre_binding_block.unwrap();
                self.cfg.goto(or_block, source_info, any_matches);
            }
            candidate.pre_binding_block = Some(any_matches);
        }
    }

    /// This is the most subtle part of the matching algorithm. At
    /// this point, the input candidates have been fully simplified,
    /// and so we know that all remaining match-pairs require some
    /// sort of test. To decide what test to do, we take the highest
    /// priority candidate (last one in the list) and extract the
    /// first match-pair from the list. From this we decide what kind
    /// of test is needed using `test`, defined in the `test` module.
    ///
    /// *Note:* taking the first match pair is somewhat arbitrary, and
    /// we might do better here by choosing more carefully what to
    /// test.
    ///
    /// For example, consider the following possible match-pairs:
    ///
    /// 1. `x @ Some(P)` -- we will do a `Switch` to decide what variant `x` has
    /// 2. `x @ 22` -- we will do a `SwitchInt`
    /// 3. `x @ 3..5` -- we will do a range test
    /// 4. etc.
    ///
    /// Once we know what sort of test we are going to perform, this
    /// Tests may also help us with other candidates. So we walk over
    /// the candidates (from high to low priority) and check. This
    /// gives us, for each outcome of the test, a transformed list of
    /// candidates. For example, if we are testing the current
    /// variant of `x.0`, and we have a candidate `{x.0 @ Some(v), x.1
    /// @ 22}`, then we would have a resulting candidate of `{(x.0 as
    /// Some).0 @ v, x.1 @ 22}`. Note that the first match-pair is now
    /// simpler (and, in fact, irrefutable).
    ///
    /// But there may also be candidates that the test just doesn't
    /// apply to. The classical example involves wildcards:
    ///
    /// ```
    /// # let (x, y, z) = (true, true, true);
    /// match (x, y, z) {
    ///     (true, _, true) => true,    // (0)
    ///     (_, true, _) => true,       // (1)
    ///     (false, false, _) => false, // (2)
    ///     (true, _, false) => false,  // (3)
    /// }
    /// ```
    ///
    /// In that case, after we test on `x`, there are 2 overlapping candidate
    /// sets:
    ///
    /// - If the outcome is that `x` is true, candidates 0, 1, and 3
    /// - If the outcome is that `x` is false, candidates 1 and 2
    ///
    /// Here, the traditional "decision tree" method would generate 2
    /// separate code-paths for the 2 separate cases.
    ///
    /// In some cases, this duplication can create an exponential amount of
    /// code. This is most easily seen by noticing that this method terminates
    /// with precisely the reachable arms being reachable - but that problem
    /// is trivially NP-complete:
    ///
    /// ```rust
    ///     match (var0, var1, var2, var3, ..) {
    ///         (true, _, _, false, true, ...) => false,
    ///         (_, true, true, false, _, ...) => false,
    ///         (false, _, false, false, _, ...) => false,
    ///         ...
    ///         _ => true
    ///     }
    /// ```
    ///
    /// Here the last arm is reachable only if there is an assignment to
    /// the variables that does not match any of the literals. Therefore,
    /// compilation would take an exponential amount of time in some cases.
    ///
    /// That kind of exponential worst-case might not occur in practice, but
    /// our simplistic treatment of constants and guards would make it occur
    /// in very common situations - for example #29740:
    ///
    /// ```rust
    /// match x {
    ///     "foo" if foo_guard => ...,
    ///     "bar" if bar_guard => ...,
    ///     "baz" if baz_guard => ...,
    ///     ...
    /// }
    /// ```
    ///
    /// Here we first test the match-pair `x @ "foo"`, which is an `Eq` test.
    ///
    /// It might seem that we would end up with 2 disjoint candidate
    /// sets, consisting of the first candidate or the other 3, but our
    /// algorithm doesn't reason about "foo" being distinct from the other
    /// constants; it considers the latter arms to potentially match after
    /// both outcomes, which obviously leads to an exponential amount
    /// of tests.
    ///
    /// To avoid these kinds of problems, our algorithm tries to ensure
    /// the amount of generated tests is linear. When we do a k-way test,
    /// we return an additional "unmatched" set alongside the obvious `k`
    /// sets. When we encounter a candidate that would be present in more
    /// than one of the sets, we put it and all candidates below it into the
    /// "unmatched" set. This ensures these `k+1` sets are disjoint.
    ///
    /// After we perform our test, we branch into the appropriate candidate
    /// set and recurse with `match_candidates`. These sub-matches are
    /// obviously inexhaustive - as we discarded our otherwise set - so
    /// we set their continuation to do `match_candidates` on the
    /// "unmatched" set (which is again inexhaustive).
    ///
    /// If you apply this to the above test, you basically wind up
    /// with an if-else-if chain, testing each candidate in turn,
    /// which is precisely what we want.
    ///
    /// In addition to avoiding exponential-time blowups, this algorithm
    /// also has nice property that each guard and arm is only generated
    /// once.
    fn test_candidates<'pat, 'b, 'c>(
        &mut self,
        span: Span,
        mut candidates: &'b mut [&'c mut Candidate<'pat, 'tcx>],
        block: BasicBlock,
        otherwise_block: &mut Option<BasicBlock>,
        fake_borrows: &mut Option<FxHashSet<Place<'tcx>>>,
    ) {
        // extract the match-pair from the highest priority candidate
        let match_pair = &candidates.first().unwrap().match_pairs[0];
        let mut test = self.test(match_pair);
        let match_place = match_pair.place;

        // most of the time, the test to perform is simply a function
        // of the main candidate; but for a test like SwitchInt, we
        // may want to add cases based on the candidates that are
        // available
        match test.kind {
            TestKind::SwitchInt { switch_ty, ref mut options, ref mut indices } => {
                for candidate in candidates.iter() {
                    if !self.add_cases_to_switch(
                        &match_place,
                        candidate,
                        switch_ty,
                        options,
                        indices,
                    ) {
                        break;
                    }
                }
            }
            TestKind::Switch { adt_def: _, ref mut variants } => {
                for candidate in candidates.iter() {
                    if !self.add_variants_to_switch(&match_place, candidate, variants) {
                        break;
                    }
                }
            }
            _ => {}
        }

        // Insert a Shallow borrow of any places that is switched on.
        fake_borrows.as_mut().map(|fb| fb.insert(match_place));

        // perform the test, branching to one of N blocks. For each of
        // those N possible outcomes, create a (initially empty)
        // vector of candidates. Those are the candidates that still
        // apply if the test has that particular outcome.
        debug!("match_candidates: test={:?} match_pair={:?}", test, match_pair);
        let mut target_candidates: Vec<Vec<&mut Candidate<'pat, 'tcx>>> = vec![];
        target_candidates.resize_with(test.targets(), Default::default);

        let total_candidate_count = candidates.len();

        // Sort the candidates into the appropriate vector in
        // `target_candidates`. Note that at some point we may
        // encounter a candidate where the test is not relevant; at
        // that point, we stop sorting.
        while let Some(candidate) = candidates.first_mut() {
            if let Some(idx) = self.sort_candidate(&match_place, &test, candidate) {
                let (candidate, rest) = candidates.split_first_mut().unwrap();
                target_candidates[idx].push(candidate);
                candidates = rest;
            } else {
                break;
            }
        }
        // at least the first candidate ought to be tested
        assert!(total_candidate_count > candidates.len());
        debug!("tested_candidates: {}", total_candidate_count - candidates.len());
        debug!("untested_candidates: {}", candidates.len());

        // HACK(matthewjasper) This is a closure so that we can let the test
        // create its blocks before the rest of the match. This currently
        // improves the speed of llvm when optimizing long string literal
        // matches
        let make_target_blocks = move |this: &mut Self| -> Vec<BasicBlock> {
            // The block that we should branch to if none of the
            // `target_candidates` match. This is either the block where we
            // start matching the untested candidates if there are any,
            // otherwise it's the `otherwise_block`.
            let remainder_start = &mut None;
            let remainder_start =
                if candidates.is_empty() { &mut *otherwise_block } else { remainder_start };

            // For each outcome of test, process the candidates that still
            // apply. Collect a list of blocks where control flow will
            // branch if one of the `target_candidate` sets is not
            // exhaustive.
            let target_blocks: Vec<_> = target_candidates
                .into_iter()
                .map(|mut candidates| {
                    if candidates.len() != 0 {
                        let candidate_start = this.cfg.start_new_block();
                        this.match_candidates(
                            span,
                            candidate_start,
                            remainder_start,
                            &mut *candidates,
                            fake_borrows,
                        );
                        candidate_start
                    } else {
                        *remainder_start.get_or_insert_with(|| this.cfg.start_new_block())
                    }
                })
                .collect();

            if !candidates.is_empty() {
                let remainder_start = remainder_start.unwrap_or_else(|| this.cfg.start_new_block());
                this.match_candidates(
                    span,
                    remainder_start,
                    otherwise_block,
                    candidates,
                    fake_borrows,
                );
            };

            target_blocks
        };

        self.perform_test(block, match_place, &test, make_target_blocks);
    }

    /// Determine the fake borrows that are needed from a set of places that
    /// have to be stable across match guards.
    ///
    /// Returns a list of places that need a fake borrow and the temporary
    /// that's used to store the fake borrow.
    ///
    /// Match exhaustiveness checking is not able to handle the case where the
    /// place being matched on is mutated in the guards. We add "fake borrows"
    /// to the guards that prevent any mutation of the place being matched.
    /// There are a some subtleties:
    ///
    /// 1. Borrowing `*x` doesn't prevent assigning to `x`. If `x` is a shared
    ///    reference, the borrow isn't even tracked. As such we have to add fake
    ///    borrows of any prefixes of a place
    /// 2. We don't want `match x { _ => (), }` to conflict with mutable
    ///    borrows of `x`, so we only add fake borrows for places which are
    ///    bound or tested by the match.
    /// 3. We don't want the fake borrows to conflict with `ref mut` bindings,
    ///    so we use a special BorrowKind for them.
    /// 4. The fake borrows may be of places in inactive variants, so it would
    ///    be UB to generate code for them. They therefore have to be removed
    ///    by a MIR pass run after borrow checking.
    fn calculate_fake_borrows<'b>(
        &mut self,
        fake_borrows: &'b FxHashSet<Place<'tcx>>,
        temp_span: Span,
    ) -> Vec<(Place<'tcx>, Local)> {
        let tcx = self.hir.tcx();

        debug!("add_fake_borrows fake_borrows = {:?}", fake_borrows);

        let mut all_fake_borrows = Vec::with_capacity(fake_borrows.len());

        // Insert a Shallow borrow of the prefixes of any fake borrows.
        for place in fake_borrows {
            let mut cursor = place.projection.as_ref();
            while let [proj_base @ .., elem] = cursor {
                cursor = proj_base;

                if let ProjectionElem::Deref = elem {
                    // Insert a shallow borrow after a deref. For other
                    // projections the borrow of prefix_cursor will
                    // conflict with any mutation of base.
                    all_fake_borrows.push(PlaceRef { local: place.local, projection: proj_base });
                }
            }

            all_fake_borrows.push(place.as_ref());
        }

        // Deduplicate and ensure a deterministic order.
        all_fake_borrows.sort();
        all_fake_borrows.dedup();

        debug!("add_fake_borrows all_fake_borrows = {:?}", all_fake_borrows);

        all_fake_borrows
            .into_iter()
            .map(|matched_place_ref| {
                let matched_place = Place {
                    local: matched_place_ref.local,
                    projection: tcx.intern_place_elems(matched_place_ref.projection),
                };
                let fake_borrow_deref_ty = matched_place.ty(&self.local_decls, tcx).ty;
                let fake_borrow_ty = tcx.mk_imm_ref(tcx.lifetimes.re_erased, fake_borrow_deref_ty);
                let fake_borrow_temp =
                    self.local_decls.push(LocalDecl::new_temp(fake_borrow_ty, temp_span));

                (matched_place, fake_borrow_temp)
            })
            .collect()
    }
}

///////////////////////////////////////////////////////////////////////////
// Pat binding - used for `let` and function parameters as well.

impl<'a, 'tcx> Builder<'a, 'tcx> {
    /// Initializes each of the bindings from the candidate by
    /// moving/copying/ref'ing the source as appropriate. Tests the guard, if
    /// any, and then branches to the arm. Returns the block for the case where
    /// the guard fails.
    ///
    /// Note: we do not check earlier that if there is a guard,
    /// there cannot be move bindings. We avoid a use-after-move by only
    /// moving the binding once the guard has evaluated to true (see below).
    fn bind_and_guard_matched_candidate<'pat>(
        &mut self,
        candidate: Candidate<'pat, 'tcx>,
        parent_bindings: &[(Vec<Binding<'tcx>>, Vec<Ascription<'tcx>>)],
        guard: Option<(&Guard<'tcx>, region::Scope)>,
        fake_borrows: &Vec<(Place<'tcx>, Local)>,
        scrutinee_span: Span,
        schedule_drops: bool,
    ) -> BasicBlock {
        debug!("bind_and_guard_matched_candidate(candidate={:?})", candidate);

        debug_assert!(candidate.match_pairs.is_empty());

        let candidate_source_info = self.source_info(candidate.span);

        let mut block = candidate.pre_binding_block.unwrap();

        if candidate.next_candidate_pre_binding_block.is_some() {
            let fresh_block = self.cfg.start_new_block();
            self.false_edges(
                block,
                fresh_block,
                candidate.next_candidate_pre_binding_block,
                candidate_source_info,
            );
            block = fresh_block;
        }

        self.ascribe_types(
            block,
            parent_bindings
                .iter()
                .flat_map(|(_, ascriptions)| ascriptions)
                .chain(&candidate.ascriptions),
        );

        // rust-lang/rust#27282: The `autoref` business deserves some
        // explanation here.
        //
        // The intent of the `autoref` flag is that when it is true,
        // then any pattern bindings of type T will map to a `&T`
        // within the context of the guard expression, but will
        // continue to map to a `T` in the context of the arm body. To
        // avoid surfacing this distinction in the user source code
        // (which would be a severe change to the language and require
        // far more revision to the compiler), when `autoref` is true,
        // then any occurrence of the identifier in the guard
        // expression will automatically get a deref op applied to it.
        //
        // So an input like:
        //
        // ```
        // let place = Foo::new();
        // match place { foo if inspect(foo)
        //     => feed(foo), ...  }
        // ```
        //
        // will be treated as if it were really something like:
        //
        // ```
        // let place = Foo::new();
        // match place { Foo { .. } if { let tmp1 = &place; inspect(*tmp1) }
        //     => { let tmp2 = place; feed(tmp2) }, ... }
        //
        // And an input like:
        //
        // ```
        // let place = Foo::new();
        // match place { ref mut foo if inspect(foo)
        //     => feed(foo), ...  }
        // ```
        //
        // will be treated as if it were really something like:
        //
        // ```
        // let place = Foo::new();
        // match place { Foo { .. } if { let tmp1 = & &mut place; inspect(*tmp1) }
        //     => { let tmp2 = &mut place; feed(tmp2) }, ... }
        // ```
        //
        // In short, any pattern binding will always look like *some*
        // kind of `&T` within the guard at least in terms of how the
        // MIR-borrowck views it, and this will ensure that guard
        // expressions cannot mutate their the match inputs via such
        // bindings. (It also ensures that guard expressions can at
        // most *copy* values from such bindings; non-Copy things
        // cannot be moved via pattern bindings in guard expressions.)
        //
        // ----
        //
        // Implementation notes (under assumption `autoref` is true).
        //
        // To encode the distinction above, we must inject the
        // temporaries `tmp1` and `tmp2`.
        //
        // There are two cases of interest: binding by-value, and binding by-ref.
        //
        // 1. Binding by-value: Things are simple.
        //
        //    * Establishing `tmp1` creates a reference into the
        //      matched place. This code is emitted by
        //      bind_matched_candidate_for_guard.
        //
        //    * `tmp2` is only initialized "lazily", after we have
        //      checked the guard. Thus, the code that can trigger
        //      moves out of the candidate can only fire after the
        //      guard evaluated to true. This initialization code is
        //      emitted by bind_matched_candidate_for_arm.
        //
        // 2. Binding by-reference: Things are tricky.
        //
        //    * Here, the guard expression wants a `&&` or `&&mut`
        //      into the original input. This means we need to borrow
        //      the reference that we create for the arm.
        //    * So we eagerly create the reference for the arm and then take a
        //      reference to that.
        if let Some((guard, region_scope)) = guard {
            let tcx = self.hir.tcx();
            let bindings = parent_bindings
                .iter()
                .flat_map(|(bindings, _)| bindings)
                .chain(&candidate.bindings);

            self.bind_matched_candidate_for_guard(block, bindings.clone());
            let guard_frame = GuardFrame {
                locals: bindings.map(|b| GuardFrameLocal::new(b.var_id, b.binding_mode)).collect(),
            };
            debug!("entering guard building context: {:?}", guard_frame);
            self.guard_context.push(guard_frame);

            let re_erased = tcx.lifetimes.re_erased;
            let scrutinee_source_info = self.source_info(scrutinee_span);
            for &(place, temp) in fake_borrows {
                let borrow = Rvalue::Ref(re_erased, BorrowKind::Shallow, place);
                self.cfg.push_assign(block, scrutinee_source_info, &Place::from(temp), borrow);
            }

            // the block to branch to if the guard fails; if there is no
            // guard, this block is simply unreachable
            let guard = match guard {
                Guard::If(e) => self.hir.mirror(e.clone()),
            };
            let source_info = self.source_info(guard.span);
            let guard_end = self.source_info(tcx.sess.source_map().end_point(guard.span));
            let (post_guard_block, otherwise_post_guard_block) =
                self.test_bool(block, guard, source_info);
            let guard_frame = self.guard_context.pop().unwrap();
            debug!("Exiting guard building context with locals: {:?}", guard_frame);

            for &(_, temp) in fake_borrows {
                let cause = FakeReadCause::ForMatchGuard;
                self.cfg.push_fake_read(post_guard_block, guard_end, cause, Place::from(temp));
            }

            let otherwise_block = candidate.otherwise_block.unwrap_or_else(|| {
                let unreachable = self.cfg.start_new_block();
                self.cfg.terminate(unreachable, source_info, TerminatorKind::Unreachable);
                unreachable
            });
            let outside_scope = self.cfg.start_new_block();
            self.exit_scope(
                source_info.span,
                region_scope,
                otherwise_post_guard_block,
                outside_scope,
            );
            self.false_edges(
                outside_scope,
                otherwise_block,
                candidate.next_candidate_pre_binding_block,
                source_info,
            );

            // We want to ensure that the matched candidates are bound
            // after we have confirmed this candidate *and* any
            // associated guard; Binding them on `block` is too soon,
            // because that would be before we've checked the result
            // from the guard.
            //
            // But binding them on the arm is *too late*, because
            // then all of the candidates for a single arm would be
            // bound in the same place, that would cause a case like:
            //
            // ```rust
            // match (30, 2) {
            //     (mut x, 1) | (2, mut x) if { true } => { ... }
            //     ...                                 // ^^^^^^^ (this is `arm_block`)
            // }
            // ```
            //
            // would yield a `arm_block` something like:
            //
            // ```
            // StorageLive(_4);        // _4 is `x`
            // _4 = &mut (_1.0: i32);  // this is handling `(mut x, 1)` case
            // _4 = &mut (_1.1: i32);  // this is handling `(2, mut x)` case
            // ```
            //
            // and that is clearly not correct.
            let by_value_bindings =
                parent_bindings
                    .iter()
                    .flat_map(|(bindings, _)| bindings)
                    .chain(&candidate.bindings)
                    .filter(|binding| {
                        if let BindingMode::ByValue = binding.binding_mode { true } else { false }
                    });
            // Read all of the by reference bindings to ensure that the
            // place they refer to can't be modified by the guard.
            for binding in by_value_bindings.clone() {
                let local_id = self.var_local_id(binding.var_id, RefWithinGuard);
                let cause = FakeReadCause::ForGuardBinding;
                self.cfg.push_fake_read(post_guard_block, guard_end, cause, Place::from(local_id));
            }
            assert!(schedule_drops, "patterns with guards must schedule drops");
            self.bind_matched_candidate_for_arm_body(post_guard_block, true, by_value_bindings);

            post_guard_block
        } else {
            // (Here, it is not too early to bind the matched
            // candidate on `block`, because there is no guard result
            // that we have to inspect before we bind them.)
            self.bind_matched_candidate_for_arm_body(
                block,
                schedule_drops,
                parent_bindings
                    .iter()
                    .flat_map(|(bindings, _)| bindings)
                    .chain(&candidate.bindings),
            );
            block
        }
    }

    /// Append `AscribeUserType` statements onto the end of `block`
    /// for each ascription
    fn ascribe_types<'b>(
        &mut self,
        block: BasicBlock,
        ascriptions: impl IntoIterator<Item = &'b Ascription<'tcx>>,
    ) where
        'tcx: 'b,
    {
        for ascription in ascriptions {
            let source_info = self.source_info(ascription.span);

            debug!(
                "adding user ascription at span {:?} of place {:?} and {:?}",
                source_info.span, ascription.source, ascription.user_ty,
            );

            let user_ty = ascription.user_ty.clone().user_ty(
                &mut self.canonical_user_type_annotations,
                ascription.source.ty(&self.local_decls, self.hir.tcx()).ty,
                source_info.span,
            );
            self.cfg.push(
                block,
                Statement {
                    source_info,
                    kind: StatementKind::AscribeUserType(
                        box (ascription.source, user_ty),
                        ascription.variance,
                    ),
                },
            );
        }
    }

    fn bind_matched_candidate_for_guard<'b>(
        &mut self,
        block: BasicBlock,
        bindings: impl IntoIterator<Item = &'b Binding<'tcx>>,
    ) where
        'tcx: 'b,
    {
        debug!("bind_matched_candidate_for_guard(block={:?})", block);

        // Assign each of the bindings. Since we are binding for a
        // guard expression, this will never trigger moves out of the
        // candidate.
        let re_erased = self.hir.tcx().lifetimes.re_erased;
        for binding in bindings {
            debug!("bind_matched_candidate_for_guard(binding={:?})", binding);
            let source_info = self.source_info(binding.span);

            // For each pattern ident P of type T, `ref_for_guard` is
            // a reference R: &T pointing to the location matched by
            // the pattern, and every occurrence of P within a guard
            // denotes *R.
            let ref_for_guard =
                self.storage_live_binding(block, binding.var_id, binding.span, RefWithinGuard);
            match binding.binding_mode {
                BindingMode::ByValue => {
                    let rvalue = Rvalue::Ref(re_erased, BorrowKind::Shared, binding.source);
                    self.cfg.push_assign(block, source_info, &ref_for_guard, rvalue);
                }
                BindingMode::ByRef(borrow_kind) => {
                    let value_for_arm = self.storage_live_binding(
                        block,
                        binding.var_id,
                        binding.span,
                        OutsideGuard,
                    );

                    let rvalue = Rvalue::Ref(re_erased, borrow_kind, binding.source);
                    self.cfg.push_assign(block, source_info, &value_for_arm, rvalue);
                    let rvalue = Rvalue::Ref(re_erased, BorrowKind::Shared, value_for_arm);
                    self.cfg.push_assign(block, source_info, &ref_for_guard, rvalue);
                }
            }
        }
    }

    fn bind_matched_candidate_for_arm_body<'b>(
        &mut self,
        block: BasicBlock,
        schedule_drops: bool,
        bindings: impl IntoIterator<Item = &'b Binding<'tcx>>,
    ) where
        'tcx: 'b,
    {
        debug!("bind_matched_candidate_for_arm_body(block={:?})", block);

        let re_erased = self.hir.tcx().lifetimes.re_erased;
        // Assign each of the bindings. This may trigger moves out of the candidate.
        for binding in bindings {
            let source_info = self.source_info(binding.span);
            let local =
                self.storage_live_binding(block, binding.var_id, binding.span, OutsideGuard);
            if schedule_drops {
                self.schedule_drop_for_binding(binding.var_id, binding.span, OutsideGuard);
            }
            let rvalue = match binding.binding_mode {
                BindingMode::ByValue => {
                    Rvalue::Use(self.consume_by_copy_or_move(binding.source.clone()))
                }
                BindingMode::ByRef(borrow_kind) => {
                    Rvalue::Ref(re_erased, borrow_kind, binding.source)
                }
            };
            self.cfg.push_assign(block, source_info, &local, rvalue);
        }
    }

    /// Each binding (`ref mut var`/`ref var`/`mut var`/`var`, where the bound
    /// `var` has type `T` in the arm body) in a pattern maps to 2 locals. The
    /// first local is a binding for occurrences of `var` in the guard, which
    /// will have type `&T`. The second local is a binding for occurrences of
    /// `var` in the arm body, which will have type `T`.
    fn declare_binding(
        &mut self,
        source_info: SourceInfo,
        visibility_scope: SourceScope,
        mutability: Mutability,
        name: Name,
        mode: BindingMode,
        var_id: HirId,
        var_ty: Ty<'tcx>,
        user_ty: UserTypeProjections,
        has_guard: ArmHasGuard,
        opt_match_place: Option<(Option<Place<'tcx>>, Span)>,
        pat_span: Span,
    ) {
        debug!(
            "declare_binding(var_id={:?}, name={:?}, mode={:?}, var_ty={:?}, \
             visibility_scope={:?}, source_info={:?})",
            var_id, name, mode, var_ty, visibility_scope, source_info
        );

        let tcx = self.hir.tcx();
        let debug_source_info = SourceInfo { span: source_info.span, scope: visibility_scope };
        let binding_mode = match mode {
            BindingMode::ByValue => ty::BindingMode::BindByValue(mutability.into()),
            BindingMode::ByRef(_) => ty::BindingMode::BindByReference(mutability.into()),
        };
        debug!("declare_binding: user_ty={:?}", user_ty);
        let local = LocalDecl::<'tcx> {
            mutability,
            ty: var_ty,
            user_ty,
            source_info,
            internal: false,
            is_block_tail: None,
            local_info: LocalInfo::User(ClearCrossCrate::Set(BindingForm::Var(VarBindingForm {
                binding_mode,
                // hypothetically, `visit_bindings` could try to unzip
                // an outermost hir::Ty as we descend, matching up
                // idents in pat; but complex w/ unclear UI payoff.
                // Instead, just abandon providing diagnostic info.
                opt_ty_info: None,
                opt_match_place,
                pat_span,
            }))),
        };
        let for_arm_body = self.local_decls.push(local);
        self.var_debug_info.push(VarDebugInfo {
            name,
            source_info: debug_source_info,
            place: for_arm_body.into(),
        });
        let locals = if has_guard.0 {
            let ref_for_guard = self.local_decls.push(LocalDecl::<'tcx> {
                // This variable isn't mutated but has a name, so has to be
                // immutable to avoid the unused mut lint.
                mutability: Mutability::Not,
                ty: tcx.mk_imm_ref(tcx.lifetimes.re_erased, var_ty),
                user_ty: UserTypeProjections::none(),
                source_info,
                internal: false,
                is_block_tail: None,
                local_info: LocalInfo::User(ClearCrossCrate::Set(BindingForm::RefForGuard)),
            });
            self.var_debug_info.push(VarDebugInfo {
                name,
                source_info: debug_source_info,
                place: ref_for_guard.into(),
            });
            LocalsForNode::ForGuard { ref_for_guard, for_arm_body }
        } else {
            LocalsForNode::One(for_arm_body)
        };
        debug!("declare_binding: vars={:?}", locals);
        self.var_indices.insert(var_id, locals);
    }
}
