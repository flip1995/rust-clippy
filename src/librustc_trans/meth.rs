// Copyright 2012 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use llvm::ValueRef;
use rustc::traits;
use callee;
use common::*;
use builder::Builder;
use consts;
use glue;
use machine;
use type_::Type;
use type_of::*;
use value::Value;
use rustc::ty;

// drop_glue pointer, size, align.
const VTABLE_OFFSET: usize = 3;

/// Extracts a method from a trait object's vtable, at the specified index.
pub fn get_virtual_method<'a, 'tcx>(bcx: &Builder<'a, 'tcx>,
                                    llvtable: ValueRef,
                                    vtable_index: usize) -> ValueRef {
    // Load the data pointer from the object.
    debug!("get_virtual_method(vtable_index={}, llvtable={:?})",
           vtable_index, Value(llvtable));

    let ptr = bcx.load_nonnull(bcx.gepi(llvtable, &[vtable_index + VTABLE_OFFSET]), None);
    // Vtable loads are invariant
    bcx.set_invariant_load(ptr);
    ptr
}

/// Creates a dynamic vtable for the given type and vtable origin.
/// This is used only for objects.
///
/// The vtables are cached instead of created on every call.
///
/// The `trait_ref` encodes the erased self type. Hence if we are
/// making an object `Foo<Trait>` from a value of type `Foo<T>`, then
/// `trait_ref` would map `T:Trait`.
pub fn get_vtable<'a, 'tcx>(ccx: &CrateContext<'a, 'tcx>,
                            ty: ty::Ty<'tcx>,
                            trait_ref: Option<ty::PolyExistentialTraitRef<'tcx>>)
                            -> ValueRef
{
    let tcx = ccx.tcx();

    debug!("get_vtable(ty={:?}, trait_ref={:?})", ty, trait_ref);

    // Check the cache.
    if let Some(&val) = ccx.vtables().borrow().get(&(ty, trait_ref)) {
        return val;
    }

    // Not in the cache. Build it.
    let nullptr = C_null(Type::nil(ccx).ptr_to());

    let size_ty = sizing_type_of(ccx, ty);
    let size = machine::llsize_of_alloc(ccx, size_ty);
    let align = align_of(ccx, ty);

    let mut components: Vec<_> = [
        // Generate a destructor for the vtable.
        glue::get_drop_glue(ccx, ty),
        C_uint(ccx, size),
        C_uint(ccx, align)
    ].iter().cloned().collect();

    if let Some(trait_ref) = trait_ref {
        let trait_ref = trait_ref.with_self_ty(tcx, ty);
        let methods = traits::get_vtable_methods(tcx, trait_ref).map(|opt_mth| {
            opt_mth.map_or(nullptr, |(def_id, substs)| {
                callee::resolve_and_get_fn(ccx, def_id, substs)
            })
        });
        components.extend(methods);
    }

    let vtable_const = C_struct(ccx, &components, false);
    let align = machine::llalign_of_pref(ccx, val_ty(vtable_const));
    let vtable = consts::addr_of(ccx, vtable_const, align, "vtable");

    ccx.vtables().borrow_mut().insert((ty, trait_ref), vtable);
    vtable
}
