// Copyright 2012-2017 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use syntax_pos::DUMMY_SP;

use rustc::mir::{self, Mir, Location};
use rustc::ty::{self, TyCtxt};
use util::elaborate_drops::DropFlagState;

use super::{MoveDataParamEnv};
use super::indexes::MovePathIndex;
use super::move_paths::{MoveData, LookupResult};

pub fn move_path_children_matching<'tcx, F>(move_data: &MoveData<'tcx>,
                                        path: MovePathIndex,
                                        mut cond: F)
                                        -> Option<MovePathIndex>
    where F: FnMut(&mir::LvalueProjection<'tcx>) -> bool
{
    let mut next_child = move_data.move_paths[path].first_child;
    while let Some(child_index) = next_child {
        match move_data.move_paths[child_index].lvalue {
            mir::Lvalue::Projection(ref proj) => {
                if cond(proj) {
                    return Some(child_index)
                }
            }
            _ => {}
        }
        next_child = move_data.move_paths[child_index].next_sibling;
    }

    None
}

/// When enumerating the child fragments of a path, don't recurse into
/// paths (1.) past arrays, slices, and pointers, nor (2.) into a type
/// that implements `Drop`.
///
/// Lvalues behind references or arrays are not tracked by elaboration
/// and are always assumed to be initialized when accessible. As
/// references and indexes can be reseated, trying to track them can
/// only lead to trouble.
///
/// Lvalues behind ADT's with a Drop impl are not tracked by
/// elaboration since they can never have a drop-flag state that
/// differs from that of the parent with the Drop impl.
///
/// In both cases, the contents can only be accessed if and only if
/// their parents are initialized. This implies for example that there
/// is no need to maintain separate drop flags to track such state.
///
/// FIXME: we have to do something for moving slice patterns.
fn lvalue_contents_drop_state_cannot_differ<'a, 'gcx, 'tcx>(tcx: TyCtxt<'a, 'gcx, 'tcx>,
                                                            mir: &Mir<'tcx>,
                                                            lv: &mir::Lvalue<'tcx>) -> bool {
    let ty = lv.ty(mir, tcx).to_ty(tcx);
    match ty.sty {
        ty::TyArray(..) | ty::TySlice(..) | ty::TyRef(..) | ty::TyRawPtr(..) => {
            debug!("lvalue_contents_drop_state_cannot_differ lv: {:?} ty: {:?} refd => true",
                   lv, ty);
            true
        }
        ty::TyAdt(def, _) if (def.has_dtor(tcx) && !def.is_box()) || def.is_union() => {
            debug!("lvalue_contents_drop_state_cannot_differ lv: {:?} ty: {:?} Drop => true",
                   lv, ty);
            true
        }
        _ => {
            false
        }
    }
}

pub(crate) fn on_lookup_result_bits<'a, 'gcx, 'tcx, F>(
    tcx: TyCtxt<'a, 'gcx, 'tcx>,
    mir: &Mir<'tcx>,
    move_data: &MoveData<'tcx>,
    lookup_result: LookupResult,
    each_child: F)
    where F: FnMut(MovePathIndex)
{
    match lookup_result {
        LookupResult::Parent(..) => {
            // access to untracked value - do not touch children
        }
        LookupResult::Exact(e) => {
            on_all_children_bits(tcx, mir, move_data, e, each_child)
        }
    }
}

pub(crate) fn on_all_children_bits<'a, 'gcx, 'tcx, F>(
    tcx: TyCtxt<'a, 'gcx, 'tcx>,
    mir: &Mir<'tcx>,
    move_data: &MoveData<'tcx>,
    move_path_index: MovePathIndex,
    mut each_child: F)
    where F: FnMut(MovePathIndex)
{
    fn is_terminal_path<'a, 'gcx, 'tcx>(
        tcx: TyCtxt<'a, 'gcx, 'tcx>,
        mir: &Mir<'tcx>,
        move_data: &MoveData<'tcx>,
        path: MovePathIndex) -> bool
    {
        lvalue_contents_drop_state_cannot_differ(
            tcx, mir, &move_data.move_paths[path].lvalue)
    }

    fn on_all_children_bits<'a, 'gcx, 'tcx, F>(
        tcx: TyCtxt<'a, 'gcx, 'tcx>,
        mir: &Mir<'tcx>,
        move_data: &MoveData<'tcx>,
        move_path_index: MovePathIndex,
        each_child: &mut F)
        where F: FnMut(MovePathIndex)
    {
        each_child(move_path_index);

        if is_terminal_path(tcx, mir, move_data, move_path_index) {
            return
        }

        let mut next_child_index = move_data.move_paths[move_path_index].first_child;
        while let Some(child_index) = next_child_index {
            on_all_children_bits(tcx, mir, move_data, child_index, each_child);
            next_child_index = move_data.move_paths[child_index].next_sibling;
        }
    }
    on_all_children_bits(tcx, mir, move_data, move_path_index, &mut each_child);
}

pub(crate) fn on_all_drop_children_bits<'a, 'gcx, 'tcx, F>(
    tcx: TyCtxt<'a, 'gcx, 'tcx>,
    mir: &Mir<'tcx>,
    ctxt: &MoveDataParamEnv<'gcx, 'tcx>,
    path: MovePathIndex,
    mut each_child: F)
    where F: FnMut(MovePathIndex)
{
    on_all_children_bits(tcx, mir, &ctxt.move_data, path, |child| {
        let lvalue = &ctxt.move_data.move_paths[path].lvalue;
        let ty = lvalue.ty(mir, tcx).to_ty(tcx);
        debug!("on_all_drop_children_bits({:?}, {:?} : {:?})", path, lvalue, ty);

        let gcx = tcx.global_tcx();
        let erased_ty = gcx.lift(&tcx.erase_regions(&ty)).unwrap();
        if erased_ty.needs_drop(gcx, ctxt.param_env) {
            each_child(child);
        } else {
            debug!("on_all_drop_children_bits - skipping")
        }
    })
}

pub(crate) fn drop_flag_effects_for_function_entry<'a, 'gcx, 'tcx, F>(
    tcx: TyCtxt<'a, 'gcx, 'tcx>,
    mir: &Mir<'tcx>,
    ctxt: &MoveDataParamEnv<'gcx, 'tcx>,
    mut callback: F)
    where F: FnMut(MovePathIndex, DropFlagState)
{
    let move_data = &ctxt.move_data;
    for arg in mir.args_iter() {
        let lvalue = mir::Lvalue::Local(arg);
        let lookup_result = move_data.rev_lookup.find(&lvalue);
        on_lookup_result_bits(tcx, mir, move_data,
                              lookup_result,
                              |mpi| callback(mpi, DropFlagState::Present));
    }
}

pub(crate) fn drop_flag_effects_for_location<'a, 'gcx, 'tcx, F>(
    tcx: TyCtxt<'a, 'gcx, 'tcx>,
    mir: &Mir<'tcx>,
    ctxt: &MoveDataParamEnv<'gcx, 'tcx>,
    loc: Location,
    mut callback: F)
    where F: FnMut(MovePathIndex, DropFlagState)
{
    let move_data = &ctxt.move_data;
    let param_env = ctxt.param_env;
    debug!("drop_flag_effects_for_location({:?})", loc);

    // first, move out of the RHS
    for mi in &move_data.loc_map[loc] {
        let path = mi.move_path_index(move_data);
        debug!("moving out of path {:?}", move_data.move_paths[path]);

        // don't move out of non-Copy things
        let lvalue = &move_data.move_paths[path].lvalue;
        let ty = lvalue.ty(mir, tcx).to_ty(tcx);
        let gcx = tcx.global_tcx();
        let erased_ty = gcx.lift(&tcx.erase_regions(&ty)).unwrap();
        if !erased_ty.moves_by_default(gcx, param_env, DUMMY_SP) {
            continue;
        }

        on_all_children_bits(tcx, mir, move_data,
                             path,
                             |mpi| callback(mpi, DropFlagState::Absent))
    }

    let block = &mir[loc.block];
    match block.statements.get(loc.statement_index) {
        Some(stmt) => match stmt.kind {
            mir::StatementKind::SetDiscriminant{ .. } => {
                span_bug!(stmt.source_info.span, "SetDiscrimant should not exist during borrowck");
            }
            mir::StatementKind::Assign(ref lvalue, ref rvalue) => {
                match rvalue.initialization_state() {
                    mir::tcx::RvalueInitializationState::Shallow => {
                        debug!("drop_flag_effects: box assignment {:?}", stmt);
                        if let LookupResult::Exact(mpi) = move_data.rev_lookup.find(lvalue) {
                            callback(mpi, DropFlagState::Present);
                        }
                    }
                    mir::tcx::RvalueInitializationState::Deep => {
                        debug!("drop_flag_effects: assignment {:?}", stmt);
                        on_lookup_result_bits(tcx, mir, move_data,
                                              move_data.rev_lookup.find(lvalue),
                                              |mpi| callback(mpi, DropFlagState::Present))
                    }
                }
            }
            mir::StatementKind::StorageDead(local) => {
                on_lookup_result_bits(tcx, mir, move_data,
                                      move_data.rev_lookup.find(&mir::Lvalue::Local(local)),
                                      |mpi| callback(mpi, DropFlagState::Absent))

            }
            mir::StatementKind::StorageLive(_) |
            mir::StatementKind::InlineAsm { .. } |
            mir::StatementKind::EndRegion(_) |
            mir::StatementKind::Validate(..) |
            mir::StatementKind::Nop => {}
        },
        None => {
            debug!("drop_flag_effects: replace {:?}", block.terminator());
            match block.terminator().kind {
                mir::TerminatorKind::DropAndReplace { ref location, .. } => {
                    on_lookup_result_bits(tcx, mir, move_data,
                                          move_data.rev_lookup.find(location),
                                          |mpi| callback(mpi, DropFlagState::Present))
                }
                _ => {
                    // other terminators do not contain move-ins
                }
            }
        }
    }
}
