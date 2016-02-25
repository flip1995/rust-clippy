// Copyright 2012-2015 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
//! Set and unset common attributes on LLVM values.

use libc::{c_uint, c_ulonglong};
use llvm::{self, ValueRef};
use middle::ty;
use middle::infer;
use session::config::NoDebugInfo;
pub use syntax::attr::InlineAttr;
use syntax::ast;
use rustc_front::hir;
use trans::abi::Abi;
use trans::common;
use trans::context::CrateContext;
use trans::machine;
use trans::type_of;

/// Mark LLVM function to use provided inline heuristic.
#[inline]
pub fn inline(val: ValueRef, inline: InlineAttr) {
    use self::InlineAttr::*;
    match inline {
        Hint   => llvm::SetFunctionAttribute(val, llvm::Attribute::InlineHint),
        Always => llvm::SetFunctionAttribute(val, llvm::Attribute::AlwaysInline),
        Never  => llvm::SetFunctionAttribute(val, llvm::Attribute::NoInline),
        None   => {
            let attr = llvm::Attribute::InlineHint |
                       llvm::Attribute::AlwaysInline |
                       llvm::Attribute::NoInline;
            unsafe {
                llvm::LLVMRemoveFunctionAttr(val, attr.bits() as c_ulonglong)
            }
        },
    };
}

/// Tell LLVM to emit or not emit the information necessary to unwind the stack for the function.
#[inline]
pub fn emit_uwtable(val: ValueRef, emit: bool) {
    if emit {
        llvm::SetFunctionAttribute(val, llvm::Attribute::UWTable);
    } else {
        unsafe {
            llvm::LLVMRemoveFunctionAttr(
                val,
                llvm::Attribute::UWTable.bits() as c_ulonglong,
            );
        }
    }
}

/// Tell LLVM whether the function can or cannot unwind.
#[inline]
pub fn unwind(val: ValueRef, can_unwind: bool) {
    if can_unwind {
        unsafe {
            llvm::LLVMRemoveFunctionAttr(
                val,
                llvm::Attribute::NoUnwind.bits() as c_ulonglong,
            );
        }
    } else {
        llvm::SetFunctionAttribute(val, llvm::Attribute::NoUnwind);
    }
}

/// Tell LLVM whether it should optimise function for size.
#[inline]
#[allow(dead_code)] // possibly useful function
pub fn set_optimize_for_size(val: ValueRef, optimize: bool) {
    if optimize {
        llvm::SetFunctionAttribute(val, llvm::Attribute::OptimizeForSize);
    } else {
        unsafe {
            llvm::LLVMRemoveFunctionAttr(
                val,
                llvm::Attribute::OptimizeForSize.bits() as c_ulonglong,
            );
        }
    }
}

/// Composite function which sets LLVM attributes for function depending on its AST (#[attribute])
/// attributes.
pub fn from_fn_attrs(ccx: &CrateContext, attrs: &[ast::Attribute], llfn: ValueRef) {
    use syntax::attr::*;
    inline(llfn, find_inline_attr(Some(ccx.sess().diagnostic()), attrs));

    // FIXME: #11906: Omitting frame pointers breaks retrieving the value of a
    // parameter.
    let no_fp_elim = (ccx.sess().opts.debuginfo != NoDebugInfo) ||
                     !ccx.sess().target.target.options.eliminate_frame_pointer;
    if no_fp_elim {
        unsafe {
            let attr = "no-frame-pointer-elim\0".as_ptr() as *const _;
            let val = "true\0".as_ptr() as *const _;
            llvm::LLVMAddFunctionAttrStringValue(llfn,
                                                 llvm::FunctionIndex as c_uint,
                                                 attr, val);
        }
    }

    for attr in attrs {
        if attr.check_name("cold") {
            llvm::Attributes::default().set(llvm::Attribute::Cold)
                .apply_llfn(llvm::FunctionIndex as c_uint, llfn)
        } else if attr.check_name("allocator") {
            llvm::Attributes::default().set(llvm::Attribute::NoAlias)
                .apply_llfn(llvm::ReturnIndex as c_uint, llfn)
        } else if attr.check_name("unwind") {
            unwind(llfn, true);
        }
    }
}

/// Composite function which converts function type into LLVM attributes for the function.
pub fn from_fn_type<'a, 'tcx>(ccx: &CrateContext<'a, 'tcx>, fn_type: ty::Ty<'tcx>)
                              -> llvm::AttrBuilder {
    use middle::ty::{BrAnon, ReLateBound};

    let f = match fn_type.sty {
        ty::TyFnDef(_, _, f) | ty::TyFnPtr(f) => f,
        _ => unreachable!("expected fn type, found {:?}", fn_type)
    };

    let fn_sig = ccx.tcx().erase_late_bound_regions(&f.sig);
    let fn_sig = infer::normalize_associated_type(ccx.tcx(), &fn_sig);

    let mut attrs = llvm::AttrBuilder::new();
    let ret_ty = fn_sig.output;

    // These have an odd calling convention, so we need to manually
    // unpack the input ty's
    let input_tys = match fn_type.sty {
        ty::TyFnDef(..) | ty::TyFnPtr(_) if f.abi == Abi::RustCall => {
            let first = Some(fn_sig.inputs[0]).into_iter();

            match fn_sig.inputs[1].sty {
                ty::TyTuple(ref t_in) => {
                    first.chain(t_in.iter().cloned())
                }
                _ => ccx.sess().bug("expected tuple'd inputs")
            }
        }
        _ => None.into_iter().chain(fn_sig.inputs.iter().cloned())
    };

    // Index 0 is the return value of the llvm func, so we start at 1
    let mut idx = 1;
    if let ty::FnConverging(ret_ty) = ret_ty {
        // A function pointer is called without the declaration
        // available, so we have to apply any attributes with ABI
        // implications directly to the call instruction. Right now,
        // the only attribute we need to worry about is `sret`.
        if type_of::return_uses_outptr(ccx, ret_ty) {
            let llret_sz = machine::llsize_of_real(ccx, type_of::type_of(ccx, ret_ty));

            // The outptr can be noalias and nocapture because it's entirely
            // invisible to the program. We also know it's nonnull as well
            // as how many bytes we can dereference
            attrs.arg(1).set(llvm::Attribute::StructRet)
                        .set(llvm::Attribute::NoAlias)
                        .set(llvm::Attribute::NoCapture)
                        .set_dereferenceable(llret_sz);

            // Add one more since there's an outptr
            idx += 1;
        } else {
            // The `noalias` attribute on the return value is useful to a
            // function ptr caller.
            match ret_ty.sty {
                // `Box` pointer return values never alias because ownership
                // is transferred
                ty::TyBox(it) if common::type_is_sized(ccx.tcx(), it) => {
                    attrs.ret().set(llvm::Attribute::NoAlias);
                }
                _ => {}
            }

            // We can also mark the return value as `dereferenceable` in certain cases
            match ret_ty.sty {
                // These are not really pointers but pairs, (pointer, len)
                ty::TyRef(_, ty::TypeAndMut { ty: inner, .. })
                | ty::TyBox(inner) if common::type_is_sized(ccx.tcx(), inner) => {
                    let llret_sz = machine::llsize_of_real(ccx, type_of::type_of(ccx, inner));
                    attrs.ret().set_dereferenceable(llret_sz);
                }
                _ => {}
            }

            if let ty::TyBool = ret_ty.sty {
                attrs.ret().set(llvm::Attribute::ZExt);
            }
        }
    }

    for t in input_tys {
        match t.sty {
            _ if type_of::arg_is_indirect(ccx, t) => {
                let llarg_sz = machine::llsize_of_real(ccx, type_of::type_of(ccx, t));

                // For non-immediate arguments the callee gets its own copy of
                // the value on the stack, so there are no aliases. It's also
                // program-invisible so can't possibly capture
                attrs.arg(idx).set(llvm::Attribute::NoAlias)
                              .set(llvm::Attribute::NoCapture)
                              .set_dereferenceable(llarg_sz);
            }

            ty::TyBool => {
                attrs.arg(idx).set(llvm::Attribute::ZExt);
            }

            // `Box` pointer parameters never alias because ownership is transferred
            ty::TyBox(inner) => {
                attrs.arg(idx).set(llvm::Attribute::NoAlias);

                if common::type_is_sized(ccx.tcx(), inner) {
                    let llsz = machine::llsize_of_real(ccx, type_of::type_of(ccx, inner));
                    attrs.arg(idx).set_dereferenceable(llsz);
                } else {
                    attrs.arg(idx).set(llvm::Attribute::NonNull);
                    if inner.is_trait() {
                        attrs.arg(idx + 1).set(llvm::Attribute::NonNull);
                    }
                }
            }

            ty::TyRef(b, mt) => {
                // `&mut` pointer parameters never alias other parameters, or mutable global data
                //
                // `&T` where `T` contains no `UnsafeCell<U>` is immutable, and can be marked as
                // both `readonly` and `noalias`, as LLVM's definition of `noalias` is based solely
                // on memory dependencies rather than pointer equality
                let interior_unsafe = mt.ty.type_contents(ccx.tcx()).interior_unsafe();

                if mt.mutbl != hir::MutMutable && !interior_unsafe {
                    attrs.arg(idx).set(llvm::Attribute::NoAlias);
                }

                if mt.mutbl == hir::MutImmutable && !interior_unsafe {
                    attrs.arg(idx).set(llvm::Attribute::ReadOnly);
                }

                // & pointer parameters are also never null and for sized types we also know
                // exactly how many bytes we can dereference
                if common::type_is_sized(ccx.tcx(), mt.ty) {
                    let llsz = machine::llsize_of_real(ccx, type_of::type_of(ccx, mt.ty));
                    attrs.arg(idx).set_dereferenceable(llsz);
                } else {
                    attrs.arg(idx).set(llvm::Attribute::NonNull);
                    if mt.ty.is_trait() {
                        attrs.arg(idx + 1).set(llvm::Attribute::NonNull);
                    }
                }

                // When a reference in an argument has no named lifetime, it's
                // impossible for that reference to escape this function
                // (returned or stored beyond the call by a closure).
                if let ReLateBound(_, BrAnon(_)) = *b {
                    attrs.arg(idx).set(llvm::Attribute::NoCapture);
                }
            }

            _ => ()
        }

        if common::type_is_fat_ptr(ccx.tcx(), t) {
            idx += 2;
        } else {
            idx += 1;
        }
    }

    attrs
}
