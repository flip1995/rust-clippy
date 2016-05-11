// Copyright 2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use super::combine::CombineFields;
use super::{Subtype};
use super::type_variable::{EqTo};

use ty::{self, Ty, TyCtxt};
use ty::TyVar;
use ty::relate::{Relate, RelateResult, TypeRelation};
use traits::PredicateObligations;

/// Ensures `a` is made equal to `b`. Returns `a` on success.
pub struct Equate<'a, 'gcx: 'a+'tcx, 'tcx: 'a> {
    fields: CombineFields<'a, 'gcx, 'tcx>
}

impl<'a, 'gcx, 'tcx> Equate<'a, 'gcx, 'tcx> {
    pub fn new(fields: CombineFields<'a, 'gcx, 'tcx>) -> Equate<'a, 'gcx, 'tcx> {
        Equate { fields: fields }
    }

    pub fn obligations(self) -> PredicateObligations<'tcx> {
        self.fields.obligations
    }
}

impl<'a, 'gcx, 'tcx> TypeRelation<'a, 'gcx, 'tcx> for Equate<'a, 'gcx, 'tcx> {
    fn tag(&self) -> &'static str { "Equate" }

    fn tcx(&self) -> TyCtxt<'a, 'gcx, 'tcx> { self.fields.tcx() }

    fn a_is_expected(&self) -> bool { self.fields.a_is_expected }

    fn relate_with_variance<T: Relate<'tcx>>(&mut self,
                                             _: ty::Variance,
                                             a: &T,
                                             b: &T)
                                             -> RelateResult<'tcx, T>
    {
        self.relate(a, b)
    }

    fn tys(&mut self, a: Ty<'tcx>, b: Ty<'tcx>) -> RelateResult<'tcx, Ty<'tcx>> {
        debug!("{}.tys({:?}, {:?})", self.tag(),
               a, b);
        if a == b { return Ok(a); }

        let infcx = self.fields.infcx;
        let a = infcx.type_variables.borrow_mut().replace_if_possible(a);
        let b = infcx.type_variables.borrow_mut().replace_if_possible(b);
        match (&a.sty, &b.sty) {
            (&ty::TyInfer(TyVar(a_id)), &ty::TyInfer(TyVar(b_id))) => {
                infcx.type_variables.borrow_mut().relate_vars(a_id, EqTo, b_id);
                Ok(a)
            }

            (&ty::TyInfer(TyVar(a_id)), _) => {
                self.fields.instantiate(b, EqTo, a_id)?;
                Ok(a)
            }

            (_, &ty::TyInfer(TyVar(b_id))) => {
                self.fields.instantiate(a, EqTo, b_id)?;
                Ok(a)
            }

            _ => {
                self.fields.infcx.super_combine_tys(self, a, b)?;
                Ok(a)
            }
        }
    }

    fn regions(&mut self, a: ty::Region, b: ty::Region) -> RelateResult<'tcx, ty::Region> {
        debug!("{}.regions({:?}, {:?})",
               self.tag(),
               a,
               b);
        let origin = Subtype(self.fields.trace.clone());
        self.fields.infcx.region_vars.make_eqregion(origin, a, b);
        Ok(a)
    }

    fn binders<T>(&mut self, a: &ty::Binder<T>, b: &ty::Binder<T>)
                  -> RelateResult<'tcx, ty::Binder<T>>
        where T: Relate<'tcx>
    {
        self.fields.higher_ranked_sub(a, b)?;
        self.fields.higher_ranked_sub(b, a)
    }
}
