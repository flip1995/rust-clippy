//! This module contains the `EvalContext` methods for executing a single step of the interpreter.
//!
//! The main entry point is the `step` method.

use hir;
use mir::visit::{Visitor, PlaceContext};
use mir;
use ty::{self, Instance};
use ty::subst::Substs;
use middle::const_val::ConstVal;

use super::{EvalResult, EvalContext, StackPopCleanup, PtrAndAlign, GlobalId, Place,
            Machine, EvalErrorKind};

use syntax::codemap::Span;
use syntax::ast::Mutability;

impl<'a, 'tcx, M: Machine<'tcx>> EvalContext<'a, 'tcx, M> {
    pub fn inc_step_counter_and_check_limit(&mut self, n: u64) -> EvalResult<'tcx> {
        self.steps_remaining = self.steps_remaining.saturating_sub(n);
        if self.steps_remaining > 0 {
            Ok(())
        } else {
            err!(ExecutionTimeLimitReached)
        }
    }

    /// Returns true as long as there are more things to do.
    pub fn step(&mut self) -> EvalResult<'tcx, bool> {
        self.inc_step_counter_and_check_limit(1)?;
        if self.stack.is_empty() {
            return Ok(false);
        }

        let block = self.frame().block;
        let stmt_id = self.frame().stmt;
        let mir = self.mir();
        let basic_block = &mir.basic_blocks()[block];

        let old_frames = self.cur_frame();

        if let Some(stmt) = basic_block.statements.get(stmt_id) {
            let mut new = Ok(false);
            ConstantExtractor {
                span: stmt.source_info.span,
                instance: self.frame().instance,
                ecx: self,
                mir,
                new_constant: &mut new,
            }.visit_statement(
                block,
                stmt,
                mir::Location {
                    block,
                    statement_index: stmt_id,
                },
            );
            // if ConstantExtractor added a new frame, we don't execute anything here
            // but await the next call to step
            if !new? {
                assert_eq!(old_frames, self.cur_frame());
                self.statement(stmt)?;
            }
            return Ok(true);
        }

        let terminator = basic_block.terminator();
        let mut new = Ok(false);
        ConstantExtractor {
            span: terminator.source_info.span,
            instance: self.frame().instance,
            ecx: self,
            mir,
            new_constant: &mut new,
        }.visit_terminator(
            block,
            terminator,
            mir::Location {
                block,
                statement_index: stmt_id,
            },
        );
        // if ConstantExtractor added a new frame, we don't execute anything here
        // but await the next call to step
        if !new? {
            assert_eq!(old_frames, self.cur_frame());
            self.terminator(terminator)?;
        }
        Ok(true)
    }

    fn statement(&mut self, stmt: &mir::Statement<'tcx>) -> EvalResult<'tcx> {
        trace!("{:?}", stmt);

        use mir::StatementKind::*;

        // Some statements (e.g. box) push new stack frames.  We have to record the stack frame number
        // *before* executing the statement.
        let frame_idx = self.cur_frame();

        match stmt.kind {
            Assign(ref place, ref rvalue) => self.eval_rvalue_into_place(rvalue, place)?,

            SetDiscriminant {
                ref place,
                variant_index,
            } => {
                let dest = self.eval_place(place)?;
                let dest_ty = self.place_ty(place);
                self.write_discriminant_value(dest_ty, dest, variant_index)?;
            }

            // Mark locals as alive
            StorageLive(local) => {
                let old_val = self.frame_mut().storage_live(local)?;
                self.deallocate_local(old_val)?;
            }

            // Mark locals as dead
            StorageDead(local) => {
                let old_val = self.frame_mut().storage_dead(local)?;
                self.deallocate_local(old_val)?;
            }

            // Validity checks.
            Validate(op, ref places) => {
                for operand in places {
                    self.validation_op(op, operand)?;
                }
            }
            EndRegion(ce) => {
                self.end_region(Some(ce))?;
            }

            // Defined to do nothing. These are added by optimization passes, to avoid changing the
            // size of MIR constantly.
            Nop => {}

            InlineAsm { .. } => return err!(InlineAsm),
        }

        self.stack[frame_idx].stmt += 1;
        Ok(())
    }

    fn terminator(&mut self, terminator: &mir::Terminator<'tcx>) -> EvalResult<'tcx> {
        trace!("{:?}", terminator.kind);
        self.eval_terminator(terminator)?;
        if !self.stack.is_empty() {
            trace!("// {:?}", self.frame().block);
        }
        Ok(())
    }

    /// returns `true` if a stackframe was pushed
    fn global_item(
        &mut self,
        instance: Instance<'tcx>,
        span: Span,
        mutability: Mutability,
        orig_substs: &'tcx Substs<'tcx>,
    ) -> EvalResult<'tcx, bool> {
        debug!("global_item: {:?}", instance);
        let cid = GlobalId {
            instance,
            promoted: None,
        };
        if self.tcx.interpret_interner.borrow().get_cached(cid).is_some() {
            return Ok(false);
        }
        if self.tcx.has_attr(instance.def_id(), "linkage") {
            M::global_item_with_linkage(self, cid.instance, mutability)?;
            return Ok(false);
        }
        let mir = self.load_mir(instance.def)?;
        let layout = self.type_layout_with_substs(mir.return_ty(), orig_substs)?;
        assert!(!layout.is_unsized());
        let ptr = self.memory.allocate(
            layout.size.bytes(),
            layout.align.abi(),
            None,
        )?;
        self.tcx.interpret_interner.borrow_mut().cache(
            cid,
            PtrAndAlign {
                ptr: ptr.into(),
                aligned: !layout.is_packed(),
            },
        );
        let internally_mutable = !layout.ty.is_freeze(
            self.tcx,
            M::param_env(self),
            span,
        );
        let mutability = if mutability == Mutability::Mutable || internally_mutable {
            Mutability::Mutable
        } else {
            Mutability::Immutable
        };
        let cleanup = StackPopCleanup::MarkStatic(mutability);
        let name = ty::tls::with(|tcx| tcx.item_path_str(instance.def_id()));
        trace!("pushing stack frame for global: {}", name);
        self.push_stack_frame(
            instance,
            span,
            mir,
            Place::from_ptr(ptr),
            cleanup,
        )?;
        Ok(true)
    }
}

struct ConstantExtractor<'a, 'b: 'a, 'tcx: 'b, M: Machine<'tcx> + 'a> {
    span: Span,
    ecx: &'a mut EvalContext<'b, 'tcx, M>,
    mir: &'tcx mir::Mir<'tcx>,
    instance: ty::Instance<'tcx>,
    // Whether a stackframe for a new constant has been pushed
    new_constant: &'a mut EvalResult<'tcx, bool>,
}

impl<'a, 'b, 'tcx, M: Machine<'tcx>> ConstantExtractor<'a, 'b, 'tcx, M> {
    fn try<F: FnOnce(&mut Self) -> EvalResult<'tcx, bool>>(&mut self, f: F) {
        match *self.new_constant {
            // already computed a constant, don't do more than one per iteration
            Ok(true) => {},
            // no constants computed yet
            Ok(false) => *self.new_constant = f(self),
            // error happened, abort the visitor traversing
            Err(_) => {},
        }
    }
}

impl<'a, 'b, 'tcx, M: Machine<'tcx>> Visitor<'tcx> for ConstantExtractor<'a, 'b, 'tcx, M> {
    fn visit_constant(&mut self, constant: &mir::Constant<'tcx>, location: mir::Location) {
        self.super_constant(constant, location);
        self.try(|this| {
            match constant.literal {
                // already computed by rustc
                mir::Literal::Value { value: &ty::Const { val: ConstVal::Unevaluated(def_id, substs), .. } } => {
                    debug!("global_item: {:?}, {:#?}", def_id, substs);
                    let substs = this.ecx.tcx.trans_apply_param_substs(this.instance.substs, &substs);
                    debug!("global_item_new_substs: {:#?}", substs);
                    debug!("global_item_param_env: {:#?}", M::param_env(this.ecx));
                    let instance = Instance::resolve(
                        this.ecx.tcx,
                        M::param_env(this.ecx),
                        def_id,
                        substs,
                    ).ok_or(EvalErrorKind::TypeckError)?; // turn error prop into a panic to expose associated type in const issue
                    this.ecx.global_item(
                        instance,
                        constant.span,
                        Mutability::Immutable,
                        this.instance.substs,
                    )
                }
                mir::Literal::Value { .. } => Ok(false),
                mir::Literal::Promoted { index } => {
                    let cid = GlobalId {
                        instance: this.instance,
                        promoted: Some(index),
                    };
                    if this.ecx.tcx.interpret_interner.borrow().get_cached(cid).is_some() {
                        return Ok(false);
                    }
                    let mir = &this.mir.promoted[index];
                    let layout = this.ecx.type_layout_with_substs(
                        mir.return_ty(),
                        this.instance.substs)?;
                    assert!(!layout.is_unsized());
                    let ptr = this.ecx.memory.allocate(
                        layout.size.bytes(),
                        layout.align.abi(),
                        None,
                    )?;
                    this.ecx.tcx.interpret_interner.borrow_mut().cache(
                        cid,
                        PtrAndAlign {
                            ptr: ptr.into(),
                            aligned: !layout.is_packed(),
                        },
                    );
                    trace!("pushing stack frame for {:?}", index);
                    this.ecx.push_stack_frame(
                        this.instance,
                        constant.span,
                        mir,
                        Place::from_ptr(ptr),
                        StackPopCleanup::MarkStatic(Mutability::Immutable),
                    )?;
                    Ok(true)
                }
            }
        });
    }

    fn visit_place(
        &mut self,
        place: &mir::Place<'tcx>,
        context: PlaceContext<'tcx>,
        location: mir::Location,
    ) {
        self.super_place(place, context, location);
        self.try(|this| {
            if let mir::Place::Static(ref static_) = *place {
                let def_id = static_.def_id;
                let span = this.span;
                if let Some(node_item) = this.ecx.tcx.hir.get_if_local(def_id) {
                    if let hir::map::Node::NodeItem(&hir::Item { ref node, .. }) = node_item {
                        if let hir::ItemStatic(_, m, _) = *node {
                            let instance = Instance::mono(this.ecx.tcx, def_id);
                            this.ecx.global_item(
                                instance,
                                span,
                                if m == hir::MutMutable {
                                    Mutability::Mutable
                                } else {
                                    Mutability::Immutable
                                },
                                this.instance.substs,
                            )
                        } else {
                            bug!("static def id doesn't point to static");
                        }
                    } else {
                        bug!("static def id doesn't point to item");
                    }
                } else {
                    let def = this.ecx.tcx.describe_def(def_id).expect("static not found");
                    if let hir::def::Def::Static(_, mutable) = def {
                        let instance = Instance::mono(this.ecx.tcx, def_id);
                        this.ecx.global_item(
                            instance,
                            span,
                            if mutable {
                                Mutability::Mutable
                            } else {
                                Mutability::Immutable
                            },
                            this.instance.substs,
                        )
                    } else {
                        bug!("static found but isn't a static: {:?}", def);
                    }
                }
            } else {
                Ok(false)
            }
        });
    }
}
