// Copyright 2012-2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use llvm::{BasicBlockRef, ValueRef};
use rustc::middle::ty;
use rustc::mir::repr as mir;
use syntax::abi::Abi;
use trans::adt;
use trans::attributes;
use trans::base;
use trans::build;
use trans::common::{self, Block, LandingPad};
use trans::debuginfo::DebugLoc;
use trans::Disr;
use trans::foreign;
use trans::glue;
use trans::type_of;
use trans::type_::Type;

use super::MirContext;
use super::operand::OperandValue::{FatPtr, Immediate, Ref};

impl<'bcx, 'tcx> MirContext<'bcx, 'tcx> {
    pub fn trans_block(&mut self, bb: mir::BasicBlock) {
        debug!("trans_block({:?})", bb);

        let mut bcx = self.bcx(bb);
        let data = self.mir.basic_block_data(bb);

        for statement in &data.statements {
            bcx = self.trans_statement(bcx, statement);
        }

        debug!("trans_block: terminator: {:?}", data.terminator());

        match *data.terminator() {
            mir::Terminator::Goto { target } => {
                build::Br(bcx, self.llblock(target), DebugLoc::None)
            }

            mir::Terminator::If { ref cond, targets: (true_bb, false_bb) } => {
                let cond = self.trans_operand(bcx, cond);
                let lltrue = self.llblock(true_bb);
                let llfalse = self.llblock(false_bb);
                build::CondBr(bcx, cond.immediate(), lltrue, llfalse, DebugLoc::None);
            }

            mir::Terminator::Switch { ref discr, ref adt_def, ref targets } => {
                let discr_lvalue = self.trans_lvalue(bcx, discr);
                let ty = discr_lvalue.ty.to_ty(bcx.tcx());
                let repr = adt::represent_type(bcx.ccx(), ty);
                let discr = adt::trans_get_discr(bcx, &repr, discr_lvalue.llval,
                                                 None, true);

                // The else branch of the Switch can't be hit, so branch to an unreachable
                // instruction so LLVM knows that
                let unreachable_blk = self.unreachable_block();
                let switch = build::Switch(bcx, discr, unreachable_blk.llbb, targets.len());
                assert_eq!(adt_def.variants.len(), targets.len());
                for (adt_variant, target) in adt_def.variants.iter().zip(targets) {
                    let llval = adt::trans_case(bcx, &*repr, Disr::from(adt_variant.disr_val));
                    let llbb = self.llblock(*target);

                    build::AddCase(switch, llval, llbb)
                }
            }

            mir::Terminator::SwitchInt { ref discr, switch_ty, ref values, ref targets } => {
                let (otherwise, targets) = targets.split_last().unwrap();
                let discr = build::Load(bcx, self.trans_lvalue(bcx, discr).llval);
                let switch = build::Switch(bcx, discr, self.llblock(*otherwise), values.len());
                for (value, target) in values.iter().zip(targets) {
                    let llval = self.trans_constval(bcx, value, switch_ty).immediate();
                    let llbb = self.llblock(*target);
                    build::AddCase(switch, llval, llbb)
                }
            }

            mir::Terminator::Resume => {
                let ps = self.get_personality_slot(bcx);
                let lp = build::Load(bcx, ps);
                base::call_lifetime_end(bcx, ps);
                base::trans_unwind_resume(bcx, lp);
            }

            mir::Terminator::Return => {
                let return_ty = bcx.monomorphize(&self.mir.return_ty);
                base::build_return_block(bcx.fcx, bcx, return_ty, DebugLoc::None);
            }

            mir::Terminator::Drop { ref value, target, unwind } => {
                let lvalue = self.trans_lvalue(bcx, value);
                let ty = lvalue.ty.to_ty(bcx.tcx());
                // Double check for necessity to drop
                if !glue::type_needs_drop(bcx.tcx(), ty) {
                    build::Br(bcx, self.llblock(target), DebugLoc::None);
                    return;
                }
                let drop_fn = glue::get_drop_glue(bcx.ccx(), ty);
                let drop_ty = glue::get_drop_glue_type(bcx.ccx(), ty);
                let llvalue = if drop_ty != ty {
                    build::PointerCast(bcx, lvalue.llval,
                                       type_of::type_of(bcx.ccx(), drop_ty).ptr_to())
                } else {
                    lvalue.llval
                };
                if let Some(unwind) = unwind {
                    let uwbcx = self.bcx(unwind);
                    let unwind = self.make_landing_pad(uwbcx);
                    build::Invoke(bcx,
                                  drop_fn,
                                  &[llvalue],
                                  self.llblock(target),
                                  unwind.llbb,
                                  None,
                                  DebugLoc::None);
                } else {
                    build::Call(bcx, drop_fn, &[llvalue], None, DebugLoc::None);
                    build::Br(bcx, self.llblock(target), DebugLoc::None);
                }
            }

            mir::Terminator::Call { ref func, ref args, ref destination, ref cleanup } => {
                // Create the callee. This will always be a fn ptr and hence a kind of scalar.
                let callee = self.trans_operand(bcx, func);
                let attrs = attributes::from_fn_type(bcx.ccx(), callee.ty);
                let debugloc = DebugLoc::None;
                // The arguments we'll be passing. Plus one to account for outptr, if used.
                let mut llargs = Vec::with_capacity(args.len() + 1);
                // Types of the arguments. We do not preallocate, because this vector is only
                // filled when `is_foreign` is `true` and foreign calls are minority of the cases.
                let mut arg_tys = Vec::new();

                // Foreign-ABI functions are translated differently
                let is_foreign = if let ty::TyBareFn(_, ref f) = callee.ty.sty {
                    // We do not translate intrinsics here (they shouldn’t be functions)
                    assert!(f.abi != Abi::RustIntrinsic && f.abi != Abi::PlatformIntrinsic);
                    f.abi != Abi::Rust && f.abi != Abi::RustCall
                } else {
                    false
                };

                // Prepare the return value destination
                let (ret_dest_ty, must_copy_dest) = if let Some((ref d, _)) = *destination {
                    let dest = self.trans_lvalue(bcx, d);
                    let ret_ty = dest.ty.to_ty(bcx.tcx());
                    if !is_foreign && type_of::return_uses_outptr(bcx.ccx(), ret_ty) {
                        llargs.push(dest.llval);
                        (Some((dest, ret_ty)), false)
                    } else {
                        (Some((dest, ret_ty)), !common::type_is_zero_size(bcx.ccx(), ret_ty))
                    }
                } else {
                    (None, false)
                };

                // Process the rest of the args.
                for arg in args {
                    let operand = self.trans_operand(bcx, arg);
                    match operand.val {
                        Ref(llval) | Immediate(llval) => llargs.push(llval),
                        FatPtr(b, e) => {
                            llargs.push(b);
                            llargs.push(e);
                        }
                    }
                    if is_foreign {
                        arg_tys.push(operand.ty);
                    }
                }

                // Many different ways to call a function handled here
                match (is_foreign, base::avoid_invoke(bcx), cleanup, destination) {
                    // The two cases below are the only ones to use LLVM’s `invoke`.
                    (false, false, &Some(cleanup), &None) => {
                        let cleanup = self.bcx(cleanup);
                        let landingpad = self.make_landing_pad(cleanup);
                        let unreachable_blk = self.unreachable_block();
                        build::Invoke(bcx,
                                      callee.immediate(),
                                      &llargs[..],
                                      unreachable_blk.llbb,
                                      landingpad.llbb,
                                      Some(attrs),
                                      debugloc);
                    },
                    (false, false, &Some(cleanup), &Some((_, success))) => {
                        let cleanup = self.bcx(cleanup);
                        let landingpad = self.make_landing_pad(cleanup);
                        let (target, postinvoke) = if must_copy_dest {
                            (bcx.fcx.new_block("", None), Some(self.bcx(success)))
                        } else {
                            (self.bcx(success), None)
                        };
                        let invokeret = build::Invoke(bcx,
                                                      callee.immediate(),
                                                      &llargs[..],
                                                      target.llbb,
                                                      landingpad.llbb,
                                                      Some(attrs),
                                                      debugloc);
                        if let Some(postinvoketarget) = postinvoke {
                            // We translate the copy into a temoprary block. The temporary block is
                            // necessary because the current block has already been terminated (by
                            // `invoke`) and we cannot really translate into the target block
                            // because:
                            //  * The target block may have more than a single precedesor;
                            //  * Some LLVM insns cannot have a preceeding store insn (phi,
                            //    cleanuppad), and adding/prepending the store now may render
                            //    those other instructions invalid.
                            //
                            // NB: This approach still may break some LLVM code. For example if the
                            // target block starts with a `phi` (which may only match on immediate
                            // precedesors), it cannot know about this temporary block thus
                            // resulting in an invalid code:
                            //
                            // this:
                            //     …
                            //     %0 = …
                            //     %1 = invoke to label %temp …
                            // temp:
                            //     store ty %1, ty* %dest
                            //     br label %actualtargetblock
                            // actualtargetblock:            ; preds: %temp, …
                            //     phi … [%this, …], [%0, …] ; ERROR: phi requires to match only on
                            //                               ; immediate precedesors
                            let (ret_dest, ret_ty) = ret_dest_ty
                                .expect("return destination and type not set");
                            base::store_ty(target, invokeret, ret_dest.llval, ret_ty);
                            build::Br(target, postinvoketarget.llbb, debugloc);
                        }
                    },
                    (false, _, _, &None) => {
                        build::Call(bcx, callee.immediate(), &llargs[..], Some(attrs), debugloc);
                        build::Unreachable(bcx);
                    }
                    (false, _, _, &Some((_, target))) => {
                        let llret = build::Call(bcx,
                                                callee.immediate(),
                                                &llargs[..],
                                                Some(attrs),
                                                debugloc);
                        if must_copy_dest {
                            let (ret_dest, ret_ty) = ret_dest_ty
                                .expect("return destination and type not set");
                            base::store_ty(bcx, llret, ret_dest.llval, ret_ty);
                        }
                        build::Br(bcx, self.llblock(target), debugloc);
                    }
                    // Foreign functions
                    (true, _, _, destination) => {
                        let (dest, _) = ret_dest_ty
                            .expect("return destination is not set");
                        bcx = foreign::trans_native_call(bcx,
                                                   callee.ty,
                                                   callee.immediate(),
                                                   dest.llval,
                                                   &llargs[..],
                                                   arg_tys,
                                                   debugloc);
                        if let Some((_, target)) = *destination {
                            build::Br(bcx, self.llblock(target), debugloc);
                        }
                    },
                }
            }
        }
    }

    fn get_personality_slot(&mut self, bcx: Block<'bcx, 'tcx>) -> ValueRef {
        let ccx = bcx.ccx();
        if let Some(slot) = self.llpersonalityslot {
            slot
        } else {
            let llretty = Type::struct_(ccx, &[Type::i8p(ccx), Type::i32(ccx)], false);
            let slot = base::alloca(bcx, llretty, "personalityslot");
            self.llpersonalityslot = Some(slot);
            base::call_lifetime_start(bcx, slot);
            slot
        }
    }

    fn make_landing_pad(&mut self, cleanup: Block<'bcx, 'tcx>) -> Block<'bcx, 'tcx> {
        let bcx = cleanup.fcx.new_block("cleanup", None);
        // FIXME(#30941) this doesn't handle msvc-style exceptions
        *bcx.lpad.borrow_mut() = Some(LandingPad::gnu());
        let ccx = bcx.ccx();
        let llpersonality = bcx.fcx.eh_personality();
        let llretty = Type::struct_(ccx, &[Type::i8p(ccx), Type::i32(ccx)], false);
        let llretval = build::LandingPad(bcx, llretty, llpersonality, 1);
        build::SetCleanup(bcx, llretval);
        let slot = self.get_personality_slot(bcx);
        build::Store(bcx, llretval, slot);
        build::Br(bcx, cleanup.llbb, DebugLoc::None);
        bcx
    }

    fn unreachable_block(&mut self) -> Block<'bcx, 'tcx> {
        match self.unreachable_block {
            Some(b) => b,
            None => {
                let bl = self.fcx.new_block("unreachable", None);
                build::Unreachable(bl);
                self.unreachable_block = Some(bl);
                bl
            }
        }
    }

    fn bcx(&self, bb: mir::BasicBlock) -> Block<'bcx, 'tcx> {
        self.blocks[bb.index()]
    }

    fn llblock(&self, bb: mir::BasicBlock) -> BasicBlockRef {
        self.blocks[bb.index()].llbb
    }
}
