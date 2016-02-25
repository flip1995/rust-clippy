// Copyright 2012-2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![allow(non_upper_case_globals)]

use arena::TypedArena;
use intrinsics::{self, Intrinsic};
use libc;
use llvm;
use llvm::{ValueRef, TypeKind};
use middle::infer;
use middle::subst;
use middle::subst::FnSpace;
use trans::abi::Abi;
use trans::adt;
use trans::attributes;
use trans::base::*;
use trans::build::*;
use trans::callee::{self, Callee};
use trans::cleanup;
use trans::cleanup::CleanupMethods;
use trans::common::*;
use trans::consts;
use trans::datum::*;
use trans::debuginfo::DebugLoc;
use trans::declare;
use trans::expr;
use trans::glue;
use trans::type_of;
use trans::machine;
use trans::type_::Type;
use middle::ty::{self, Ty, TypeFoldable};
use trans::Disr;
use middle::subst::Substs;
use rustc::dep_graph::DepNode;
use rustc_front::hir;
use syntax::ast;
use syntax::ptr::P;
use syntax::parse::token;

use rustc::lint;
use rustc::session::Session;
use syntax::codemap::Span;

use std::cmp::Ordering;

fn get_simple_intrinsic(ccx: &CrateContext, name: &str) -> Option<ValueRef> {
    let llvm_name = match name {
        "sqrtf32" => "llvm.sqrt.f32",
        "sqrtf64" => "llvm.sqrt.f64",
        "powif32" => "llvm.powi.f32",
        "powif64" => "llvm.powi.f64",
        "sinf32" => "llvm.sin.f32",
        "sinf64" => "llvm.sin.f64",
        "cosf32" => "llvm.cos.f32",
        "cosf64" => "llvm.cos.f64",
        "powf32" => "llvm.pow.f32",
        "powf64" => "llvm.pow.f64",
        "expf32" => "llvm.exp.f32",
        "expf64" => "llvm.exp.f64",
        "exp2f32" => "llvm.exp2.f32",
        "exp2f64" => "llvm.exp2.f64",
        "logf32" => "llvm.log.f32",
        "logf64" => "llvm.log.f64",
        "log10f32" => "llvm.log10.f32",
        "log10f64" => "llvm.log10.f64",
        "log2f32" => "llvm.log2.f32",
        "log2f64" => "llvm.log2.f64",
        "fmaf32" => "llvm.fma.f32",
        "fmaf64" => "llvm.fma.f64",
        "fabsf32" => "llvm.fabs.f32",
        "fabsf64" => "llvm.fabs.f64",
        "copysignf32" => "llvm.copysign.f32",
        "copysignf64" => "llvm.copysign.f64",
        "floorf32" => "llvm.floor.f32",
        "floorf64" => "llvm.floor.f64",
        "ceilf32" => "llvm.ceil.f32",
        "ceilf64" => "llvm.ceil.f64",
        "truncf32" => "llvm.trunc.f32",
        "truncf64" => "llvm.trunc.f64",
        "rintf32" => "llvm.rint.f32",
        "rintf64" => "llvm.rint.f64",
        "nearbyintf32" => "llvm.nearbyint.f32",
        "nearbyintf64" => "llvm.nearbyint.f64",
        "roundf32" => "llvm.round.f32",
        "roundf64" => "llvm.round.f64",
        "assume" => "llvm.assume",
        _ => return None
    };
    Some(ccx.get_intrinsic(&llvm_name))
}

pub fn span_transmute_size_error(a: &Session, b: Span, msg: &str) {
    span_err!(a, b, E0512, "{}", msg);
}

/// Performs late verification that intrinsics are used correctly. At present,
/// the only intrinsic that needs such verification is `transmute`.
pub fn check_intrinsics(ccx: &CrateContext) {
    let _task = ccx.tcx().dep_graph.in_task(DepNode::IntrinsicUseCheck);
    let mut last_failing_id = None;
    for transmute_restriction in ccx.tcx().transmute_restrictions.borrow().iter() {
        // Sometimes, a single call to transmute will push multiple
        // type pairs to test in order to exhaustively test the
        // possibility around a type parameter. If one of those fails,
        // there is no sense reporting errors on the others.
        if last_failing_id == Some(transmute_restriction.id) {
            continue;
        }

        debug!("transmute_restriction: {:?}", transmute_restriction);

        assert!(!transmute_restriction.substituted_from.has_param_types());
        assert!(!transmute_restriction.substituted_to.has_param_types());

        let llfromtype = type_of::sizing_type_of(ccx,
                                                 transmute_restriction.substituted_from);
        let lltotype = type_of::sizing_type_of(ccx,
                                               transmute_restriction.substituted_to);
        let from_type_size = machine::llbitsize_of_real(ccx, llfromtype);
        let to_type_size = machine::llbitsize_of_real(ccx, lltotype);

        if let ty::TyFnDef(..) = transmute_restriction.substituted_from.sty {
            if to_type_size == machine::llbitsize_of_real(ccx, ccx.int_type()) {
                // FIXME #19925 Remove this warning after a release cycle.
                lint::raw_emit_lint(&ccx.tcx().sess,
                                    &ccx.tcx().sess.lint_store.borrow(),
                                    lint::builtin::TRANSMUTE_FROM_FN_ITEM_TYPES,
                                    (lint::Warn, lint::LintSource::Default),
                                    Some(transmute_restriction.span),
                                    &format!("`{}` is now zero-sized and has to be cast \
                                              to a pointer before transmuting to `{}`",
                                             transmute_restriction.substituted_from,
                                             transmute_restriction.substituted_to));
                continue;
            }
        }
        if from_type_size != to_type_size {
            last_failing_id = Some(transmute_restriction.id);

            if transmute_restriction.original_from != transmute_restriction.substituted_from {
                span_transmute_size_error(ccx.sess(), transmute_restriction.span,
                    &format!("transmute called with differently sized types: \
                              {} (could be {} bits) to {} (could be {} bits)",
                             transmute_restriction.original_from,
                             from_type_size,
                             transmute_restriction.original_to,
                             to_type_size));
            } else {
                span_transmute_size_error(ccx.sess(), transmute_restriction.span,
                    &format!("transmute called with differently sized types: \
                              {} ({} bits) to {} ({} bits)",
                             transmute_restriction.original_from,
                             from_type_size,
                             transmute_restriction.original_to,
                             to_type_size));
            }
        }
    }
    ccx.sess().abort_if_errors();
}

/// Remember to add all intrinsics here, in librustc_typeck/check/mod.rs,
/// and in libcore/intrinsics.rs; if you need access to any llvm intrinsics,
/// add them to librustc_trans/trans/context.rs
pub fn trans_intrinsic_call<'a, 'blk, 'tcx>(mut bcx: Block<'blk, 'tcx>,
                                            callee_ty: Ty<'tcx>,
                                            cleanup_scope: cleanup::CustomScopeIndex,
                                            args: callee::CallArgs<'a, 'tcx>,
                                            dest: expr::Dest,
                                            call_info: NodeIdAndSpan)
                                            -> Result<'blk, 'tcx> {
    let fcx = bcx.fcx;
    let ccx = fcx.ccx;
    let tcx = bcx.tcx();

    let _icx = push_ctxt("trans_intrinsic_call");

    let (def_id, substs, sig) = match callee_ty.sty {
        ty::TyFnDef(def_id, substs, fty) => {
            let sig = tcx.erase_late_bound_regions(&fty.sig);
            (def_id, substs, infer::normalize_associated_type(tcx, &sig))
        }
        _ => unreachable!("expected fn item type, found {}", callee_ty)
    };
    let arg_tys = sig.inputs;
    let ret_ty = sig.output;
    let name = tcx.item_name(def_id).as_str();

    let call_debug_location = DebugLoc::At(call_info.id, call_info.span);

    // For `transmute` we can just trans the input expr directly into dest
    if name == "transmute" {
        let llret_ty = type_of::type_of(ccx, ret_ty.unwrap());
        match args {
            callee::ArgExprs(arg_exprs) => {
                assert_eq!(arg_exprs.len(), 1);

                let (in_type, out_type) = (*substs.types.get(FnSpace, 0),
                                           *substs.types.get(FnSpace, 1));
                let llintype = type_of::type_of(ccx, in_type);
                let llouttype = type_of::type_of(ccx, out_type);

                let in_type_size = machine::llbitsize_of_real(ccx, llintype);
                let out_type_size = machine::llbitsize_of_real(ccx, llouttype);

                if let ty::TyFnDef(def_id, substs, _) = in_type.sty {
                    if out_type_size != 0 {
                        // FIXME #19925 Remove this hack after a release cycle.
                        let _ = unpack_datum!(bcx, expr::trans(bcx, &arg_exprs[0]));
                        let llfn = Callee::def(ccx, def_id, substs).reify(ccx).val;
                        let llfnty = val_ty(llfn);
                        let llresult = match dest {
                            expr::SaveIn(d) => d,
                            expr::Ignore => alloc_ty(bcx, out_type, "ret")
                        };
                        Store(bcx, llfn, PointerCast(bcx, llresult, llfnty.ptr_to()));
                        if dest == expr::Ignore {
                            bcx = glue::drop_ty(bcx, llresult, out_type,
                                                call_debug_location);
                        }
                        fcx.scopes.borrow_mut().last_mut().unwrap().drop_non_lifetime_clean();
                        fcx.pop_and_trans_custom_cleanup_scope(bcx, cleanup_scope);
                        return Result::new(bcx, llresult);
                    }
                }

                // This should be caught by the intrinsicck pass
                assert_eq!(in_type_size, out_type_size);

                let nonpointer_nonaggregate = |llkind: TypeKind| -> bool {
                    use llvm::TypeKind::*;
                    match llkind {
                        Half | Float | Double | X86_FP80 | FP128 |
                            PPC_FP128 | Integer | Vector | X86_MMX => true,
                        _ => false
                    }
                };

                // An approximation to which types can be directly cast via
                // LLVM's bitcast.  This doesn't cover pointer -> pointer casts,
                // but does, importantly, cover SIMD types.
                let in_kind = llintype.kind();
                let ret_kind = llret_ty.kind();
                let bitcast_compatible =
                    (nonpointer_nonaggregate(in_kind) && nonpointer_nonaggregate(ret_kind)) || {
                        in_kind == TypeKind::Pointer && ret_kind == TypeKind::Pointer
                    };

                let dest = if bitcast_compatible {
                    // if we're here, the type is scalar-like (a primitive, a
                    // SIMD type or a pointer), and so can be handled as a
                    // by-value ValueRef and can also be directly bitcast to the
                    // target type.  Doing this special case makes conversions
                    // like `u32x4` -> `u64x2` much nicer for LLVM and so more
                    // efficient (these are done efficiently implicitly in C
                    // with the `__m128i` type and so this means Rust doesn't
                    // lose out there).
                    let expr = &arg_exprs[0];
                    let datum = unpack_datum!(bcx, expr::trans(bcx, expr));
                    let datum = unpack_datum!(bcx, datum.to_rvalue_datum(bcx, "transmute_temp"));
                    let val = if datum.kind.is_by_ref() {
                        load_ty(bcx, datum.val, datum.ty)
                    } else {
                        from_arg_ty(bcx, datum.val, datum.ty)
                    };

                    let cast_val = BitCast(bcx, val, llret_ty);

                    match dest {
                        expr::SaveIn(d) => {
                            // this often occurs in a sequence like `Store(val,
                            // d); val2 = Load(d)`, so disappears easily.
                            Store(bcx, cast_val, d);
                        }
                        expr::Ignore => {}
                    }
                    dest
                } else {
                    // The types are too complicated to do with a by-value
                    // bitcast, so pointer cast instead. We need to cast the
                    // dest so the types work out.
                    let dest = match dest {
                        expr::SaveIn(d) => expr::SaveIn(PointerCast(bcx, d, llintype.ptr_to())),
                        expr::Ignore => expr::Ignore
                    };
                    bcx = expr::trans_into(bcx, &arg_exprs[0], dest);
                    dest
                };

                fcx.scopes.borrow_mut().last_mut().unwrap().drop_non_lifetime_clean();
                fcx.pop_and_trans_custom_cleanup_scope(bcx, cleanup_scope);

                return match dest {
                    expr::SaveIn(d) => Result::new(bcx, d),
                    expr::Ignore => Result::new(bcx, C_undef(llret_ty.ptr_to()))
                };

            }

            _ => {
                ccx.sess().bug("expected expr as argument for transmute");
            }
        }
    }

    // For `move_val_init` we can evaluate the destination address
    // (the first argument) and then trans the source value (the
    // second argument) directly into the resulting destination
    // address.
    if name == "move_val_init" {
        if let callee::ArgExprs(ref exprs) = args {
            let (dest_expr, source_expr) = if exprs.len() != 2 {
                ccx.sess().bug("expected two exprs as arguments for `move_val_init` intrinsic");
            } else {
                (&exprs[0], &exprs[1])
            };

            // evaluate destination address
            let dest_datum = unpack_datum!(bcx, expr::trans(bcx, dest_expr));
            let dest_datum = unpack_datum!(
                bcx, dest_datum.to_rvalue_datum(bcx, "arg"));
            let dest_datum = unpack_datum!(
                bcx, dest_datum.to_appropriate_datum(bcx));

            // `expr::trans_into(bcx, expr, dest)` is equiv to
            //
            //    `trans(bcx, expr).store_to_dest(dest)`,
            //
            // which for `dest == expr::SaveIn(addr)`, is equivalent to:
            //
            //    `trans(bcx, expr).store_to(bcx, addr)`.
            let lldest = expr::Dest::SaveIn(dest_datum.val);
            bcx = expr::trans_into(bcx, source_expr, lldest);

            let llresult = C_nil(ccx);
            fcx.pop_and_trans_custom_cleanup_scope(bcx, cleanup_scope);

            return Result::new(bcx, llresult);
        } else {
            ccx.sess().bug("expected two exprs as arguments for `move_val_init` intrinsic");
        }
    }

    // For `try` we need some custom control flow
    if &name[..] == "try" {
        if let callee::ArgExprs(ref exprs) = args {
            let (func, data, local_ptr) = if exprs.len() != 3 {
                ccx.sess().bug("expected three exprs as arguments for \
                                `try` intrinsic");
            } else {
                (&exprs[0], &exprs[1], &exprs[2])
            };

            // translate arguments
            let func = unpack_datum!(bcx, expr::trans(bcx, func));
            let func = unpack_datum!(bcx, func.to_rvalue_datum(bcx, "func"));
            let data = unpack_datum!(bcx, expr::trans(bcx, data));
            let data = unpack_datum!(bcx, data.to_rvalue_datum(bcx, "data"));
            let local_ptr = unpack_datum!(bcx, expr::trans(bcx, local_ptr));
            let local_ptr = local_ptr.to_rvalue_datum(bcx, "local_ptr");
            let local_ptr = unpack_datum!(bcx, local_ptr);

            let dest = match dest {
                expr::SaveIn(d) => d,
                expr::Ignore => alloc_ty(bcx, tcx.mk_mut_ptr(tcx.types.i8),
                                         "try_result"),
            };

            // do the invoke
            bcx = try_intrinsic(bcx, func.val, data.val, local_ptr.val, dest,
                                call_debug_location);

            fcx.pop_and_trans_custom_cleanup_scope(bcx, cleanup_scope);
            return Result::new(bcx, dest);
        } else {
            ccx.sess().bug("expected two exprs as arguments for \
                            `try` intrinsic");
        }
    }

    // save the actual AST arguments for later (some places need to do
    // const-evaluation on them)
    let expr_arguments = match args {
        callee::ArgExprs(args) => Some(args),
        _ => None,
    };

    // Push the arguments.
    let mut llargs = Vec::new();
    bcx = callee::trans_args(bcx,
                             args,
                             callee_ty,
                             &mut llargs,
                             cleanup::CustomScope(cleanup_scope),
                             Abi::RustIntrinsic);

    fcx.scopes.borrow_mut().last_mut().unwrap().drop_non_lifetime_clean();

    // These are the only intrinsic functions that diverge.
    if name == "abort" {
        let llfn = ccx.get_intrinsic(&("llvm.trap"));
        Call(bcx, llfn, &[], call_debug_location);
        fcx.pop_and_trans_custom_cleanup_scope(bcx, cleanup_scope);
        Unreachable(bcx);
        return Result::new(bcx, C_undef(Type::nil(ccx).ptr_to()));
    } else if &name[..] == "unreachable" {
        fcx.pop_and_trans_custom_cleanup_scope(bcx, cleanup_scope);
        Unreachable(bcx);
        return Result::new(bcx, C_nil(ccx));
    }

    let ret_ty = match ret_ty {
        ty::FnConverging(ret_ty) => ret_ty,
        ty::FnDiverging => unreachable!()
    };

    let llret_ty = type_of::type_of(ccx, ret_ty);

    // Get location to store the result. If the user does
    // not care about the result, just make a stack slot
    let llresult = match dest {
        expr::SaveIn(d) => d,
        expr::Ignore => {
            if !type_is_zero_size(ccx, ret_ty) {
                let llresult = alloc_ty(bcx, ret_ty, "intrinsic_result");
                call_lifetime_start(bcx, llresult);
                llresult
            } else {
                C_undef(llret_ty.ptr_to())
            }
        }
    };

    let simple = get_simple_intrinsic(ccx, &name);
    let llval = match (simple, &name[..]) {
        (Some(llfn), _) => {
            Call(bcx, llfn, &llargs, call_debug_location)
        }
        (_, "breakpoint") => {
            let llfn = ccx.get_intrinsic(&("llvm.debugtrap"));
            Call(bcx, llfn, &[], call_debug_location)
        }
        (_, "size_of") => {
            let tp_ty = *substs.types.get(FnSpace, 0);
            let lltp_ty = type_of::type_of(ccx, tp_ty);
            C_uint(ccx, machine::llsize_of_alloc(ccx, lltp_ty))
        }
        (_, "size_of_val") => {
            let tp_ty = *substs.types.get(FnSpace, 0);
            if !type_is_sized(tcx, tp_ty) {
                let (llsize, _) = glue::size_and_align_of_dst(bcx, tp_ty, llargs[1]);
                llsize
            } else {
                let lltp_ty = type_of::type_of(ccx, tp_ty);
                C_uint(ccx, machine::llsize_of_alloc(ccx, lltp_ty))
            }
        }
        (_, "min_align_of") => {
            let tp_ty = *substs.types.get(FnSpace, 0);
            C_uint(ccx, type_of::align_of(ccx, tp_ty))
        }
        (_, "min_align_of_val") => {
            let tp_ty = *substs.types.get(FnSpace, 0);
            if !type_is_sized(tcx, tp_ty) {
                let (_, llalign) = glue::size_and_align_of_dst(bcx, tp_ty, llargs[1]);
                llalign
            } else {
                C_uint(ccx, type_of::align_of(ccx, tp_ty))
            }
        }
        (_, "pref_align_of") => {
            let tp_ty = *substs.types.get(FnSpace, 0);
            let lltp_ty = type_of::type_of(ccx, tp_ty);
            C_uint(ccx, machine::llalign_of_pref(ccx, lltp_ty))
        }
        (_, "drop_in_place") => {
            let tp_ty = *substs.types.get(FnSpace, 0);
            let ptr = if type_is_sized(tcx, tp_ty) {
                llargs[0]
            } else {
                let scratch = rvalue_scratch_datum(bcx, tp_ty, "tmp");
                Store(bcx, llargs[0], expr::get_dataptr(bcx, scratch.val));
                Store(bcx, llargs[1], expr::get_meta(bcx, scratch.val));
                fcx.schedule_lifetime_end(cleanup::CustomScope(cleanup_scope), scratch.val);
                scratch.val
            };
            glue::drop_ty(bcx, ptr, tp_ty, call_debug_location);
            C_nil(ccx)
        }
        (_, "type_name") => {
            let tp_ty = *substs.types.get(FnSpace, 0);
            let ty_name = token::intern_and_get_ident(&tp_ty.to_string());
            C_str_slice(ccx, ty_name)
        }
        (_, "type_id") => {
            let hash = ccx.tcx().hash_crate_independent(*substs.types.get(FnSpace, 0),
                                                        &ccx.link_meta().crate_hash);
            C_u64(ccx, hash)
        }
        (_, "init_dropped") => {
            let tp_ty = *substs.types.get(FnSpace, 0);
            if !return_type_is_void(ccx, tp_ty) {
                drop_done_fill_mem(bcx, llresult, tp_ty);
            }
            C_nil(ccx)
        }
        (_, "init") => {
            let tp_ty = *substs.types.get(FnSpace, 0);
            if !return_type_is_void(ccx, tp_ty) {
                // Just zero out the stack slot. (See comment on base::memzero for explanation)
                init_zero_mem(bcx, llresult, tp_ty);
            }
            C_nil(ccx)
        }
        // Effectively no-ops
        (_, "uninit") | (_, "forget") => {
            C_nil(ccx)
        }
        (_, "needs_drop") => {
            let tp_ty = *substs.types.get(FnSpace, 0);

            C_bool(ccx, bcx.fcx.type_needs_drop(tp_ty))
        }
        (_, "offset") => {
            let ptr = llargs[0];
            let offset = llargs[1];
            InBoundsGEP(bcx, ptr, &[offset])
        }
        (_, "arith_offset") => {
            let ptr = llargs[0];
            let offset = llargs[1];
            GEP(bcx, ptr, &[offset])
        }

        (_, "copy_nonoverlapping") => {
            copy_intrinsic(bcx,
                           false,
                           false,
                           *substs.types.get(FnSpace, 0),
                           llargs[1],
                           llargs[0],
                           llargs[2],
                           call_debug_location)
        }
        (_, "copy") => {
            copy_intrinsic(bcx,
                           true,
                           false,
                           *substs.types.get(FnSpace, 0),
                           llargs[1],
                           llargs[0],
                           llargs[2],
                           call_debug_location)
        }
        (_, "write_bytes") => {
            memset_intrinsic(bcx,
                             false,
                             *substs.types.get(FnSpace, 0),
                             llargs[0],
                             llargs[1],
                             llargs[2],
                             call_debug_location)
        }

        (_, "volatile_copy_nonoverlapping_memory") => {
            copy_intrinsic(bcx,
                           false,
                           true,
                           *substs.types.get(FnSpace, 0),
                           llargs[0],
                           llargs[1],
                           llargs[2],
                           call_debug_location)
        }
        (_, "volatile_copy_memory") => {
            copy_intrinsic(bcx,
                           true,
                           true,
                           *substs.types.get(FnSpace, 0),
                           llargs[0],
                           llargs[1],
                           llargs[2],
                           call_debug_location)
        }
        (_, "volatile_set_memory") => {
            memset_intrinsic(bcx,
                             true,
                             *substs.types.get(FnSpace, 0),
                             llargs[0],
                             llargs[1],
                             llargs[2],
                             call_debug_location)
        }
        (_, "volatile_load") => {
            let tp_ty = *substs.types.get(FnSpace, 0);
            let ptr = to_arg_ty_ptr(bcx, llargs[0], tp_ty);
            let load = VolatileLoad(bcx, ptr);
            unsafe {
                llvm::LLVMSetAlignment(load, type_of::align_of(ccx, tp_ty));
            }
            to_arg_ty(bcx, load, tp_ty)
        },
        (_, "volatile_store") => {
            let tp_ty = *substs.types.get(FnSpace, 0);
            let ptr = to_arg_ty_ptr(bcx, llargs[0], tp_ty);
            let val = if type_is_immediate(bcx.ccx(), tp_ty) {
                from_arg_ty(bcx, llargs[1], tp_ty)
            } else {
                Load(bcx, llargs[1])
            };
            let store = VolatileStore(bcx, val, ptr);
            unsafe {
                llvm::LLVMSetAlignment(store, type_of::align_of(ccx, tp_ty));
            }
            C_nil(ccx)
        },

        (_, "ctlz") | (_, "cttz") | (_, "ctpop") | (_, "bswap") |
        (_, "add_with_overflow") | (_, "sub_with_overflow") | (_, "mul_with_overflow") |
        (_, "overflowing_add") | (_, "overflowing_sub") | (_, "overflowing_mul") |
        (_, "unchecked_div") | (_, "unchecked_rem") => {
            let sty = &arg_tys[0].sty;
            match int_type_width_signed(sty, ccx) {
                Some((width, signed)) =>
                    match &*name {
                        "ctlz" => count_zeros_intrinsic(bcx, &format!("llvm.ctlz.i{}", width),
                                                        llargs[0], call_debug_location),
                        "cttz" => count_zeros_intrinsic(bcx, &format!("llvm.cttz.i{}", width),
                                                        llargs[0], call_debug_location),
                        "ctpop" => Call(bcx, ccx.get_intrinsic(&format!("llvm.ctpop.i{}", width)),
                                        &llargs, call_debug_location),
                        "bswap" => {
                            if width == 8 {
                                llargs[0] // byte swap a u8/i8 is just a no-op
                            } else {
                                Call(bcx, ccx.get_intrinsic(&format!("llvm.bswap.i{}", width)),
                                        &llargs, call_debug_location)
                            }
                        }
                        "add_with_overflow" | "sub_with_overflow" | "mul_with_overflow" => {
                            let intrinsic = format!("llvm.{}{}.with.overflow.i{}",
                                                    if signed { 's' } else { 'u' },
                                                    &name[..3], width);
                            with_overflow_intrinsic(bcx, &intrinsic, llargs[0], llargs[1], llresult,
                                                    call_debug_location)
                        },
                        "overflowing_add" => Add(bcx, llargs[0], llargs[1], call_debug_location),
                        "overflowing_sub" => Sub(bcx, llargs[0], llargs[1], call_debug_location),
                        "overflowing_mul" => Mul(bcx, llargs[0], llargs[1], call_debug_location),
                        "unchecked_div" =>
                            if signed {
                                SDiv(bcx, llargs[0], llargs[1], call_debug_location)
                            } else {
                                UDiv(bcx, llargs[0], llargs[1], call_debug_location)
                            },
                        "unchecked_rem" =>
                            if signed {
                                SRem(bcx, llargs[0], llargs[1], call_debug_location)
                            } else {
                                URem(bcx, llargs[0], llargs[1], call_debug_location)
                            },
                        _ => unreachable!(),
                    },
                None => {
                    span_invalid_monomorphization_error(
                        tcx.sess, call_info.span,
                        &format!("invalid monomorphization of `{}` intrinsic: \
                                  expected basic integer type, found `{}`", name, sty));
                        C_null(llret_ty)
                }
            }

        },


        (_, "return_address") => {
            if !fcx.caller_expects_out_pointer {
                span_err!(tcx.sess, call_info.span, E0510,
                          "invalid use of `return_address` intrinsic: function \
                           does not use out pointer");
                C_null(Type::i8p(ccx))
            } else {
                PointerCast(bcx, llvm::get_param(fcx.llfn, 0), Type::i8p(ccx))
            }
        }

        (_, "discriminant_value") => {
            let val_ty = substs.types.get(FnSpace, 0);
            match val_ty.sty {
                ty::TyEnum(..) => {
                    let repr = adt::represent_type(ccx, *val_ty);
                    adt::trans_get_discr(bcx, &repr, llargs[0],
                                         Some(llret_ty), true)
                }
                _ => C_null(llret_ty)
            }
        }
        (_, name) if name.starts_with("simd_") => {
            generic_simd_intrinsic(bcx, name,
                                   substs,
                                   callee_ty,
                                   expr_arguments,
                                   &llargs,
                                   ret_ty, llret_ty,
                                   call_debug_location,
                                   call_info)
        }
        // This requires that atomic intrinsics follow a specific naming pattern:
        // "atomic_<operation>[_<ordering>]", and no ordering means SeqCst
        (_, name) if name.starts_with("atomic_") => {
            let split: Vec<&str> = name.split('_').collect();

            let (order, failorder) = match split.len() {
                2 => (llvm::SequentiallyConsistent, llvm::SequentiallyConsistent),
                3 => match split[2] {
                    "unordered" => (llvm::Unordered, llvm::Unordered),
                    "relaxed" => (llvm::Monotonic, llvm::Monotonic),
                    "acq"     => (llvm::Acquire, llvm::Acquire),
                    "rel"     => (llvm::Release, llvm::Monotonic),
                    "acqrel"  => (llvm::AcquireRelease, llvm::Acquire),
                    "failrelaxed" if split[1] == "cxchg" || split[1] == "cxchgweak" =>
                        (llvm::SequentiallyConsistent, llvm::Monotonic),
                    "failacq" if split[1] == "cxchg" || split[1] == "cxchgweak" =>
                        (llvm::SequentiallyConsistent, llvm::Acquire),
                    _ => ccx.sess().fatal("unknown ordering in atomic intrinsic")
                },
                4 => match (split[2], split[3]) {
                    ("acq", "failrelaxed") if split[1] == "cxchg" || split[1] == "cxchgweak" =>
                        (llvm::Acquire, llvm::Monotonic),
                    ("acqrel", "failrelaxed") if split[1] == "cxchg" || split[1] == "cxchgweak" =>
                        (llvm::AcquireRelease, llvm::Monotonic),
                    _ => ccx.sess().fatal("unknown ordering in atomic intrinsic")
                },
                _ => ccx.sess().fatal("Atomic intrinsic not in correct format"),
            };

            match split[1] {
                "cxchg" => {
                    let tp_ty = *substs.types.get(FnSpace, 0);
                    let ptr = to_arg_ty_ptr(bcx, llargs[0], tp_ty);
                    let cmp = from_arg_ty(bcx, llargs[1], tp_ty);
                    let src = from_arg_ty(bcx, llargs[2], tp_ty);
                    let res = AtomicCmpXchg(bcx, ptr, cmp, src, order, failorder, llvm::False);
                    ExtractValue(bcx, res, 0)
                }

                "cxchgweak" => {
                    let tp_ty = *substs.types.get(FnSpace, 0);
                    let ptr = to_arg_ty_ptr(bcx, llargs[0], tp_ty);
                    let cmp = from_arg_ty(bcx, llargs[1], tp_ty);
                    let src = from_arg_ty(bcx, llargs[2], tp_ty);
                    let val = AtomicCmpXchg(bcx, ptr, cmp, src, order, failorder, llvm::True);
                    let result = ExtractValue(bcx, val, 0);
                    let success = ZExt(bcx, ExtractValue(bcx, val, 1), Type::bool(bcx.ccx()));
                    Store(bcx, result, StructGEP(bcx, llresult, 0));
                    Store(bcx, success, StructGEP(bcx, llresult, 1));
                    C_nil(ccx)
                }

                "load" => {
                    let tp_ty = *substs.types.get(FnSpace, 0);
                    let ptr = to_arg_ty_ptr(bcx, llargs[0], tp_ty);
                    to_arg_ty(bcx, AtomicLoad(bcx, ptr, order), tp_ty)
                }
                "store" => {
                    let tp_ty = *substs.types.get(FnSpace, 0);
                    let ptr = to_arg_ty_ptr(bcx, llargs[0], tp_ty);
                    let val = from_arg_ty(bcx, llargs[1], tp_ty);
                    AtomicStore(bcx, val, ptr, order);
                    C_nil(ccx)
                }

                "fence" => {
                    AtomicFence(bcx, order, llvm::CrossThread);
                    C_nil(ccx)
                }

                "singlethreadfence" => {
                    AtomicFence(bcx, order, llvm::SingleThread);
                    C_nil(ccx)
                }

                // These are all AtomicRMW ops
                op => {
                    let atom_op = match op {
                        "xchg"  => llvm::AtomicXchg,
                        "xadd"  => llvm::AtomicAdd,
                        "xsub"  => llvm::AtomicSub,
                        "and"   => llvm::AtomicAnd,
                        "nand"  => llvm::AtomicNand,
                        "or"    => llvm::AtomicOr,
                        "xor"   => llvm::AtomicXor,
                        "max"   => llvm::AtomicMax,
                        "min"   => llvm::AtomicMin,
                        "umax"  => llvm::AtomicUMax,
                        "umin"  => llvm::AtomicUMin,
                        _ => ccx.sess().fatal("unknown atomic operation")
                    };

                    let tp_ty = *substs.types.get(FnSpace, 0);
                    let ptr = to_arg_ty_ptr(bcx, llargs[0], tp_ty);
                    let val = from_arg_ty(bcx, llargs[1], tp_ty);
                    AtomicRMW(bcx, atom_op, ptr, val, order)
                }
            }

        }

        (_, _) => {
            let intr = match Intrinsic::find(tcx, &name) {
                Some(intr) => intr,
                None => unreachable!("unknown intrinsic '{}'", name),
            };
            fn one<T>(x: Vec<T>) -> T {
                assert_eq!(x.len(), 1);
                x.into_iter().next().unwrap()
            }
            fn ty_to_type(ccx: &CrateContext, t: &intrinsics::Type,
                          any_changes_needed: &mut bool) -> Vec<Type> {
                use intrinsics::Type::*;
                match *t {
                    Void => vec![Type::void(ccx)],
                    Integer(_signed, width, llvm_width) => {
                        *any_changes_needed |= width != llvm_width;
                        vec![Type::ix(ccx, llvm_width as u64)]
                    }
                    Float(x) => {
                        match x {
                            32 => vec![Type::f32(ccx)],
                            64 => vec![Type::f64(ccx)],
                            _ => unreachable!()
                        }
                    }
                    Pointer(ref t, ref llvm_elem, _const) => {
                        *any_changes_needed |= llvm_elem.is_some();

                        let t = llvm_elem.as_ref().unwrap_or(t);
                        let elem = one(ty_to_type(ccx, t,
                                                  any_changes_needed));
                        vec![elem.ptr_to()]
                    }
                    Vector(ref t, ref llvm_elem, length) => {
                        *any_changes_needed |= llvm_elem.is_some();

                        let t = llvm_elem.as_ref().unwrap_or(t);
                        let elem = one(ty_to_type(ccx, t,
                                                  any_changes_needed));
                        vec![Type::vector(&elem,
                                          length as u64)]
                    }
                    Aggregate(false, ref contents) => {
                        let elems = contents.iter()
                                            .map(|t| one(ty_to_type(ccx, t, any_changes_needed)))
                                            .collect::<Vec<_>>();
                        vec![Type::struct_(ccx, &elems, false)]
                    }
                    Aggregate(true, ref contents) => {
                        *any_changes_needed = true;
                        contents.iter()
                                .flat_map(|t| ty_to_type(ccx, t, any_changes_needed))
                                .collect()
                    }
                }
            }

            // This allows an argument list like `foo, (bar, baz),
            // qux` to be converted into `foo, bar, baz, qux`, integer
            // arguments to be truncated as needed and pointers to be
            // cast.
            fn modify_as_needed<'blk, 'tcx>(bcx: Block<'blk, 'tcx>,
                                            t: &intrinsics::Type,
                                            arg_type: Ty<'tcx>,
                                            llarg: ValueRef)
                                            -> Vec<ValueRef>
            {
                match *t {
                    intrinsics::Type::Aggregate(true, ref contents) => {
                        // We found a tuple that needs squishing! So
                        // run over the tuple and load each field.
                        //
                        // This assumes the type is "simple", i.e. no
                        // destructors, and the contents are SIMD
                        // etc.
                        assert!(!bcx.fcx.type_needs_drop(arg_type));

                        let repr = adt::represent_type(bcx.ccx(), arg_type);
                        let repr_ptr = &repr;
                        let arg = adt::MaybeSizedValue::sized(llarg);
                        (0..contents.len())
                            .map(|i| {
                                Load(bcx, adt::trans_field_ptr(bcx, repr_ptr, arg, Disr(0), i))
                            })
                            .collect()
                    }
                    intrinsics::Type::Pointer(_, Some(ref llvm_elem), _) => {
                        let llvm_elem = one(ty_to_type(bcx.ccx(), llvm_elem, &mut false));
                        vec![PointerCast(bcx, llarg,
                                         llvm_elem.ptr_to())]
                    }
                    intrinsics::Type::Vector(_, Some(ref llvm_elem), length) => {
                        let llvm_elem = one(ty_to_type(bcx.ccx(), llvm_elem, &mut false));
                        vec![BitCast(bcx, llarg,
                                     Type::vector(&llvm_elem, length as u64))]
                    }
                    intrinsics::Type::Integer(_, width, llvm_width) if width != llvm_width => {
                        // the LLVM intrinsic uses a smaller integer
                        // size than the C intrinsic's signature, so
                        // we have to trim it down here.
                        vec![Trunc(bcx, llarg, Type::ix(bcx.ccx(), llvm_width as u64))]
                    }
                    _ => vec![llarg],
                }
            }


            let mut any_changes_needed = false;
            let inputs = intr.inputs.iter()
                                    .flat_map(|t| ty_to_type(ccx, t, &mut any_changes_needed))
                                    .collect::<Vec<_>>();

            let mut out_changes = false;
            let outputs = one(ty_to_type(ccx, &intr.output, &mut out_changes));
            // outputting a flattened aggregate is nonsense
            assert!(!out_changes);

            let llargs = if !any_changes_needed {
                // no aggregates to flatten, so no change needed
                llargs
            } else {
                // there are some aggregates that need to be flattened
                // in the LLVM call, so we need to run over the types
                // again to find them and extract the arguments
                intr.inputs.iter()
                           .zip(&llargs)
                           .zip(&arg_tys)
                           .flat_map(|((t, llarg), ty)| modify_as_needed(bcx, t, ty, *llarg))
                           .collect()
            };
            assert_eq!(inputs.len(), llargs.len());

            let val = match intr.definition {
                intrinsics::IntrinsicDef::Named(name) => {
                    let f = declare::declare_cfn(ccx,
                                                 name,
                                                 Type::func(&inputs, &outputs));
                    Call(bcx, f, &llargs, call_debug_location)
                }
            };

            match *intr.output {
                intrinsics::Type::Aggregate(flatten, ref elems) => {
                    // the output is a tuple so we need to munge it properly
                    assert!(!flatten);

                    for i in 0..elems.len() {
                        let val = ExtractValue(bcx, val, i);
                        Store(bcx, val, StructGEP(bcx, llresult, i));
                    }
                    C_nil(ccx)
                }
                _ => val,
            }
        }
    };

    if val_ty(llval) != Type::void(ccx) &&
       machine::llsize_of_alloc(ccx, val_ty(llval)) != 0 {
        store_ty(bcx, llval, llresult, ret_ty);
    }

    // If we made a temporary stack slot, let's clean it up
    match dest {
        expr::Ignore => {
            bcx = glue::drop_ty(bcx, llresult, ret_ty, call_debug_location);
            call_lifetime_end(bcx, llresult);
        }
        expr::SaveIn(_) => {}
    }

    fcx.pop_and_trans_custom_cleanup_scope(bcx, cleanup_scope);

    Result::new(bcx, llresult)
}

fn copy_intrinsic<'blk, 'tcx>(bcx: Block<'blk, 'tcx>,
                              allow_overlap: bool,
                              volatile: bool,
                              tp_ty: Ty<'tcx>,
                              dst: ValueRef,
                              src: ValueRef,
                              count: ValueRef,
                              call_debug_location: DebugLoc)
                              -> ValueRef {
    let ccx = bcx.ccx();
    let lltp_ty = type_of::type_of(ccx, tp_ty);
    let align = C_i32(ccx, type_of::align_of(ccx, tp_ty) as i32);
    let size = machine::llsize_of(ccx, lltp_ty);
    let int_size = machine::llbitsize_of_real(ccx, ccx.int_type());

    let operation = if allow_overlap {
        "memmove"
    } else {
        "memcpy"
    };

    let name = format!("llvm.{}.p0i8.p0i8.i{}", operation, int_size);

    let dst_ptr = PointerCast(bcx, dst, Type::i8p(ccx));
    let src_ptr = PointerCast(bcx, src, Type::i8p(ccx));
    let llfn = ccx.get_intrinsic(&name);

    Call(bcx,
         llfn,
         &[dst_ptr,
           src_ptr,
           Mul(bcx, size, count, DebugLoc::None),
           align,
           C_bool(ccx, volatile)],
         call_debug_location)
}

fn memset_intrinsic<'blk, 'tcx>(bcx: Block<'blk, 'tcx>,
                                volatile: bool,
                                tp_ty: Ty<'tcx>,
                                dst: ValueRef,
                                val: ValueRef,
                                count: ValueRef,
                                call_debug_location: DebugLoc)
                                -> ValueRef {
    let ccx = bcx.ccx();
    let lltp_ty = type_of::type_of(ccx, tp_ty);
    let align = C_i32(ccx, type_of::align_of(ccx, tp_ty) as i32);
    let size = machine::llsize_of(ccx, lltp_ty);
    let int_size = machine::llbitsize_of_real(ccx, ccx.int_type());

    let name = format!("llvm.memset.p0i8.i{}", int_size);

    let dst_ptr = PointerCast(bcx, dst, Type::i8p(ccx));
    let llfn = ccx.get_intrinsic(&name);

    Call(bcx,
         llfn,
         &[dst_ptr,
           val,
           Mul(bcx, size, count, DebugLoc::None),
           align,
           C_bool(ccx, volatile)],
         call_debug_location)
}

fn count_zeros_intrinsic(bcx: Block,
                         name: &str,
                         val: ValueRef,
                         call_debug_location: DebugLoc)
                         -> ValueRef {
    let y = C_bool(bcx.ccx(), false);
    let llfn = bcx.ccx().get_intrinsic(&name);
    Call(bcx, llfn, &[val, y], call_debug_location)
}

fn with_overflow_intrinsic<'blk, 'tcx>(bcx: Block<'blk, 'tcx>,
                                       name: &str,
                                       a: ValueRef,
                                       b: ValueRef,
                                       out: ValueRef,
                                       call_debug_location: DebugLoc)
                                       -> ValueRef {
    let llfn = bcx.ccx().get_intrinsic(&name);

    // Convert `i1` to a `bool`, and write it to the out parameter
    let val = Call(bcx, llfn, &[a, b], call_debug_location);
    let result = ExtractValue(bcx, val, 0);
    let overflow = ZExt(bcx, ExtractValue(bcx, val, 1), Type::bool(bcx.ccx()));
    Store(bcx, result, StructGEP(bcx, out, 0));
    Store(bcx, overflow, StructGEP(bcx, out, 1));

    C_nil(bcx.ccx())
}

fn try_intrinsic<'blk, 'tcx>(bcx: Block<'blk, 'tcx>,
                             func: ValueRef,
                             data: ValueRef,
                             local_ptr: ValueRef,
                             dest: ValueRef,
                             dloc: DebugLoc) -> Block<'blk, 'tcx> {
    if bcx.sess().no_landing_pads() {
        Call(bcx, func, &[data], dloc);
        Store(bcx, C_null(Type::i8p(bcx.ccx())), dest);
        bcx
    } else if wants_msvc_seh(bcx.sess()) {
        trans_msvc_try(bcx, func, data, local_ptr, dest, dloc)
    } else {
        trans_gnu_try(bcx, func, data, local_ptr, dest, dloc)
    }
}

// MSVC's definition of the `rust_try` function.
//
// This implementation uses the new exception handling instructions in LLVM
// which have support in LLVM for SEH on MSVC targets. Although these
// instructions are meant to work for all targets, as of the time of this
// writing, however, LLVM does not recommend the usage of these new instructions
// as the old ones are still more optimized.
fn trans_msvc_try<'blk, 'tcx>(bcx: Block<'blk, 'tcx>,
                              func: ValueRef,
                              data: ValueRef,
                              local_ptr: ValueRef,
                              dest: ValueRef,
                              dloc: DebugLoc) -> Block<'blk, 'tcx> {
    let llfn = get_rust_try_fn(bcx.fcx, &mut |bcx| {
        let ccx = bcx.ccx();
        let dloc = DebugLoc::None;

        SetPersonalityFn(bcx, bcx.fcx.eh_personality());

        let normal = bcx.fcx.new_temp_block("normal");
        let catchswitch = bcx.fcx.new_temp_block("catchswitch");
        let catchpad = bcx.fcx.new_temp_block("catchpad");
        let caught = bcx.fcx.new_temp_block("caught");

        let func = llvm::get_param(bcx.fcx.llfn, 0);
        let data = llvm::get_param(bcx.fcx.llfn, 1);
        let local_ptr = llvm::get_param(bcx.fcx.llfn, 2);

        // We're generating an IR snippet that looks like:
        //
        //   declare i32 @rust_try(%func, %data, %ptr) {
        //      %slot = alloca i8*
        //      call @llvm.localescape(%slot)
        //      store %ptr, %slot
        //      invoke %func(%data) to label %normal unwind label %catchswitch
        //
        //   normal:
        //      ret i32 0
        //
        //   catchswitch:
        //      %cs = catchswitch within none [%catchpad] unwind to caller
        //
        //   catchpad:
        //      %tok = catchpad within %cs [%rust_try_filter]
        //      catchret from %tok to label %caught
        //
        //   caught:
        //      ret i32 1
        //   }
        //
        // This structure follows the basic usage of the instructions in LLVM
        // (see their documentation/test cases for examples), but a
        // perhaps-surprising part here is the usage of the `localescape`
        // intrinsic. This is used to allow the filter function (also generated
        // here) to access variables on the stack of this intrinsic. This
        // ability enables us to transfer information about the exception being
        // thrown to this point, where we're catching the exception.
        //
        // More information can be found in libstd's seh.rs implementation.
        let slot = Alloca(bcx, Type::i8p(ccx), "slot");
        let localescape = ccx.get_intrinsic(&"llvm.localescape");
        Call(bcx, localescape, &[slot], dloc);
        Store(bcx, local_ptr, slot);
        Invoke(bcx, func, &[data], normal.llbb, catchswitch.llbb, dloc);

        Ret(normal, C_i32(ccx, 0), dloc);

        let cs = CatchSwitch(catchswitch, None, None, 1);
        AddHandler(catchswitch, cs, catchpad.llbb);

        let filter = generate_filter_fn(bcx.fcx, bcx.fcx.llfn);
        let filter = BitCast(catchpad, filter, Type::i8p(ccx));
        let tok = CatchPad(catchpad, cs, &[filter]);
        CatchRet(catchpad, tok, caught.llbb);

        Ret(caught, C_i32(ccx, 1), dloc);
    });

    // Note that no invoke is used here because by definition this function
    // can't panic (that's what it's catching).
    let ret = Call(bcx, llfn, &[func, data, local_ptr], dloc);
    Store(bcx, ret, dest);
    return bcx
}

// Definition of the standard "try" function for Rust using the GNU-like model
// of exceptions (e.g. the normal semantics of LLVM's landingpad and invoke
// instructions).
//
// This translation is a little surprising because we always call a shim
// function instead of inlining the call to `invoke` manually here. This is done
// because in LLVM we're only allowed to have one personality per function
// definition. The call to the `try` intrinsic is being inlined into the
// function calling it, and that function may already have other personality
// functions in play. By calling a shim we're guaranteed that our shim will have
// the right personality function.
fn trans_gnu_try<'blk, 'tcx>(bcx: Block<'blk, 'tcx>,
                             func: ValueRef,
                             data: ValueRef,
                             local_ptr: ValueRef,
                             dest: ValueRef,
                             dloc: DebugLoc) -> Block<'blk, 'tcx> {
    let llfn = get_rust_try_fn(bcx.fcx, &mut |bcx| {
        let ccx = bcx.ccx();
        let tcx = ccx.tcx();
        let dloc = DebugLoc::None;

        // Translates the shims described above:
        //
        //   bcx:
        //      invoke %func(%args...) normal %normal unwind %catch
        //
        //   normal:
        //      ret 0
        //
        //   catch:
        //      (ptr, _) = landingpad
        //      store ptr, %local_ptr
        //      ret 1
        //
        // Note that the `local_ptr` data passed into the `try` intrinsic is
        // expected to be `*mut *mut u8` for this to actually work, but that's
        // managed by the standard library.

        attributes::emit_uwtable(bcx.fcx.llfn, true);
        let catch_pers = match tcx.lang_items.eh_personality_catch() {
            Some(did) => {
                Callee::def(ccx, did, tcx.mk_substs(Substs::empty())).reify(ccx).val
            }
            None => ccx.sess().bug("eh_personality_catch not defined"),
        };

        let then = bcx.fcx.new_temp_block("then");
        let catch = bcx.fcx.new_temp_block("catch");

        let func = llvm::get_param(bcx.fcx.llfn, 0);
        let data = llvm::get_param(bcx.fcx.llfn, 1);
        let local_ptr = llvm::get_param(bcx.fcx.llfn, 2);
        Invoke(bcx, func, &[data], then.llbb, catch.llbb, dloc);
        Ret(then, C_i32(ccx, 0), dloc);

        // Type indicator for the exception being thrown.
        //
        // The first value in this tuple is a pointer to the exception object
        // being thrown.  The second value is a "selector" indicating which of
        // the landing pad clauses the exception's type had been matched to.
        // rust_try ignores the selector.
        let lpad_ty = Type::struct_(ccx, &[Type::i8p(ccx), Type::i32(ccx)],
                                    false);
        let vals = LandingPad(catch, lpad_ty, catch_pers, 1);
        AddClause(catch, vals, C_null(Type::i8p(ccx)));
        let ptr = ExtractValue(catch, vals, 0);
        Store(catch, ptr, BitCast(catch, local_ptr, Type::i8p(ccx).ptr_to()));
        Ret(catch, C_i32(ccx, 1), dloc);
    });

    // Note that no invoke is used here because by definition this function
    // can't panic (that's what it's catching).
    let ret = Call(bcx, llfn, &[func, data, local_ptr], dloc);
    Store(bcx, ret, dest);
    return bcx;
}

// Helper function to give a Block to a closure to translate a shim function.
// This is currently primarily used for the `try` intrinsic functions above.
fn gen_fn<'a, 'tcx>(fcx: &FunctionContext<'a, 'tcx>,
                    name: &str,
                    ty: Ty<'tcx>,
                    output: ty::FnOutput<'tcx>,
                    trans: &mut for<'b> FnMut(Block<'b, 'tcx>))
                    -> ValueRef {
    let ccx = fcx.ccx;
    let llfn = declare::define_internal_fn(ccx, name, ty);
    let (fcx, block_arena);
    block_arena = TypedArena::new();
    fcx = new_fn_ctxt(ccx, llfn, ast::DUMMY_NODE_ID, false,
                      output, ccx.tcx().mk_substs(Substs::trans_empty()),
                      None, &block_arena);
    let bcx = init_function(&fcx, true, output);
    trans(bcx);
    fcx.cleanup();
    return llfn
}

// Helper function used to get a handle to the `__rust_try` function used to
// catch exceptions.
//
// This function is only generated once and is then cached.
fn get_rust_try_fn<'a, 'tcx>(fcx: &FunctionContext<'a, 'tcx>,
                             trans: &mut for<'b> FnMut(Block<'b, 'tcx>))
                             -> ValueRef {
    let ccx = fcx.ccx;
    if let Some(llfn) = ccx.rust_try_fn().get() {
        return llfn;
    }

    // Define the type up front for the signature of the rust_try function.
    let tcx = ccx.tcx();
    let i8p = tcx.mk_mut_ptr(tcx.types.i8);
    let fn_ty = tcx.mk_fn_ptr(ty::BareFnTy {
        unsafety: hir::Unsafety::Unsafe,
        abi: Abi::Rust,
        sig: ty::Binder(ty::FnSig {
            inputs: vec![i8p],
            output: ty::FnOutput::FnConverging(tcx.mk_nil()),
            variadic: false,
        }),
    });
    let output = ty::FnOutput::FnConverging(tcx.types.i32);
    let try_fn_ty  = ty::BareFnTy {
        unsafety: hir::Unsafety::Unsafe,
        abi: Abi::Rust,
        sig: ty::Binder(ty::FnSig {
            inputs: vec![fn_ty, i8p, i8p],
            output: output,
            variadic: false,
        }),
    };
    let rust_try = gen_fn(fcx, "__rust_try", tcx.mk_fn_ptr(try_fn_ty), output,
                          trans);
    ccx.rust_try_fn().set(Some(rust_try));
    return rust_try
}

// For MSVC-style exceptions (SEH), the compiler generates a filter function
// which is used to determine whether an exception is being caught (e.g. if it's
// a Rust exception or some other).
//
// This function is used to generate said filter function. The shim generated
// here is actually just a thin wrapper to call the real implementation in the
// standard library itself. For reasons as to why, see seh.rs in the standard
// library.
fn generate_filter_fn<'a, 'tcx>(fcx: &FunctionContext<'a, 'tcx>,
                                rust_try_fn: ValueRef)
                                -> ValueRef {
    let ccx = fcx.ccx;
    let tcx = ccx.tcx();
    let dloc = DebugLoc::None;

    let rust_try_filter = match tcx.lang_items.msvc_try_filter() {
        Some(did) => {
            Callee::def(ccx, did, tcx.mk_substs(Substs::empty())).reify(ccx).val
        }
        None => ccx.sess().bug("msvc_try_filter not defined"),
    };

    let output = ty::FnOutput::FnConverging(tcx.types.i32);
    let i8p = tcx.mk_mut_ptr(tcx.types.i8);

    let frameaddress = ccx.get_intrinsic(&"llvm.frameaddress");
    let recoverfp = ccx.get_intrinsic(&"llvm.x86.seh.recoverfp");
    let localrecover = ccx.get_intrinsic(&"llvm.localrecover");

    // On all platforms, once we have the EXCEPTION_POINTERS handle as well as
    // the base pointer, we follow the standard layout of:
    //
    //      block:
    //          %parentfp = call i8* llvm.x86.seh.recoverfp(@rust_try_fn, %bp)
    //          %arg = call i8* llvm.localrecover(@rust_try_fn, %parentfp, 0)
    //          %ret = call i32 @the_real_filter_function(%ehptrs, %arg)
    //          ret i32 %ret
    //
    // The recoverfp intrinsic is used to recover the frame frame pointer of the
    // `rust_try_fn` function, which is then in turn passed to the
    // `localrecover` intrinsic (pairing with the `localescape` intrinsic
    // mentioned above). Putting all this together means that we now have a
    // handle to the arguments passed into the `try` function, allowing writing
    // to the stack over there.
    //
    // For more info, see seh.rs in the standard library.
    let do_trans = |bcx: Block, ehptrs, base_pointer| {
        let rust_try_fn = BitCast(bcx, rust_try_fn, Type::i8p(ccx));
        let parentfp = Call(bcx, recoverfp, &[rust_try_fn, base_pointer], dloc);
        let arg = Call(bcx, localrecover,
                       &[rust_try_fn, parentfp, C_i32(ccx, 0)], dloc);
        let ret = Call(bcx, rust_try_filter, &[ehptrs, arg], dloc);
        Ret(bcx, ret, dloc);
    };

    if ccx.tcx().sess.target.target.arch == "x86" {
        // On x86 the filter function doesn't actually receive any arguments.
        // Instead the %ebp register contains some contextual information.
        //
        // Unfortunately I don't know of any great documentation as to what's
        // going on here, all I can say is that there's a few tests cases in
        // LLVM's test suite which follow this pattern of instructions, so we
        // just do the same.
        let filter_fn_ty = tcx.mk_fn_ptr(ty::BareFnTy {
            unsafety: hir::Unsafety::Unsafe,
            abi: Abi::Rust,
            sig: ty::Binder(ty::FnSig {
                inputs: vec![],
                output: output,
                variadic: false,
            }),
        });
        gen_fn(fcx, "__rustc_try_filter", filter_fn_ty, output, &mut |bcx| {
            let ebp = Call(bcx, frameaddress, &[C_i32(ccx, 1)], dloc);
            let exn = InBoundsGEP(bcx, ebp, &[C_i32(ccx, -20)]);
            let exn = Load(bcx, BitCast(bcx, exn, Type::i8p(ccx).ptr_to()));
            do_trans(bcx, exn, ebp);
        })
    } else if ccx.tcx().sess.target.target.arch == "x86_64" {
        // Conveniently on x86_64 the EXCEPTION_POINTERS handle and base pointer
        // are passed in as arguments to the filter function, so we just pass
        // those along.
        let filter_fn_ty = tcx.mk_fn_ptr(ty::BareFnTy {
            unsafety: hir::Unsafety::Unsafe,
            abi: Abi::Rust,
            sig: ty::Binder(ty::FnSig {
                inputs: vec![i8p, i8p],
                output: output,
                variadic: false,
            }),
        });
        gen_fn(fcx, "__rustc_try_filter", filter_fn_ty, output, &mut |bcx| {
            let exn = llvm::get_param(bcx.fcx.llfn, 0);
            let rbp = llvm::get_param(bcx.fcx.llfn, 1);
            do_trans(bcx, exn, rbp);
        })
    } else {
        panic!("unknown target to generate a filter function")
    }
}

fn span_invalid_monomorphization_error(a: &Session, b: Span, c: &str) {
    span_err!(a, b, E0511, "{}", c);
}

fn generic_simd_intrinsic<'blk, 'tcx, 'a>
    (bcx: Block<'blk, 'tcx>,
     name: &str,
     substs: &'tcx subst::Substs<'tcx>,
     callee_ty: Ty<'tcx>,
     args: Option<&[P<hir::Expr>]>,
     llargs: &[ValueRef],
     ret_ty: Ty<'tcx>,
     llret_ty: Type,
     call_debug_location: DebugLoc,
     call_info: NodeIdAndSpan) -> ValueRef
{
    // macros for error handling:
    macro_rules! emit_error {
        ($msg: tt) => {
            emit_error!($msg, )
        };
        ($msg: tt, $($fmt: tt)*) => {
            span_invalid_monomorphization_error(
                bcx.sess(), call_info.span,
                &format!(concat!("invalid monomorphization of `{}` intrinsic: ",
                                 $msg),
                         name, $($fmt)*));
        }
    }
    macro_rules! require {
        ($cond: expr, $($fmt: tt)*) => {
            if !$cond {
                emit_error!($($fmt)*);
                return C_null(llret_ty)
            }
        }
    }
    macro_rules! require_simd {
        ($ty: expr, $position: expr) => {
            require!($ty.is_simd(), "expected SIMD {} type, found non-SIMD `{}`", $position, $ty)
        }
    }



    let tcx = bcx.tcx();
    let sig = tcx.erase_late_bound_regions(callee_ty.fn_sig());
    let sig = infer::normalize_associated_type(tcx, &sig);
    let arg_tys = sig.inputs;

    // every intrinsic takes a SIMD vector as its first argument
    require_simd!(arg_tys[0], "input");
    let in_ty = arg_tys[0];
    let in_elem = arg_tys[0].simd_type(tcx);
    let in_len = arg_tys[0].simd_size(tcx);

    let comparison = match name {
        "simd_eq" => Some(hir::BiEq),
        "simd_ne" => Some(hir::BiNe),
        "simd_lt" => Some(hir::BiLt),
        "simd_le" => Some(hir::BiLe),
        "simd_gt" => Some(hir::BiGt),
        "simd_ge" => Some(hir::BiGe),
        _ => None
    };

    if let Some(cmp_op) = comparison {
        require_simd!(ret_ty, "return");

        let out_len = ret_ty.simd_size(tcx);
        require!(in_len == out_len,
                 "expected return type with length {} (same as input type `{}`), \
                  found `{}` with length {}",
                 in_len, in_ty,
                 ret_ty, out_len);
        require!(llret_ty.element_type().kind() == llvm::Integer,
                 "expected return type with integer elements, found `{}` with non-integer `{}`",
                 ret_ty,
                 ret_ty.simd_type(tcx));

        return compare_simd_types(bcx,
                                  llargs[0],
                                  llargs[1],
                                  in_elem,
                                  llret_ty,
                                  cmp_op,
                                  call_debug_location)
    }

    if name.starts_with("simd_shuffle") {
        let n: usize = match name["simd_shuffle".len()..].parse() {
            Ok(n) => n,
            Err(_) => tcx.sess.span_bug(call_info.span,
                                        "bad `simd_shuffle` instruction only caught in trans?")
        };

        require_simd!(ret_ty, "return");

        let out_len = ret_ty.simd_size(tcx);
        require!(out_len == n,
                 "expected return type of length {}, found `{}` with length {}",
                 n, ret_ty, out_len);
        require!(in_elem == ret_ty.simd_type(tcx),
                 "expected return element type `{}` (element of input `{}`), \
                  found `{}` with element type `{}`",
                 in_elem, in_ty,
                 ret_ty, ret_ty.simd_type(tcx));

        let total_len = in_len as u64 * 2;

        let vector = match args {
            Some(args) => &args[2],
            None => bcx.sess().span_bug(call_info.span,
                                        "intrinsic call with unexpected argument shape"),
        };
        let vector = match consts::const_expr(bcx.ccx(), vector, substs, None,
            consts::TrueConst::Yes, // this should probably help simd error reporting
        ) {
            Ok((vector, _)) => vector,
            Err(err) => bcx.sess().span_fatal(call_info.span, &err.description()),
        };

        let indices: Option<Vec<_>> = (0..n)
            .map(|i| {
                let arg_idx = i;
                let val = const_get_elt(vector, &[i as libc::c_uint]);
                let c = const_to_opt_uint(val);
                match c {
                    None => {
                        emit_error!("shuffle index #{} is not a constant", arg_idx);
                        None
                    }
                    Some(idx) if idx >= total_len => {
                        emit_error!("shuffle index #{} is out of bounds (limit {})",
                                    arg_idx, total_len);
                        None
                    }
                    Some(idx) => Some(C_i32(bcx.ccx(), idx as i32)),
                }
            })
            .collect();
        let indices = match indices {
            Some(i) => i,
            None => return C_null(llret_ty)
        };

        return ShuffleVector(bcx, llargs[0], llargs[1], C_vector(&indices))
    }

    if name == "simd_insert" {
        require!(in_elem == arg_tys[2],
                 "expected inserted type `{}` (element of input `{}`), found `{}`",
                 in_elem, in_ty, arg_tys[2]);
        return InsertElement(bcx, llargs[0], llargs[2], llargs[1])
    }
    if name == "simd_extract" {
        require!(ret_ty == in_elem,
                 "expected return type `{}` (element of input `{}`), found `{}`",
                 in_elem, in_ty, ret_ty);
        return ExtractElement(bcx, llargs[0], llargs[1])
    }

    if name == "simd_cast" {
        require_simd!(ret_ty, "return");
        let out_len = ret_ty.simd_size(tcx);
        require!(in_len == out_len,
                 "expected return type with length {} (same as input type `{}`), \
                  found `{}` with length {}",
                 in_len, in_ty,
                 ret_ty, out_len);
        // casting cares about nominal type, not just structural type
        let out_elem = ret_ty.simd_type(tcx);

        if in_elem == out_elem { return llargs[0]; }

        enum Style { Float, Int(/* is signed? */ bool), Unsupported }

        let (in_style, in_width) = match in_elem.sty {
            // vectors of pointer-sized integers should've been
            // disallowed before here, so this unwrap is safe.
            ty::TyInt(i) => (Style::Int(true), i.bit_width().unwrap()),
            ty::TyUint(u) => (Style::Int(false), u.bit_width().unwrap()),
            ty::TyFloat(f) => (Style::Float, f.bit_width()),
            _ => (Style::Unsupported, 0)
        };
        let (out_style, out_width) = match out_elem.sty {
            ty::TyInt(i) => (Style::Int(true), i.bit_width().unwrap()),
            ty::TyUint(u) => (Style::Int(false), u.bit_width().unwrap()),
            ty::TyFloat(f) => (Style::Float, f.bit_width()),
            _ => (Style::Unsupported, 0)
        };

        match (in_style, out_style) {
            (Style::Int(in_is_signed), Style::Int(_)) => {
                return match in_width.cmp(&out_width) {
                    Ordering::Greater => Trunc(bcx, llargs[0], llret_ty),
                    Ordering::Equal => llargs[0],
                    Ordering::Less => if in_is_signed {
                        SExt(bcx, llargs[0], llret_ty)
                    } else {
                        ZExt(bcx, llargs[0], llret_ty)
                    }
                }
            }
            (Style::Int(in_is_signed), Style::Float) => {
                return if in_is_signed {
                    SIToFP(bcx, llargs[0], llret_ty)
                } else {
                    UIToFP(bcx, llargs[0], llret_ty)
                }
            }
            (Style::Float, Style::Int(out_is_signed)) => {
                return if out_is_signed {
                    FPToSI(bcx, llargs[0], llret_ty)
                } else {
                    FPToUI(bcx, llargs[0], llret_ty)
                }
            }
            (Style::Float, Style::Float) => {
                return match in_width.cmp(&out_width) {
                    Ordering::Greater => FPTrunc(bcx, llargs[0], llret_ty),
                    Ordering::Equal => llargs[0],
                    Ordering::Less => FPExt(bcx, llargs[0], llret_ty)
                }
            }
            _ => {/* Unsupported. Fallthrough. */}
        }
        require!(false,
                 "unsupported cast from `{}` with element `{}` to `{}` with element `{}`",
                 in_ty, in_elem,
                 ret_ty, out_elem);
    }
    macro_rules! arith {
        ($($name: ident: $($($p: ident),* => $call: expr),*;)*) => {
            $(
                if name == stringify!($name) {
                    match in_elem.sty {
                        $(
                            $(ty::$p(_))|* => {
                                return $call(bcx, llargs[0], llargs[1], call_debug_location)
                            }
                            )*
                        _ => {},
                    }
                    require!(false,
                             "unsupported operation on `{}` with element `{}`",
                             in_ty,
                             in_elem)
                })*
        }
    }
    arith! {
        simd_add: TyUint, TyInt => Add, TyFloat => FAdd;
        simd_sub: TyUint, TyInt => Sub, TyFloat => FSub;
        simd_mul: TyUint, TyInt => Mul, TyFloat => FMul;
        simd_div: TyFloat => FDiv;
        simd_shl: TyUint, TyInt => Shl;
        simd_shr: TyUint => LShr, TyInt => AShr;
        simd_and: TyUint, TyInt => And;
        simd_or: TyUint, TyInt => Or;
        simd_xor: TyUint, TyInt => Xor;
    }
    bcx.sess().span_bug(call_info.span, "unknown SIMD intrinsic");
}

// Returns the width of an int TypeVariant, and if it's signed or not
// Returns None if the type is not an integer
fn int_type_width_signed<'tcx>(sty: &ty::TypeVariants<'tcx>, ccx: &CrateContext)
        -> Option<(u64, bool)> {
    use rustc::middle::ty::{TyInt, TyUint};
    match *sty {
        TyInt(t) => Some((match t {
            ast::IntTy::Is => {
                match &ccx.tcx().sess.target.target.target_pointer_width[..] {
                    "32" => 32,
                    "64" => 64,
                    tws => panic!("Unsupported target word size for isize: {}", tws),
                }
            },
            ast::IntTy::I8 => 8,
            ast::IntTy::I16 => 16,
            ast::IntTy::I32 => 32,
            ast::IntTy::I64 => 64,
        }, true)),
        TyUint(t) => Some((match t {
            ast::UintTy::Us => {
                match &ccx.tcx().sess.target.target.target_pointer_width[..] {
                    "32" => 32,
                    "64" => 64,
                    tws => panic!("Unsupported target word size for usize: {}", tws),
                }
            },
            ast::UintTy::U8 => 8,
            ast::UintTy::U16 => 16,
            ast::UintTy::U32 => 32,
            ast::UintTy::U64 => 64,
        }, false)),
        _ => None,
    }
}
