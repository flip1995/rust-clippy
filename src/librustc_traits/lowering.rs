// Copyright 2018 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use rustc::hir::def_id::DefId;
use rustc::hir::intravisit::{self, NestedVisitorMap, Visitor};
use rustc::hir::map::definitions::DefPathData;
use rustc::hir::{self, ImplPolarity};
use rustc::traits::{
    Clause, Clauses, DomainGoal, Goal, PolyDomainGoal, ProgramClause, WhereClauseAtom,
};
use rustc::ty::subst::Substs;
use rustc::ty::{self, Slice, TyCtxt};
use rustc_data_structures::fx::FxHashSet;
use std::mem;
use syntax::ast;

use std::iter;

crate fn provide(p: &mut Providers) {
    *p = Providers {
        program_clauses_for,
        program_clauses_for_env,
        ..*p
    };
}

crate trait Lower<T> {
    /// Lower a rustc construct (e.g. `ty::TraitPredicate`) to a chalk-like type.
    fn lower(&self) -> T;
}

impl<T, U> Lower<Vec<U>> for Vec<T>
where
    T: Lower<U>,
{
    fn lower(&self) -> Vec<U> {
        self.iter().map(|item| item.lower()).collect()
    }
}

impl<'tcx> Lower<WhereClause<'tcx>> for ty::TraitPredicate<'tcx> {
    fn lower(&self) -> WhereClause<'tcx> {
        WhereClause::Implemented(*self)
    }
}

impl<'tcx> Lower<WhereClause<'tcx>> for ty::ProjectionPredicate<'tcx> {
    fn lower(&self) -> WhereClause<'tcx> {
        WhereClause::ProjectionEq(*self)
    }
}

impl<'tcx> Lower<WhereClause<'tcx>> for ty::RegionOutlivesPredicate<'tcx> {
    fn lower(&self) -> WhereClause<'tcx> {
        WhereClause::RegionOutlives(*self)
    }
}

impl<'tcx> Lower<WhereClause<'tcx>> for ty::TypeOutlivesPredicate<'tcx> {
    fn lower(&self) -> WhereClause<'tcx> {
        WhereClause::TypeOutlives(*self)
    }
}

impl<'tcx, T> Lower<DomainGoal<'tcx>> for T
where
    T: Lower<WhereClause<'tcx>>,
{
    fn lower(&self) -> DomainGoal<'tcx> {
        DomainGoal::Holds(self.lower())
    }
}

/// `ty::Binder` is used for wrapping a rustc construction possibly containing generic
/// lifetimes, e.g. `for<'a> T: Fn(&'a i32)`. Instead of representing higher-ranked things
/// in that leaf-form (i.e. `Holds(Implemented(Binder<TraitPredicate>))` in the previous
/// example), we model them with quantified domain goals, e.g. as for the previous example:
/// `forall<'a> { T: Fn(&'a i32) }` which corresponds to something like
/// `Binder<Holds(Implemented(TraitPredicate))>`.
impl<'tcx, T> Lower<PolyDomainGoal<'tcx>> for ty::Binder<T>
where
    T: Lower<DomainGoal<'tcx>> + ty::fold::TypeFoldable<'tcx>,
{
    fn lower(&self) -> PolyDomainGoal<'tcx> {
        self.map_bound_ref(|p| p.lower())
    }
}

impl<'tcx> Lower<PolyDomainGoal<'tcx>> for ty::Predicate<'tcx> {
    fn lower(&self) -> PolyDomainGoal<'tcx> {
        use rustc::ty::Predicate;

        match self {
            Predicate::Trait(predicate) => predicate.lower(),
            Predicate::RegionOutlives(predicate) => predicate.lower(),
            Predicate::TypeOutlives(predicate) => predicate.lower(),
            Predicate::Projection(predicate) => predicate.lower(),
            Predicate::WellFormed(ty) => {
                ty::Binder::dummy(DomainGoal::WellFormed(WellFormed::Ty(*ty)))
            }
            Predicate::ObjectSafe(..)
            | Predicate::ClosureKind(..)
            | Predicate::Subtype(..)
            | Predicate::ConstEvaluatable(..) => unimplemented!(),
        }
    }
}

/// Used for lowered where clauses (see rustc guide).
trait IntoFromEnvGoal {
    // Transforms an existing goal into a FromEnv goal.
    fn into_from_env_goal(self) -> Self;
}

trait IntoWellFormedGoal {
    // Transforms an existing goal into a WellFormed goal.
    fn into_wellformed_goal(self) -> Self;
}

impl<'tcx> IntoFromEnvGoal for DomainGoal<'tcx> {
    fn into_from_env_goal(self) -> DomainGoal<'tcx> {
        use self::WhereClause::*;

        match self {
            DomainGoal::Holds(Implemented(trait_ref)) => {
                DomainGoal::FromEnv(FromEnv::Trait(trait_ref))
            }
            other => other,
        }
    }
}

impl<'tcx> IntoWellFormedGoal for DomainGoal<'tcx> {
    fn into_wellformed_goal(self) -> DomainGoal<'tcx> {
        use self::DomainGoal::*;
        match self {
            Holds(wc_atom) => WellFormed(wc_atom),
            WellFormed(..) | FromEnv(..) | WellFormedTy(..) | FromEnvTy(..) | Normalize(..)
            | RegionOutlives(..) | TypeOutlives(..) => self,
        }
    }
}

crate fn program_clauses_for<'a, 'tcx>(
    tcx: TyCtxt<'a, 'tcx, 'tcx>,
    def_id: DefId,
) -> Clauses<'tcx> {
    match tcx.def_key(def_id).disambiguated_data.data {
        DefPathData::Trait(_) => program_clauses_for_trait(tcx, def_id),
        DefPathData::Impl => program_clauses_for_impl(tcx, def_id),
        DefPathData::AssocTypeInImpl(..) => program_clauses_for_associated_type_value(tcx, def_id),
        DefPathData::AssocTypeInTrait(..) => program_clauses_for_associated_type_def(tcx, def_id),
        DefPathData::TypeNs(..) => program_clauses_for_type_def(tcx, def_id),
        _ => Slice::empty(),
    }
}

crate fn program_clauses_for_env<'a, 'tcx>(
    tcx: TyCtxt<'a, 'tcx, 'tcx>,
    param_env: ty::ParamEnv<'tcx>,
) -> Clauses<'tcx> {
    debug!("program_clauses_for_env(param_env={:?})", param_env);

    let mut last_round = FxHashSet();
    last_round.extend(
        param_env
            .caller_bounds
            .iter()
            .flat_map(|&p| predicate_def_id(p)),
    );

    let mut closure = last_round.clone();
    let mut next_round = FxHashSet();
    while !last_round.is_empty() {
        next_round.extend(
            last_round
                .drain()
                .flat_map(|def_id| {
                    tcx.predicates_of(def_id)
                        .instantiate_identity(tcx)
                        .predicates
                })
                .flat_map(|p| predicate_def_id(p))
                .filter(|&def_id| closure.insert(def_id)),
        );
        mem::swap(&mut next_round, &mut last_round);
    }

    debug!("program_clauses_for_env: closure = {:#?}", closure);

    return tcx.mk_clauses(
        closure
            .into_iter()
            .flat_map(|def_id| tcx.program_clauses_for(def_id).iter().cloned()),
    );

    /// Given that `predicate` is in the environment, returns the
    /// def-id of something (e.g., a trait, associated item, etc)
    /// whose predicates can also be assumed to be true. We will
    /// compute the transitive closure of such things.
    fn predicate_def_id<'tcx>(predicate: ty::Predicate<'tcx>) -> Option<DefId> {
        match predicate {
            ty::Predicate::Trait(predicate) => Some(predicate.def_id()),

            ty::Predicate::Projection(projection) => Some(projection.item_def_id()),

            ty::Predicate::WellFormed(..)
            | ty::Predicate::RegionOutlives(..)
            | ty::Predicate::TypeOutlives(..)
            | ty::Predicate::ObjectSafe(..)
            | ty::Predicate::ClosureKind(..)
            | ty::Predicate::Subtype(..)
            | ty::Predicate::ConstEvaluatable(..) => None,
        }
    }
}

fn program_clauses_for_trait<'a, 'tcx>(
    tcx: TyCtxt<'a, 'tcx, 'tcx>,
    def_id: DefId,
) -> Clauses<'tcx> {
    // `trait Trait<P1..Pn> where WC { .. } // P0 == Self`

    // Rule Implemented-From-Env (see rustc guide)
    //
    // ```
    // forall<Self, P1..Pn> {
    //   Implemented(Self: Trait<P1..Pn>) :- FromEnv(Self: Trait<P1..Pn>)
    // }
    // ```

    // `Self: Trait<P1..Pn>`
    let trait_pred = ty::TraitPredicate {
        trait_ref: ty::TraitRef::identity(tcx, def_id),
    };

    // `Implemented(Self: Trait<P1..Pn>)`
    let impl_trait: DomainGoal = trait_pred.lower();

    // `FromEnv(Self: Trait<P1..Pn>)`
    let from_env_goal = impl_trait.into_from_env_goal().into_goal();
    let hypotheses = tcx.intern_goals(&[from_env_goal]);

    // `Implemented(Self: Trait<P1..Pn>) :- FromEnv(Self: Trait<P1..Pn>)`
    let implemented_from_env = ProgramClause {
        goal: impl_trait,
        hypotheses,
    };

    let clauses = iter::once(Clause::ForAll(ty::Binder::dummy(implemented_from_env)));

    // Rule Implied-Bound-From-Trait
    //
    // For each where clause WC:
    // ```
    // forall<Self, P1..Pn> {
    //   FromEnv(WC) :- FromEnv(Self: Trait<P1..Pn)
    // }
    // ```

    // `FromEnv(WC) :- FromEnv(Self: Trait<P1..Pn>)`, for each where clause WC
    let where_clauses = &tcx.predicates_defined_on(def_id).predicates;
    let implied_bound_clauses = where_clauses
        .into_iter()
        .map(|wc| wc.lower())

        // `FromEnv(WC) :- FromEnv(Self: Trait<P1..Pn>)`
        .map(|wc| wc.map_bound(|goal| ProgramClause {
            goal: goal.into_from_env_goal(),
            hypotheses,
        }))
        .map(|wc| implied_bound_from_trait(tcx, trait_pred, wc));
    let wellformed_clauses = wellformed_from_bound(tcx, trait_pred, &where_clauses[1..]);
    tcx.mk_clauses(clauses.chain(implied_bound_clauses).chain(wellformed_clauses))
}

fn wellformed_from_bound<'a, 'tcx>(
    tcx: TyCtxt<'a, 'tcx, 'tcx>,
    trait_pred: ty::TraitPredicate<'tcx>,
    where_clauses: &[ty::Predicate<'tcx>],
) -> iter::Once<Clause<'tcx>> {
    // Rule WellFormed-TraitRef
    //
    // For each where clause WC:
    // forall<Self, P1..Pn> {
    //   WellFormed(Self: Trait<P1..Pn>) :- Implemented(Self: Trait<P1..Pn>) && WellFormed(WC)
    // }

    // WellFormed(Self: Trait<P1..Pn>)
    let wellformed_trait = DomainGoal::WellFormed(WhereClauseAtom::Implemented(trait_pred));
    // Implemented(Self: Trait<P1..Pn>)
    let impl_trait = ty::Binder::dummy(DomainGoal::Holds(WhereClauseAtom::Implemented(trait_pred)));
    // WellFormed(WC)
    let wellformed_wc = where_clause
        .lower()
        .map_bound(|wc| wc.into_wellformed_goal());
    // Implemented(Self: Trait<P1..Pn>) && WellFormed(WC)
    let mut wcs = vec![impl_trait];
    wcs.extend(wellformed_wcs);

    let clause = ProgramClause {
        goal: wellformed_trait,
        hypotheses: tcx.mk_goals(
            wcs.into_iter()
                .map(|wc| Goal::from_poly_domain_goal(wc, tcx)),
        ),
    };
    iter::once(Clause::ForAll(ty::Binder::dummy(clause)))
}

fn program_clauses_for_impl<'a, 'tcx>(tcx: TyCtxt<'a, 'tcx, 'tcx>, def_id: DefId) -> Clauses<'tcx> {
    if let ImplPolarity::Negative = tcx.impl_polarity(def_id) {
        return Slice::empty();
    }

    // Rule Implemented-From-Impl (see rustc guide)
    //
    // `impl<P0..Pn> Trait<A1..An> for A0 where WC { .. }`
    //
    // ```
    // forall<P0..Pn> {
    //   Implemented(A0: Trait<A1..An>) :- WC
    // }
    // ```

    let trait_ref = tcx.impl_trait_ref(def_id).expect("not an impl");

    // `Implemented(A0: Trait<A1..An>)`
    let trait_pred = ty::TraitPredicate { trait_ref }.lower();

    // `WC`
    let where_clauses = tcx.predicates_of(def_id).predicates.lower();

    // `Implemented(A0: Trait<A1..An>) :- WC`
    let clause = ProgramClause {
        goal: trait_pred,
        hypotheses: tcx.mk_goals(
            where_clauses
                .into_iter()
                .map(|wc| Goal::from_poly_domain_goal(wc, tcx)),
        ),
    };
    tcx.intern_clauses(&[Clause::ForAll(ty::Binder::dummy(clause))])
}

pub fn program_clauses_for_type_def<'a, 'tcx>(
    tcx: TyCtxt<'a, 'tcx, 'tcx>,
    def_id: DefId,
) -> Clauses<'tcx> {
    // Rule WellFormed-Type
    //
    // `struct Ty<P1..Pn> where WC1, ..., WCm`
    //
    // ```
    // forall<P1..Pn> {
    //   WellFormed(Ty<...>) :- WC1, ..., WCm`
    // }
    // ```

    // `Ty<...>`
    let ty = tcx.type_of(def_id);

    // `WC`
    let where_clauses = tcx.predicates_of(def_id).predicates.lower();

    // `WellFormed(Ty<...>) :- WC1, ..., WCm`
    let well_formed = ProgramClause {
        goal: DomainGoal::WellFormed(WellFormed::Ty(ty)),
        hypotheses: tcx.mk_goals(
            where_clauses
                .iter()
                .cloned()
                .map(|wc| Goal::from_poly_domain_goal(wc, tcx)),
        ),
    };

    let well_formed_clause = iter::once(Clause::ForAll(ty::Binder::dummy(well_formed)));

    // Rule FromEnv-Type
    //
    // For each where clause `WC`:
    // ```
    // forall<P1..Pn> {
    //   FromEnv(WC) :- FromEnv(Ty<...>)
    // }
    // ```

    // `FromEnv(Ty<...>)`
    let from_env_goal = DomainGoal::FromEnv(FromEnv::Ty(ty)).into_goal();
    let hypotheses = tcx.intern_goals(&[from_env_goal]);

    // For each where clause `WC`:
    let from_env_clauses = where_clauses
        .into_iter()

        // `FromEnv(WC) :- FromEnv(Ty<...>)`
        .map(|wc| wc.map_bound(|goal| ProgramClause {
            goal: goal.into_from_env_goal(),
            hypotheses,
        }))

        .map(Clause::ForAll);

    tcx.mk_clauses(well_formed_clause.chain(from_env_clauses))
}

pub fn program_clauses_for_associated_type_def<'a, 'tcx>(
    _tcx: TyCtxt<'a, 'tcx, 'tcx>,
    _item_id: DefId,
) -> Clauses<'tcx> {
    unimplemented!()
}

pub fn program_clauses_for_associated_type_value<'a, 'tcx>(
    tcx: TyCtxt<'a, 'tcx, 'tcx>,
    item_id: DefId,
) -> Clauses<'tcx> {
    // Rule Normalize-From-Impl (see rustc guide)
    //
    // ```impl<P0..Pn> Trait<A1..An> for A0
    // {
    //     type AssocType<Pn+1..Pm> = T;
    // }```
    //
    // FIXME: For the moment, we don't account for where clauses written on the associated
    // ty definition (i.e. in the trait def, as in `type AssocType<T> where T: Sized`).
    // ```
    // forall<P0..Pm> {
    //   forall<Pn+1..Pm> {
    //     Normalize(<A0 as Trait<A1..An>>::AssocType<Pn+1..Pm> -> T) :-
    //       Implemented(A0: Trait<A1..An>)
    //   }
    // }
    // ```

    let item = tcx.associated_item(item_id);
    debug_assert_eq!(item.kind, ty::AssociatedKind::Type);
    let impl_id = match item.container {
        ty::AssociatedItemContainer::ImplContainer(impl_id) => impl_id,
        _ => bug!("not an impl container"),
    };

    // `A0 as Trait<A1..An>`
    let trait_ref = tcx.impl_trait_ref(impl_id).unwrap();

    // `T`
    let ty = tcx.type_of(item_id);

    // `Implemented(A0: Trait<A1..An>)`
    let trait_implemented = ty::Binder::dummy(ty::TraitPredicate { trait_ref }.lower());

    // `Implemented(A0: Trait<A1..An>)`
    let hypotheses = vec![trait_implemented];

    // `<A0 as Trait<A1..An>>::AssocType<Pn+1..Pm>`
    let projection_ty = ty::ProjectionTy::from_ref_and_name(tcx, trait_ref, item.ident);

    // `Normalize(<A0 as Trait<A1..An>>::AssocType<Pn+1..Pm> -> T)`
    let normalize_goal = DomainGoal::Normalize(ty::ProjectionPredicate { projection_ty, ty });

    // `Normalize(... -> T) :- ...`
    let clause = ProgramClause {
        goal: normalize_goal,
        hypotheses: tcx.mk_goals(
            hypotheses
                .into_iter()
                .map(|wc| Goal::from_poly_domain_goal(wc, tcx)),
        ),
    };
    tcx.intern_clauses(&[Clause::ForAll(ty::Binder::dummy(clause))])
}

pub fn dump_program_clauses<'a, 'tcx>(tcx: TyCtxt<'a, 'tcx, 'tcx>) {
    if !tcx.features().rustc_attrs {
        return;
    }

    let mut visitor = ClauseDumper { tcx };
    tcx.hir
        .krate()
        .visit_all_item_likes(&mut visitor.as_deep_visitor());
}

struct ClauseDumper<'a, 'tcx: 'a> {
    tcx: TyCtxt<'a, 'tcx, 'tcx>,
}

impl<'a, 'tcx> ClauseDumper<'a, 'tcx> {
    fn process_attrs(&mut self, node_id: ast::NodeId, attrs: &[ast::Attribute]) {
        let def_id = self.tcx.hir.local_def_id(node_id);
        for attr in attrs {
            let mut clauses = None;

            if attr.check_name("rustc_dump_program_clauses") {
                clauses = Some(self.tcx.program_clauses_for(def_id));
            }

            if attr.check_name("rustc_dump_env_program_clauses") {
                let param_env = self.tcx.param_env(def_id);
                clauses = Some(self.tcx.program_clauses_for_env(param_env));
            }

            if let Some(clauses) = clauses {
                let mut err = self
                    .tcx
                    .sess
                    .struct_span_err(attr.span, "program clause dump");

                let mut strings: Vec<_> = clauses
                    .iter()
                    .map(|clause| {
                        // Skip the top-level binder for a less verbose output
                        let program_clause = match clause {
                            Clause::Implies(program_clause) => program_clause,
                            Clause::ForAll(program_clause) => program_clause.skip_binder(),
                        };
                        format!("{}", program_clause)
                    })
                    .collect();

                strings.sort();

                for string in strings {
                    err.note(&string);
                }

                err.emit();
            }
        }
    }
}

impl<'a, 'tcx> Visitor<'tcx> for ClauseDumper<'a, 'tcx> {
    fn nested_visit_map<'this>(&'this mut self) -> NestedVisitorMap<'this, 'tcx> {
        NestedVisitorMap::OnlyBodies(&self.tcx.hir)
    }

    fn visit_item(&mut self, item: &'tcx hir::Item) {
        self.process_attrs(item.id, &item.attrs);
        intravisit::walk_item(self, item);
    }

    fn visit_trait_item(&mut self, trait_item: &'tcx hir::TraitItem) {
        self.process_attrs(trait_item.id, &trait_item.attrs);
        intravisit::walk_trait_item(self, trait_item);
    }

    fn visit_impl_item(&mut self, impl_item: &'tcx hir::ImplItem) {
        self.process_attrs(impl_item.id, &impl_item.attrs);
        intravisit::walk_impl_item(self, impl_item);
    }

    fn visit_struct_field(&mut self, s: &'tcx hir::StructField) {
        self.process_attrs(s.id, &s.attrs);
        intravisit::walk_struct_field(self, s);
    }
}
