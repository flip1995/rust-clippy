// Copyright 2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! See `doc.rs` for high-level documentation

use super::SelectionContext;
use super::{Obligation, ObligationCause};
use super::util;

use middle::subst::Subst;
use middle::ty::{self, Ty};
use middle::infer::InferCtxt;
use std::collections::HashSet;
use std::rc::Rc;
use syntax::ast;
use syntax::codemap::DUMMY_SP;
use util::ppaux::Repr;

pub fn impl_can_satisfy(infcx: &InferCtxt,
                        impl1_def_id: ast::DefId,
                        impl2_def_id: ast::DefId)
                        -> bool
{
    debug!("impl_can_satisfy(\
           impl1_def_id={}, \
           impl2_def_id={})",
           impl1_def_id.repr(infcx.tcx),
           impl2_def_id.repr(infcx.tcx));

    // `impl1` provides an implementation of `Foo<X,Y> for Z`.
    let impl1_substs =
        util::fresh_substs_for_impl(infcx, DUMMY_SP, impl1_def_id);
    let impl1_trait_ref =
        (*ty::impl_trait_ref(infcx.tcx, impl1_def_id).unwrap()).subst(infcx.tcx, &impl1_substs);

    // Determine whether `impl2` can provide an implementation for those
    // same types.
    let param_env = ty::empty_parameter_environment(infcx.tcx);
    let mut selcx = SelectionContext::intercrate(infcx, &param_env);
    let obligation = Obligation::new(ObligationCause::dummy(),
                                     ty::Binder(ty::TraitPredicate {
                                         trait_ref: Rc::new(impl1_trait_ref),
                                     }));
    debug!("impl_can_satisfy(obligation={})", obligation.repr(infcx.tcx));
    selcx.evaluate_impl(impl2_def_id, &obligation)
}

#[allow(missing_copy_implementations)]
pub enum OrphanCheckErr {
    NoLocalInputType,
    UncoveredTypeParameter(ty::ParamTy),
}

/// Checks the coherence orphan rules. `impl_def_id` should be the
/// def-id of a trait impl. To pass, either the trait must be local, or else
/// two conditions must be satisfied:
///
/// 1. At least one of the input types must involve a local type.
/// 2. All type parameters must be covered by a local type.
pub fn orphan_check(tcx: &ty::ctxt,
                    impl_def_id: ast::DefId)
                    -> Result<(), OrphanCheckErr>
{
    debug!("impl_is_local({})", impl_def_id.repr(tcx));

    // We only except this routine to be invoked on implementations
    // of a trait, not inherent implementations.
    let trait_ref = ty::impl_trait_ref(tcx, impl_def_id).unwrap();
    debug!("trait_ref={}", trait_ref.repr(tcx));

    // If the *trait* is local to the crate, ok.
    if trait_ref.def_id.krate == ast::LOCAL_CRATE {
        debug!("trait {} is local to current crate",
               trait_ref.def_id.repr(tcx));
        return Ok(());
    }

    // Check condition 1: at least one type must be local.
    if !trait_ref.input_types().iter().any(|&t| ty_reaches_local(tcx, t)) {
        return Err(OrphanCheckErr::NoLocalInputType);
    }

    // Check condition 2: type parameters must be "covered" by a local type.
    let covered_params: HashSet<_> =
        trait_ref.input_types().iter()
                               .flat_map(|&t| type_parameters_covered_by_ty(tcx, t).into_iter())
                               .collect();
    let all_params: HashSet<_> =
        trait_ref.input_types().iter()
                               .flat_map(|&t| type_parameters_reachable_from_ty(t).into_iter())
                               .collect();
    for &param in all_params.difference(&covered_params) {
        return Err(OrphanCheckErr::UncoveredTypeParameter(param));
    }

    return Ok(());
}

fn ty_reaches_local<'tcx>(tcx: &ty::ctxt<'tcx>, ty: Ty<'tcx>) -> bool {
    ty.walk().any(|t| ty_is_local_constructor(tcx, t))
}

fn ty_is_local_constructor<'tcx>(tcx: &ty::ctxt<'tcx>, ty: Ty<'tcx>) -> bool {
    debug!("ty_is_local_constructor({})", ty.repr(tcx));

    match ty.sty {
        ty::ty_bool |
        ty::ty_char |
        ty::ty_int(..) |
        ty::ty_uint(..) |
        ty::ty_float(..) |
        ty::ty_str(..) |
        ty::ty_bare_fn(..) |
        ty::ty_closure(..) |
        ty::ty_vec(..) |
        ty::ty_ptr(..) |
        ty::ty_rptr(..) |
        ty::ty_tup(..) |
        ty::ty_param(..) |
        ty::ty_projection(..) => {
            false
        }

        ty::ty_enum(def_id, _) |
        ty::ty_struct(def_id, _) => {
            def_id.krate == ast::LOCAL_CRATE
        }

        ty::ty_uniq(_) => { // treat ~T like Box<T>
            let krate = tcx.lang_items.owned_box().map(|d| d.krate);
            krate == Some(ast::LOCAL_CRATE)
        }

        ty::ty_trait(ref tt) => {
            tt.principal_def_id().krate == ast::LOCAL_CRATE
        }

        ty::ty_unboxed_closure(..) |
        ty::ty_infer(..) |
        ty::ty_open(..) |
        ty::ty_err => {
            tcx.sess.bug(
                format!("ty_is_local invoked on unexpected type: {}",
                        ty.repr(tcx))[])
        }
    }
}

fn type_parameters_covered_by_ty<'tcx>(tcx: &ty::ctxt<'tcx>,
                                 ty: Ty<'tcx>)
                                 -> HashSet<ty::ParamTy>
{
    if ty_is_local_constructor(tcx, ty) {
        type_parameters_reachable_from_ty(ty)
    } else {
        ty.walk_children().flat_map(|t| type_parameters_covered_by_ty(tcx, t).into_iter()).collect()
    }
}

/// All type parameters reachable from `ty`
fn type_parameters_reachable_from_ty<'tcx>(ty: Ty<'tcx>) -> HashSet<ty::ParamTy> {
    ty.walk()
        .filter_map(|t| {
            match t.sty {
                ty::ty_param(ref param_ty) => Some(param_ty.clone()),
                _ => None,
            }
        })
        .collect()
}

