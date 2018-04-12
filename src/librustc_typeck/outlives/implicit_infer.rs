// Copyright 2013 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![allow(unused)]

use rustc::hir;
use rustc::hir::def::{CtorKind, Def};
use rustc::hir::def_id::{self, CrateNum, DefId, LOCAL_CRATE};
use rustc::hir::itemlikevisit::ItemLikeVisitor;
use rustc::hir::map as hir_map;
use rustc::ty::Slice;
use rustc::ty::maps::Providers;
use rustc::ty::outlives::Component;
use rustc::ty::subst::{Kind, Subst, UnpackedKind};
use rustc::ty::{self, AdtKind, CratePredicatesMap, Region, RegionKind, ReprOptions,
                ToPolyTraitRef, ToPredicate, Ty, TyCtxt};
use rustc::util::nodemap::{FxHashMap, FxHashSet};
use rustc_data_structures::sync::Lrc;
use syntax::{abi, ast};
use syntax_pos::{Span, DUMMY_SP};

/// Infer predicates for the items in the crate.
///
/// global_inferred_outlives: this is initially the empty map that
///     was generated by walking the items in the crate. This will
///     now be filled with inferred predicates.
pub fn infer_predicates<'tcx>(
    tcx: TyCtxt<'_, 'tcx, 'tcx>,
    explicit_map: &FxHashMap<DefId, Lrc<Vec<ty::Predicate<'tcx>>>>,
) -> FxHashMap<DefId, RequiredPredicates<'tcx>> {
    debug!("infer_predicates");

    let mut predicates_added = true;

    let mut global_inferred_outlives = FxHashMap::default();

    // If new predicates were added then we need to re-calculate
    // all crates since there could be new implied predicates.
    while predicates_added {
        predicates_added = false;

        let mut visitor = InferVisitor {
            tcx: tcx,
            global_inferred_outlives: &mut global_inferred_outlives,
            predicates_added: &mut predicates_added,
            explicit_map: explicit_map,
        };

        // Visit all the crates and infer predicates
        tcx.hir.krate().visit_all_item_likes(&mut visitor);
    }

    global_inferred_outlives
}

pub struct InferVisitor<'cx, 'tcx: 'cx> {
    tcx: TyCtxt<'cx, 'tcx, 'tcx>,
    global_inferred_outlives: &'cx mut FxHashMap<DefId, RequiredPredicates<'tcx>>,
    predicates_added: &'cx mut bool,
    explicit_map: &'cx FxHashMap<DefId, Lrc<Vec<ty::Predicate<'tcx>>>>,
}

/// Tracks the `T: 'a` or `'a: 'a` predicates that we have inferred
/// must be added to the struct header.
type RequiredPredicates<'tcx> = FxHashSet<ty::OutlivesPredicate<Kind<'tcx>, ty::Region<'tcx>>>;

impl<'cx, 'tcx> ItemLikeVisitor<'tcx> for InferVisitor<'cx, 'tcx> {
    fn visit_item(&mut self, item: &hir::Item) {
        let item_did = self.tcx.hir.local_def_id(item.id);

        debug!("InferVisitor::visit_item(item={:?})", item_did);

        let node_id = self.tcx
            .hir
            .as_local_node_id(item_did)
            .expect("expected local def-id");
        let item = match self.tcx.hir.get(node_id) {
            hir::map::NodeItem(item) => item,
            _ => bug!(),
        };

        let mut item_required_predicates = RequiredPredicates::default();
        match item.node {
            hir::ItemUnion(..) | hir::ItemEnum(..) | hir::ItemStruct(..) => {
                let adt_def = self.tcx.adt_def(item_did);

                // Iterate over all fields in item_did
                for field_def in adt_def.all_fields() {
                    // Calculating the predicate requirements necessary
                    // for item_did.
                    //
                    // For field of type &'a T (reference) or TyAdt
                    // (struct/enum/union) there will be outlive
                    // requirements for adt_def.
                    let field_ty = self.tcx.type_of(field_def.did);
                    insert_required_predicates_to_be_wf(
                        self.tcx,
                        field_ty,
                        self.global_inferred_outlives,
                        &mut item_required_predicates,
                        self.explicit_map,
                    );
                }
            }

            _ => {}
        };

        // If new predicates were added (`local_predicate_map` has more
        // predicates than the `global_inferred_outlives`), the new predicates
        // might result in implied predicates for their parent types.
        // Therefore mark `predicates_added` as true and which will ensure
        // we walk the crates again and re-calculate predicates for all
        // items.
        let item_predicates_len: usize = self.global_inferred_outlives
            .get(&item_did)
            .map(|p| p.len())
            .unwrap_or(0);
        if item_required_predicates.len() > item_predicates_len {
            *self.predicates_added = true;
            self.global_inferred_outlives
                .insert(item_did, item_required_predicates);
        }
    }

    fn visit_trait_item(&mut self, trait_item: &'tcx hir::TraitItem) {}

    fn visit_impl_item(&mut self, impl_item: &'tcx hir::ImplItem) {}
}

fn insert_required_predicates_to_be_wf<'tcx>(
    tcx: TyCtxt<'_, 'tcx, 'tcx>,
    field_ty: Ty<'tcx>,
    global_inferred_outlives: &FxHashMap<DefId, RequiredPredicates<'tcx>>,
    required_predicates: &mut RequiredPredicates<'tcx>,
    explicit_map: &FxHashMap<DefId, Lrc<Vec<ty::Predicate<'tcx>>>>,
) {
    for ty in field_ty.walk() {
        match ty.sty {
            // The field is of type &'a T which means that we will have
            // a predicate requirement of T: 'a (T outlives 'a).
            //
            // We also want to calculate potential predicates for the T
            ty::TyRef(region, mt) => {
                insert_outlives_predicate(tcx, mt.ty.into(), region, required_predicates);
            }

            // For each TyAdt (struct/enum/union) type `Foo<'a, T>`, we
            // can load the current set of inferred and explicit
            // predicates from `global_inferred_outlives` and filter the
            // ones that are TypeOutlives.
            //
            ty::TyAdt(def, substs) => {
                // First check the inferred predicates
                //
                // Example 1:
                //
                //     struct Foo<'a, T> {
                //         field1: Bar<'a, T>
                //     }
                //
                //     struct Bar<'b, U> {
                //         field2: &'b U
                //     }
                //
                // Here, when processing the type of `field1`, we would
                // request the set of implicit predicates computed for `Bar`
                // thus far. This will initially come back empty, but in next
                // round we will get `U: 'b`. We then apply the substitution
                // `['b => 'a, U => T]` and thus get the requirement that `T:
                // 'a` holds for `Foo`.
                if let Some(unsubstituted_predicates) = global_inferred_outlives.get(&def.did) {
                    for unsubstituted_predicate in unsubstituted_predicates {
                        // `unsubstituted_predicate` is `U: 'b` in the
                        // example above.  So apply the substitution to
                        // get `T: 'a` (or `predicate`):
                        let predicate = unsubstituted_predicate.subst(tcx, substs);
                        insert_outlives_predicate(
                            tcx,
                            predicate.0,
                            predicate.1,
                            required_predicates,
                        );
                    }
                }

                // Check if the type has any explicit predicates that need
                // to be added to `required_predicates`
                // let _: () = substs.region_at(0);
                check_explicit_predicates(tcx, &def.did, substs, required_predicates, explicit_map);
            }

            ty::TyDynamic(obj, region) => {
                // FIXME This corresponds to `dyn Trait<..>`. In this
                // case, we should use the explicit predicates as
                // well.
                if let Some(p) = obj.principal() {
                    check_explicit_predicates(
                        tcx,
                        &p.skip_binder().def_id,
                        &[region.into()],
                        required_predicates,
                        explicit_map,
                    );
                }
            }

            ty::TyProjection(obj) => {
                // FIXME This corresponds to `<T as Foo<'a>>::Bar`. In this case, we should use the
                // explicit predicates as well.
                check_explicit_predicates(
                    tcx,
                    &obj.item_def_id,
                    obj.substs,
                    required_predicates,
                    explicit_map,
                );
            }

            _ => {}
        }
    }
}

/// We also have to check the explicit predicates
/// declared on the type.
///
///     struct Foo<'a, T> {
///         field1: Bar<T>
///     }
///
///     struct Bar<U> where U: 'static, U: Foo {
///         ...
///     }
///
/// Here, we should fetch the explicit predicates, which
/// will give us `U: 'static` and `U: Foo`. The latter we
/// can ignore, but we will want to process `U: 'static`,
/// applying the substitution as above.
fn check_explicit_predicates<'tcx>(
    tcx: TyCtxt<'_, 'tcx, 'tcx>,
    def_id: &DefId,
    substs: &[Kind<'tcx>],
    required_predicates: &mut RequiredPredicates<'tcx>,
    explicit_map: &FxHashMap<DefId, Lrc<Vec<ty::Predicate<'tcx>>>>,
) {
    if let Some(general_predicates) = explicit_map.get(def_id) {
        for general_predicate in general_predicates.iter() {
            match general_predicate {
                // `poly` is `PolyTypeOutlivesPredicate<OutlivesPredicate<Ty>>`
                // where OutlivesPredicate<type1, region1> is the predicate
                // we want to add.
                ty::Predicate::TypeOutlives(poly) => {
                    let predicate = poly.0.subst(tcx, substs);
                    insert_outlives_predicate(
                        tcx,
                        predicate.0.into(),
                        predicate.1,
                        required_predicates,
                    );
                }

                // `poly` is `PolyRegionOutlivesPredicate<OutlivesPredicate<Ty>>`
                // where OutlivesPredicate<region1, region2> is the predicate
                // we want to add.
                ty::Predicate::RegionOutlives(poly) => {
                    let predicate = poly.0.subst(tcx, substs);
                    insert_outlives_predicate(
                        tcx,
                        predicate.0.into(),
                        predicate.1,
                        required_predicates,
                    );
                }

                ty::Predicate::Trait(..)
                | ty::Predicate::Projection(..)
                | ty::Predicate::WellFormed(..)
                | ty::Predicate::ObjectSafe(..)
                | ty::Predicate::ClosureKind(..)
                | ty::Predicate::Subtype(..)
                | ty::Predicate::ConstEvaluatable(..) => (),
            }
        }
    }
}

/// Given a requirement `T: 'a` or `'b: 'a`, deduce the
/// outlives_component and add it to `required_predicates`
fn insert_outlives_predicate<'tcx>(
    tcx: TyCtxt<'_, 'tcx, 'tcx>,
    kind: Kind<'tcx>,
    outlived_region: Region<'tcx>,
    required_predicates: &mut RequiredPredicates<'tcx>,
) {
    // If the `'a` region is bound within the field type itself, we
    // don't want to propagate this constraint to the header.
    if !is_free_region(outlived_region) {
        return;
    }

    match kind.unpack() {
        UnpackedKind::Type(ty) => {
            // `T: 'outlived_region` for some type `T`
            // But T could be a lot of things:
            // e.g., if `T = &'b u32`, then `'b: 'outlived_region` is
            // what we want to add.
            //
            // Or if within `struct Foo<U>` you had `T = Vec<U>`, then
            // we would want to add `U: 'outlived_region`
            for component in tcx.outlives_components(ty) {
                match component {
                    Component::Region(r) => {
                        // This would arise from something like:
                        //
                        // ```
                        // struct Foo<'a, 'b> {
                        //    x:  &'a &'b u32
                        // }
                        // ```
                        //
                        // Here `outlived_region = 'a` and `kind = &'b
                        // u32`.  Decomposing `&'b u32` into
                        // components would yield `'b`, and we add the
                        // where clause that `'b: 'a`.
                        insert_outlives_predicate(
                            tcx,
                            r.into(),
                            outlived_region,
                            required_predicates,
                        );
                    }

                    Component::Param(param_ty) => {
                        // param_ty: ty::ParamTy
                        // This would arise from something like:
                        //
                        // ```
                        // struct Foo<'a, U> {
                        //    x:  &'a Vec<U>
                        // }
                        // ```
                        //
                        // Here `outlived_region = 'a` and `kind =
                        // Vec<U>`.  Decomposing `Vec<U>` into
                        // components would yield `U`, and we add the
                        // where clause that `U: 'a`.
                        let ty: Ty<'tcx> = tcx.mk_param(param_ty.idx, param_ty.name);
                        required_predicates
                            .insert(ty::OutlivesPredicate(ty.into(), outlived_region));
                    }

                    Component::Projection(proj_ty) => {
                        // This would arise from something like:
                        //
                        // ```
                        // struct Foo<'a, T: Iterator> {
                        //    x:  &'a <T as Iterator>::Item
                        // }
                        // ```
                        //
                        // Here we want to add an explicit `where <T as Iterator>::Item: 'a`.
                        let ty: Ty<'tcx> = tcx.mk_projection(proj_ty.item_def_id, proj_ty.substs);
                        required_predicates
                            .insert(ty::OutlivesPredicate(ty.into(), outlived_region));
                    }

                    Component::EscapingProjection(_) => {
                        // As above, but the projection involves
                        // late-bound regions.  Therefore, the WF
                        // requirement is not checked in type definition
                        // but at fn call site, so ignore it.
                        //
                        // ```
                        // struct Foo<'a, T: Iterator> {
                        //    x: for<'b> fn(<&'b T as Iterator>::Item)
                        //              //  ^^^^^^^^^^^^^^^^^^^^^^^^^
                        // }
                        // ```
                        //
                        // Since `'b` is not in scope on `Foo`, can't
                        // do anything here, ignore it.
                    }

                    Component::UnresolvedInferenceVariable(_) => bug!("not using infcx"),
                }
            }
        }

        UnpackedKind::Lifetime(r) => {
            if !is_free_region(r) {
                return;
            }
            required_predicates.insert(ty::OutlivesPredicate(kind, outlived_region));
        }
    }
}

fn is_free_region(region: Region<'_>) -> bool {
    // First, screen for regions that might appear in a type header.
    match region {
        // *These* correspond to `T: 'a` relationships where `'a` is
        // either declared on the type or `'static`:
        //
        //     struct Foo<'a, T> {
        //         field: &'a T, // this would generate a ReEarlyBound referencing `'a`
        //         field2: &'static T, // this would generate a ReStatic
        //     }
        //
        // We care about these, so fall through.
        RegionKind::ReStatic | RegionKind::ReEarlyBound(_) => true,

        // Late-bound regions can appear in `fn` types:
        //
        //     struct Foo<T> {
        //         field: for<'b> fn(&'b T) // e.g., 'b here
        //     }
        //
        // The type above might generate a `T: 'b` bound, but we can
        // ignore it.  We can't put it on the struct header anyway.
        RegionKind::ReLateBound(..) => false,

        // These regions don't appear in types from type declarations:
        RegionKind::ReEmpty
        | RegionKind::ReErased
        | RegionKind::ReClosureBound(..)
        | RegionKind::ReCanonical(..)
        | RegionKind::ReScope(..)
        | RegionKind::ReVar(..)
        | RegionKind::ReSkolemized(..)
        | RegionKind::ReFree(..) => {
            bug!("unexpected region in outlives inference: {:?}", region);
        }
    }
}
