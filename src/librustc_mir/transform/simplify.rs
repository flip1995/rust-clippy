//! A number of passes which remove various redundancies in the CFG.
//!
//! The `SimplifyCfg` pass gets rid of unnecessary blocks in the CFG, whereas the `SimplifyLocals`
//! gets rid of all the unnecessary local variable declarations.
//!
//! The `SimplifyLocals` pass is kinda expensive and therefore not very suitable to be run often.
//! Most of the passes should not care or be impacted in meaningful ways due to extra locals
//! either, so running the pass once, right before codegen, should suffice.
//!
//! On the other side of the spectrum, the `SimplifyCfg` pass is considerably cheap to run, thus
//! one should run it after every pass which may modify CFG in significant ways. This pass must
//! also be run before any analysis passes because it removes dead blocks, and some of these can be
//! ill-typed.
//!
//! The cause of this typing issue is typeck allowing most blocks whose end is not reachable have
//! an arbitrary return type, rather than having the usual () return type (as a note, typeck's
//! notion of reachability is in fact slightly weaker than MIR CFG reachability - see #31617). A
//! standard example of the situation is:
//!
//! ```rust
//!   fn example() {
//!       let _a: char = { return; };
//!   }
//! ```
//!
//! Here the block (`{ return; }`) has the return type `char`, rather than `()`, but the MIR we
//! naively generate still contains the `_a = ()` write in the unreachable block "after" the
//! return.

use crate::transform::{MirPass, MirSource};
use rustc_index::bit_set::BitSet;
use rustc_index::vec::{Idx, IndexVec};
use rustc_middle::mir::visit::{MutVisitor, MutatingUseContext, PlaceContext, Visitor};
use rustc_middle::mir::*;
use rustc_middle::ty::{self, TyCtxt};
use std::borrow::Cow;

pub struct SimplifyCfg {
    label: String,
}

impl SimplifyCfg {
    pub fn new(label: &str) -> Self {
        SimplifyCfg { label: format!("SimplifyCfg-{}", label) }
    }
}

pub fn simplify_cfg(body: &mut BodyAndCache<'_>) {
    CfgSimplifier::new(body).simplify();
    remove_dead_blocks(body);

    // FIXME: Should probably be moved into some kind of pass manager
    body.basic_blocks_mut().raw.shrink_to_fit();
}

impl<'tcx> MirPass<'tcx> for SimplifyCfg {
    fn name(&self) -> Cow<'_, str> {
        Cow::Borrowed(&self.label)
    }

    fn run_pass(&self, _tcx: TyCtxt<'tcx>, _src: MirSource<'tcx>, body: &mut BodyAndCache<'tcx>) {
        debug!("SimplifyCfg({:?}) - simplifying {:?}", self.label, body);
        simplify_cfg(body);
    }
}

pub struct CfgSimplifier<'a, 'tcx> {
    basic_blocks: &'a mut IndexVec<BasicBlock, BasicBlockData<'tcx>>,
    pred_count: IndexVec<BasicBlock, u32>,
}

impl<'a, 'tcx> CfgSimplifier<'a, 'tcx> {
    pub fn new(body: &'a mut BodyAndCache<'tcx>) -> Self {
        let mut pred_count = IndexVec::from_elem(0u32, body.basic_blocks());

        // we can't use mir.predecessors() here because that counts
        // dead blocks, which we don't want to.
        pred_count[START_BLOCK] = 1;

        for (_, data) in traversal::preorder(body) {
            if let Some(ref term) = data.terminator {
                for &tgt in term.successors() {
                    pred_count[tgt] += 1;
                }
            }
        }

        let basic_blocks = body.basic_blocks_mut();

        CfgSimplifier { basic_blocks, pred_count }
    }

    pub fn simplify(mut self) {
        self.strip_nops();

        let mut start = START_BLOCK;

        // Vec of the blocks that should be merged. We store the indices here, instead of the
        // statements itself to avoid moving the (relatively) large statements twice.
        // We do not push the statements directly into the target block (`bb`) as that is slower
        // due to additional reallocations
        let mut merged_blocks = Vec::new();
        loop {
            let mut changed = false;

            self.collapse_goto_chain(&mut start, &mut changed);

            for bb in self.basic_blocks.indices() {
                if self.pred_count[bb] == 0 {
                    continue;
                }

                debug!("simplifying {:?}", bb);

                let mut terminator =
                    self.basic_blocks[bb].terminator.take().expect("invalid terminator state");

                for successor in terminator.successors_mut() {
                    self.collapse_goto_chain(successor, &mut changed);
                }

                let mut inner_changed = true;
                merged_blocks.clear();
                while inner_changed {
                    inner_changed = false;
                    inner_changed |= self.simplify_branch(&mut terminator);
                    inner_changed |= self.merge_successor(&mut merged_blocks, &mut terminator);
                    changed |= inner_changed;
                }

                let statements_to_merge =
                    merged_blocks.iter().map(|&i| self.basic_blocks[i].statements.len()).sum();

                if statements_to_merge > 0 {
                    let mut statements = std::mem::take(&mut self.basic_blocks[bb].statements);
                    statements.reserve(statements_to_merge);
                    for &from in &merged_blocks {
                        statements.append(&mut self.basic_blocks[from].statements);
                    }
                    self.basic_blocks[bb].statements = statements;
                }

                self.basic_blocks[bb].terminator = Some(terminator);

                changed |= inner_changed;
            }

            if !changed {
                break;
            }
        }

        if start != START_BLOCK {
            debug_assert!(self.pred_count[START_BLOCK] == 0);
            self.basic_blocks.swap(START_BLOCK, start);
            self.pred_count.swap(START_BLOCK, start);

            // pred_count == 1 if the start block has no predecessor _blocks_.
            if self.pred_count[START_BLOCK] > 1 {
                for (bb, data) in self.basic_blocks.iter_enumerated_mut() {
                    if self.pred_count[bb] == 0 {
                        continue;
                    }

                    for target in data.terminator_mut().successors_mut() {
                        if *target == start {
                            *target = START_BLOCK;
                        }
                    }
                }
            }
        }
    }

    // Collapse a goto chain starting from `start`
    fn collapse_goto_chain(&mut self, start: &mut BasicBlock, changed: &mut bool) {
        let mut terminator = match self.basic_blocks[*start] {
            BasicBlockData {
                ref statements,
                terminator:
                    ref mut terminator @ Some(Terminator { kind: TerminatorKind::Goto { .. }, .. }),
                ..
            } if statements.is_empty() => terminator.take(),
            // if `terminator` is None, this means we are in a loop. In that
            // case, let all the loop collapse to its entry.
            _ => return,
        };

        let target = match terminator {
            Some(Terminator { kind: TerminatorKind::Goto { ref mut target }, .. }) => {
                self.collapse_goto_chain(target, changed);
                *target
            }
            _ => unreachable!(),
        };
        self.basic_blocks[*start].terminator = terminator;

        debug!("collapsing goto chain from {:?} to {:?}", *start, target);

        *changed |= *start != target;

        if self.pred_count[*start] == 1 {
            // This is the last reference to *start, so the pred-count to
            // to target is moved into the current block.
            self.pred_count[*start] = 0;
        } else {
            self.pred_count[target] += 1;
            self.pred_count[*start] -= 1;
        }

        *start = target;
    }

    // merge a block with 1 `goto` predecessor to its parent
    fn merge_successor(
        &mut self,
        merged_blocks: &mut Vec<BasicBlock>,
        terminator: &mut Terminator<'tcx>,
    ) -> bool {
        let target = match terminator.kind {
            TerminatorKind::Goto { target } if self.pred_count[target] == 1 => target,
            _ => return false,
        };

        debug!("merging block {:?} into {:?}", target, terminator);
        *terminator = match self.basic_blocks[target].terminator.take() {
            Some(terminator) => terminator,
            None => {
                // unreachable loop - this should not be possible, as we
                // don't strand blocks, but handle it correctly.
                return false;
            }
        };

        merged_blocks.push(target);
        self.pred_count[target] = 0;

        true
    }

    // turn a branch with all successors identical to a goto
    fn simplify_branch(&mut self, terminator: &mut Terminator<'tcx>) -> bool {
        match terminator.kind {
            TerminatorKind::SwitchInt { .. } => {}
            _ => return false,
        };

        let first_succ = {
            if let Some(&first_succ) = terminator.successors().next() {
                if terminator.successors().all(|s| *s == first_succ) {
                    let count = terminator.successors().count();
                    self.pred_count[first_succ] -= (count - 1) as u32;
                    first_succ
                } else {
                    return false;
                }
            } else {
                return false;
            }
        };

        debug!("simplifying branch {:?}", terminator);
        terminator.kind = TerminatorKind::Goto { target: first_succ };
        true
    }

    fn strip_nops(&mut self) {
        for blk in self.basic_blocks.iter_mut() {
            blk.statements
                .retain(|stmt| if let StatementKind::Nop = stmt.kind { false } else { true })
        }
    }
}

pub fn remove_dead_blocks(body: &mut BodyAndCache<'_>) {
    let mut seen = BitSet::new_empty(body.basic_blocks().len());
    for (bb, _) in traversal::preorder(body) {
        seen.insert(bb.index());
    }

    let basic_blocks = body.basic_blocks_mut();

    let num_blocks = basic_blocks.len();
    let mut replacements: Vec<_> = (0..num_blocks).map(BasicBlock::new).collect();
    let mut used_blocks = 0;
    for alive_index in seen.iter() {
        replacements[alive_index] = BasicBlock::new(used_blocks);
        if alive_index != used_blocks {
            // Swap the next alive block data with the current available slot. Since
            // alive_index is non-decreasing this is a valid operation.
            basic_blocks.raw.swap(alive_index, used_blocks);
        }
        used_blocks += 1;
    }
    basic_blocks.raw.truncate(used_blocks);

    for block in basic_blocks {
        for target in block.terminator_mut().successors_mut() {
            *target = replacements[target.index()];
        }
    }
}

pub struct SimplifyLocals;

impl<'tcx> MirPass<'tcx> for SimplifyLocals {
    fn run_pass(&self, tcx: TyCtxt<'tcx>, source: MirSource<'tcx>, body: &mut BodyAndCache<'tcx>) {
        trace!("running SimplifyLocals on {:?}", source);
        let locals = {
            let read_only_cache = read_only!(body);
            let mut marker = DeclMarker { locals: BitSet::new_empty(body.local_decls.len()), body };
            marker.visit_body(&read_only_cache);
            // Return pointer and arguments are always live
            marker.locals.insert(RETURN_PLACE);
            for arg in body.args_iter() {
                marker.locals.insert(arg);
            }

            marker.locals
        };

        let map = make_local_map(&mut body.local_decls, locals);
        // Update references to all vars and tmps now
        LocalUpdater { map, tcx }.visit_body(body);
        body.local_decls.shrink_to_fit();
    }
}

/// Construct the mapping while swapping out unused stuff out from the `vec`.
fn make_local_map<V>(
    vec: &mut IndexVec<Local, V>,
    mask: BitSet<Local>,
) -> IndexVec<Local, Option<Local>> {
    let mut map: IndexVec<Local, Option<Local>> = IndexVec::from_elem(None, &*vec);
    let mut used = Local::new(0);
    for alive_index in mask.iter() {
        map[alive_index] = Some(used);
        if alive_index != used {
            vec.swap(alive_index, used);
        }
        used.increment_by(1);
    }
    vec.truncate(used.index());
    map
}

struct DeclMarker<'a, 'tcx> {
    pub locals: BitSet<Local>,
    pub body: &'a Body<'tcx>,
}

impl<'a, 'tcx> Visitor<'tcx> for DeclMarker<'a, 'tcx> {
    fn visit_local(&mut self, local: &Local, ctx: PlaceContext, location: Location) {
        // Ignore storage markers altogether, they get removed along with their otherwise unused
        // decls.
        // FIXME: Extend this to all non-uses.
        if ctx.is_storage_marker() {
            return;
        }

        // Ignore stores of constants because `ConstProp` and `CopyProp` can remove uses of many
        // of these locals. However, if the local is still needed, then it will be referenced in
        // another place and we'll mark it as being used there.
        if ctx == PlaceContext::MutatingUse(MutatingUseContext::Store)
            || ctx == PlaceContext::MutatingUse(MutatingUseContext::Projection)
        {
            let block = &self.body.basic_blocks()[location.block];
            if location.statement_index != block.statements.len() {
                let stmt = &block.statements[location.statement_index];

                fn can_skip_constant(c: &ty::Const<'tcx>) -> bool {
                    // Keep assignments from unevaluated constants around, since the
                    // evaluation may report errors, even if the use of the constant
                    // is dead code.
                    !matches!(c.val, ty::ConstKind::Unevaluated(..))
                }

                fn can_skip_operand(o: &Operand<'_>) -> bool {
                    match o {
                        Operand::Copy(p) | Operand::Move(p) => !p.is_indirect(),
                        Operand::Constant(c) => can_skip_constant(c.literal),
                    }
                }

                if let StatementKind::Assign(box (dest, rvalue)) = &stmt.kind {
                    if !dest.is_indirect() && dest.local == *local {
                        let can_skip = match rvalue {
                            Rvalue::Use(op) => can_skip_operand(op),
                            Rvalue::Discriminant(_) => true,
                            Rvalue::BinaryOp(_, l, r) | Rvalue::CheckedBinaryOp(_, l, r) => {
                                can_skip_operand(l) && can_skip_operand(r)
                            }
                            Rvalue::Repeat(op, c) => can_skip_operand(op) && can_skip_constant(c),
                            Rvalue::AddressOf(_, _) => true,
                            Rvalue::Len(_) => true,
                            Rvalue::UnaryOp(_, op) => can_skip_operand(op),
                            Rvalue::Aggregate(_, operands) => operands.iter().all(can_skip_operand),

                            _ => false,
                        };

                        if can_skip {
                            trace!("skipping store of {:?} to {:?}", rvalue, dest);
                            return;
                        }
                    }
                }
            }
        }

        self.locals.insert(*local);
    }
}

struct LocalUpdater<'tcx> {
    map: IndexVec<Local, Option<Local>>,
    tcx: TyCtxt<'tcx>,
}

impl<'tcx> MutVisitor<'tcx> for LocalUpdater<'tcx> {
    fn tcx(&self) -> TyCtxt<'tcx> {
        self.tcx
    }

    fn visit_basic_block_data(&mut self, block: BasicBlock, data: &mut BasicBlockData<'tcx>) {
        // Remove unnecessary StorageLive and StorageDead annotations.
        data.statements.retain(|stmt| match &stmt.kind {
            StatementKind::StorageLive(l) | StatementKind::StorageDead(l) => self.map[*l].is_some(),
            StatementKind::Assign(box (place, _)) => self.map[place.local].is_some(),
            _ => true,
        });
        self.super_basic_block_data(block, data);
    }

    fn visit_local(&mut self, l: &mut Local, _: PlaceContext, _: Location) {
        *l = self.map[*l].unwrap();
    }
}
