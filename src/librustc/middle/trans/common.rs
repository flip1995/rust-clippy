// Copyright 2012-2013 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Code that is useful in various trans modules.

use core::prelude::*;

use driver::session;
use driver::session::Session;
use lib::llvm::{ValueRef, BasicBlockRef, BuilderRef};
use lib::llvm::{True, False, Bool};
use lib::llvm::{llvm};
use lib;
use middle::trans::base;
use middle::trans::build;
use middle::trans::datum;
use middle::trans::glue;
use middle::trans::write_guard;
use middle::ty::substs;
use middle::ty;
use middle::typeck;
use middle::borrowck::root_map_key;
use util::ppaux::{Repr};

use middle::trans::type_::Type;

use core::cast::transmute;
use core::cast;
use core::hashmap::{HashMap};
use core::libc::{c_uint, c_longlong, c_ulonglong};
use core::to_bytes;
use core::vec;
use syntax::ast::ident;
use syntax::ast_map::{path, path_elt};
use syntax::codemap::span;
use syntax::parse::token;
use syntax::{ast, ast_map};

pub use middle::trans::context::CrateContext;

pub fn gensym_name(name: &str) -> ident {
    token::str_to_ident(fmt!("%s_%u", name, token::gensym(name)))
}

pub struct tydesc_info {
    ty: ty::t,
    tydesc: ValueRef,
    size: ValueRef,
    align: ValueRef,
    take_glue: Option<ValueRef>,
    drop_glue: Option<ValueRef>,
    free_glue: Option<ValueRef>,
    visit_glue: Option<ValueRef>
}

/*
 * A note on nomenclature of linking: "extern", "foreign", and "upcall".
 *
 * An "extern" is an LLVM symbol we wind up emitting an undefined external
 * reference to. This means "we don't have the thing in this compilation unit,
 * please make sure you link it in at runtime". This could be a reference to
 * C code found in a C library, or rust code found in a rust crate.
 *
 * Most "externs" are implicitly declared (automatically) as a result of a
 * user declaring an extern _module_ dependency; this causes the rust driver
 * to locate an extern crate, scan its compilation metadata, and emit extern
 * declarations for any symbols used by the declaring crate.
 *
 * A "foreign" is an extern that references C (or other non-rust ABI) code.
 * There is no metadata to scan for extern references so in these cases either
 * a header-digester like bindgen, or manual function prototypes, have to
 * serve as declarators. So these are usually given explicitly as prototype
 * declarations, in rust code, with ABI attributes on them noting which ABI to
 * link via.
 *
 * An "upcall" is a foreign call generated by the compiler (not corresponding
 * to any user-written call in the code) into the runtime library, to perform
 * some helper task such as bringing a task to life, allocating memory, etc.
 *
 */

pub struct Stats {
    n_static_tydescs: uint,
    n_glues_created: uint,
    n_null_glues: uint,
    n_real_glues: uint,
    n_fns: uint,
    n_monos: uint,
    n_inlines: uint,
    n_closures: uint,
    llvm_insns: HashMap<~str, uint>,
    fn_times: ~[(~str, int)] // (ident, time)
}

pub struct BuilderRef_res {
    B: BuilderRef,
}

impl Drop for BuilderRef_res {
    fn drop(&self) {
        unsafe {
            llvm::LLVMDisposeBuilder(self.B);
        }
    }
}

pub fn BuilderRef_res(B: BuilderRef) -> BuilderRef_res {
    BuilderRef_res {
        B: B
    }
}

pub type ExternMap = HashMap<@str, ValueRef>;

// Types used for llself.
pub struct ValSelfData {
    v: ValueRef,
    t: ty::t,
    is_owned: bool
}

// Here `self_ty` is the real type of the self parameter to this method. It
// will only be set in the case of default methods.
pub struct param_substs {
    tys: ~[ty::t],
    vtables: Option<typeck::vtable_res>,
    type_param_defs: @~[ty::TypeParameterDef],
    self_ty: Option<ty::t>,
    self_vtable: Option<typeck::vtable_origin>
}

impl param_substs {
    pub fn validate(&self) {
        for self.tys.iter().advance |t| { assert!(!ty::type_needs_infer(*t)); }
        for self.self_ty.iter().advance |t| { assert!(!ty::type_needs_infer(*t)); }
    }
}

fn param_substs_to_str(this: &param_substs, tcx: ty::ctxt) -> ~str {
    fmt!("param_substs {tys:%s, vtables:%s, type_param_defs:%s}",
         this.tys.repr(tcx),
         this.vtables.repr(tcx),
         this.type_param_defs.repr(tcx))
}

impl Repr for param_substs {
    fn repr(&self, tcx: ty::ctxt) -> ~str {
        param_substs_to_str(self, tcx)
    }
}

impl Repr for @param_substs {
    fn repr(&self, tcx: ty::ctxt) -> ~str {
        param_substs_to_str(*self, tcx)
    }
}

// Function context.  Every LLVM function we create will have one of
// these.
pub struct fn_ctxt_ {
    // The ValueRef returned from a call to llvm::LLVMAddFunction; the
    // address of the first instruction in the sequence of
    // instructions for this function that will go in the .text
    // section of the executable we're generating.
    llfn: ValueRef,

    // The implicit environment argument that arrives in the function we're
    // creating.
    llenv: ValueRef,

    // The place to store the return value. If the return type is immediate,
    // this is an alloca in the function. Otherwise, it's the hidden first
    // parameter to the function. After function construction, this should
    // always be Some.
    llretptr: Option<ValueRef>,

    // These elements: "hoisted basic blocks" containing
    // administrative activities that have to happen in only one place in
    // the function, due to LLVM's quirks.
    // A block for all the function's static allocas, so that LLVM
    // will coalesce them into a single alloca call.
    llstaticallocas: BasicBlockRef,
    // A block containing code that copies incoming arguments to space
    // already allocated by code in one of the llallocas blocks.
    // (LLVM requires that arguments be copied to local allocas before
    // allowing most any operation to be performed on them.)
    llloadenv: Option<BasicBlockRef>,
    llreturn: BasicBlockRef,
    // The 'self' value currently in use in this function, if there
    // is one.
    //
    // NB: This is the type of the self *variable*, not the self *type*. The
    // self type is set only for default methods, while the self variable is
    // set for all methods.
    llself: Option<ValSelfData>,
    // The a value alloca'd for calls to upcalls.rust_personality. Used when
    // outputting the resume instruction.
    personality: Option<ValueRef>,
    // If this is a for-loop body that returns, this holds the pointers needed
    // for that (flagptr, retptr)
    loop_ret: Option<(ValueRef, ValueRef)>,

    // True if this function has an immediate return value, false otherwise.
    // If this is false, the llretptr will alias the first argument of the
    // function.
    has_immediate_return_value: bool,

    // Maps arguments to allocas created for them in llallocas.
    llargs: @mut HashMap<ast::node_id, ValueRef>,
    // Maps the def_ids for local variables to the allocas created for
    // them in llallocas.
    lllocals: @mut HashMap<ast::node_id, ValueRef>,
    // Same as above, but for closure upvars
    llupvars: @mut HashMap<ast::node_id, ValueRef>,

    // The node_id of the function, or -1 if it doesn't correspond to
    // a user-defined function.
    id: ast::node_id,

    // The def_id of the impl we're inside, or None if we aren't inside one.
    impl_id: Option<ast::def_id>,

    // If this function is being monomorphized, this contains the type
    // substitutions used.
    param_substs: Option<@param_substs>,

    // The source span and nesting context where this function comes from, for
    // error reporting and symbol generation.
    span: Option<span>,
    path: path,

    // This function's enclosing crate context.
    ccx: @mut CrateContext
}

impl fn_ctxt_ {
    pub fn arg_pos(&self, arg: uint) -> uint {
        if self.has_immediate_return_value {
            arg + 1u
        } else {
            arg + 2u
        }
    }

    pub fn out_arg_pos(&self) -> uint {
        assert!(self.has_immediate_return_value);
        0u
    }

    pub fn env_arg_pos(&self) -> uint {
        if !self.has_immediate_return_value {
            1u
        } else {
            0u
        }
    }

}

pub type fn_ctxt = @mut fn_ctxt_;

pub fn warn_not_to_commit(ccx: &mut CrateContext, msg: &str) {
    if !ccx.do_not_commit_warning_issued {
        ccx.do_not_commit_warning_issued = true;
        ccx.sess.warn(msg.to_str() + " -- do not commit like this!");
    }
}

// Heap selectors. Indicate which heap something should go on.
#[deriving(Eq)]
pub enum heap {
    heap_managed,
    heap_managed_unique,
    heap_exchange,
}

#[deriving(Eq)]
pub enum cleantype {
    normal_exit_only,
    normal_exit_and_unwind
}

pub enum cleanup {
    clean(@fn(block) -> block, cleantype),
    clean_temp(ValueRef, @fn(block) -> block, cleantype),
}

// Used to remember and reuse existing cleanup paths
// target: none means the path ends in an resume instruction
pub struct cleanup_path {
    target: Option<BasicBlockRef>,
    size: uint,
    dest: BasicBlockRef
}

pub fn shrink_scope_clean(scope_info: &mut scope_info, size: uint) {
    scope_info.landing_pad = None;
    scope_info.cleanup_paths = scope_info.cleanup_paths.iter()
            .take_while(|&cu| cu.size <= size).transform(|&x|x).collect();
}

pub fn grow_scope_clean(scope_info: &mut scope_info) {
    scope_info.landing_pad = None;
}

pub fn cleanup_type(cx: ty::ctxt, ty: ty::t) -> cleantype {
    if ty::type_needs_unwind_cleanup(cx, ty) {
        normal_exit_and_unwind
    } else {
        normal_exit_only
    }
}

pub fn add_clean(bcx: block, val: ValueRef, t: ty::t) {
    if !ty::type_needs_drop(bcx.tcx(), t) { return; }

    debug!("add_clean(%s, %s, %s)", bcx.to_str(), bcx.val_to_str(val), t.repr(bcx.tcx()));

    let cleanup_type = cleanup_type(bcx.tcx(), t);
    do in_scope_cx(bcx) |scope_info| {
        scope_info.cleanups.push(clean(|a| glue::drop_ty(a, val, t), cleanup_type));
        grow_scope_clean(scope_info);
    }
}

pub fn add_clean_temp_immediate(cx: block, val: ValueRef, ty: ty::t) {
    if !ty::type_needs_drop(cx.tcx(), ty) { return; }
    debug!("add_clean_temp_immediate(%s, %s, %s)",
           cx.to_str(), cx.val_to_str(val),
           ty.repr(cx.tcx()));
    let cleanup_type = cleanup_type(cx.tcx(), ty);
    do in_scope_cx(cx) |scope_info| {
        scope_info.cleanups.push(
            clean_temp(val, |a| glue::drop_ty_immediate(a, val, ty),
                       cleanup_type));
        grow_scope_clean(scope_info);
    }
}
pub fn add_clean_temp_mem(bcx: block, val: ValueRef, t: ty::t) {
    if !ty::type_needs_drop(bcx.tcx(), t) { return; }
    debug!("add_clean_temp_mem(%s, %s, %s)",
           bcx.to_str(), bcx.val_to_str(val),
           t.repr(bcx.tcx()));
    let cleanup_type = cleanup_type(bcx.tcx(), t);
    do in_scope_cx(bcx) |scope_info| {
        scope_info.cleanups.push(clean_temp(val, |a| glue::drop_ty(a, val, t), cleanup_type));
        grow_scope_clean(scope_info);
    }
}
pub fn add_clean_return_to_mut(bcx: block,
                               root_key: root_map_key,
                               frozen_val_ref: ValueRef,
                               bits_val_ref: ValueRef,
                               filename_val: ValueRef,
                               line_val: ValueRef) {
    //! When an `@mut` has been frozen, we have to
    //! call the lang-item `return_to_mut` when the
    //! freeze goes out of scope. We need to pass
    //! in both the value which was frozen (`frozen_val`) and
    //! the value (`bits_val_ref`) which was returned when the
    //! box was frozen initially. Here, both `frozen_val_ref` and
    //! `bits_val_ref` are in fact pointers to stack slots.

    debug!("add_clean_return_to_mut(%s, %s, %s)",
           bcx.to_str(),
           bcx.val_to_str(frozen_val_ref),
           bcx.val_to_str(bits_val_ref));
    do in_scope_cx(bcx) |scope_info| {
        scope_info.cleanups.push(
            clean_temp(
                frozen_val_ref,
                |bcx| write_guard::return_to_mut(bcx, root_key, frozen_val_ref, bits_val_ref,
                                                 filename_val, line_val),
                normal_exit_only));
        grow_scope_clean(scope_info);
    }
}
pub fn add_clean_free(cx: block, ptr: ValueRef, heap: heap) {
    let free_fn = match heap {
      heap_managed | heap_managed_unique => {
        let f: @fn(block) -> block = |a| glue::trans_free(a, ptr);
        f
      }
      heap_exchange => {
        let f: @fn(block) -> block = |a| glue::trans_exchange_free(a, ptr);
        f
      }
    };
    do in_scope_cx(cx) |scope_info| {
        scope_info.cleanups.push(clean_temp(ptr, free_fn,
                                      normal_exit_and_unwind));
        grow_scope_clean(scope_info);
    }
}

// Note that this only works for temporaries. We should, at some point, move
// to a system where we can also cancel the cleanup on local variables, but
// this will be more involved. For now, we simply zero out the local, and the
// drop glue checks whether it is zero.
pub fn revoke_clean(cx: block, val: ValueRef) {
    do in_scope_cx(cx) |scope_info| {
        let cleanup_pos = scope_info.cleanups.iter().position_(
            |cu| match *cu {
                clean_temp(v, _, _) if v == val => true,
                _ => false
            });
        for cleanup_pos.iter().advance |i| {
            scope_info.cleanups =
                vec::append(scope_info.cleanups.slice(0u, *i).to_owned(),
                            scope_info.cleanups.slice(*i + 1u,
                                                      scope_info.cleanups.len()));
            shrink_scope_clean(scope_info, *i);
        }
    }
}

pub fn block_cleanups(bcx: block) -> ~[cleanup] {
    match bcx.kind {
       block_non_scope  => ~[],
       block_scope(inf) => /*bad*/copy inf.cleanups
    }
}

pub enum block_kind {
    // A scope at the end of which temporary values created inside of it are
    // cleaned up. May correspond to an actual block in the language, but also
    // to an implicit scope, for example, calls introduce an implicit scope in
    // which the arguments are evaluated and cleaned up.
    block_scope(@mut scope_info),

    // A non-scope block is a basic block created as a translation artifact
    // from translating code that expresses conditional logic rather than by
    // explicit { ... } block structure in the source language.  It's called a
    // non-scope block because it doesn't introduce a new variable scope.
    block_non_scope,
}

pub struct scope_info {
    loop_break: Option<block>,
    loop_label: Option<ident>,
    // A list of functions that must be run at when leaving this
    // block, cleaning up any variables that were introduced in the
    // block.
    cleanups: ~[cleanup],
    // Existing cleanup paths that may be reused, indexed by destination and
    // cleared when the set of cleanups changes.
    cleanup_paths: ~[cleanup_path],
    // Unwinding landing pad. Also cleared when cleanups change.
    landing_pad: Option<BasicBlockRef>,
}

impl scope_info {
    pub fn empty_cleanups(&mut self) -> bool {
        self.cleanups.is_empty()
    }
}

pub trait get_node_info {
    fn info(&self) -> Option<NodeInfo>;
}

impl get_node_info for ast::expr {
    fn info(&self) -> Option<NodeInfo> {
        Some(NodeInfo {id: self.id,
                       callee_id: self.get_callee_id(),
                       span: self.span})
    }
}

impl get_node_info for ast::blk {
    fn info(&self) -> Option<NodeInfo> {
        Some(NodeInfo {id: self.node.id,
                       callee_id: None,
                       span: self.span})
    }
}

impl get_node_info for Option<@ast::expr> {
    fn info(&self) -> Option<NodeInfo> {
        self.chain_ref(|s| s.info())
    }
}

pub struct NodeInfo {
    id: ast::node_id,
    callee_id: Option<ast::node_id>,
    span: span
}

// Basic block context.  We create a block context for each basic block
// (single-entry, single-exit sequence of instructions) we generate from Rust
// code.  Each basic block we generate is attached to a function, typically
// with many basic blocks per function.  All the basic blocks attached to a
// function are organized as a directed graph.
pub struct block_ {
    // The BasicBlockRef returned from a call to
    // llvm::LLVMAppendBasicBlock(llfn, name), which adds a basic
    // block to the function pointed to by llfn.  We insert
    // instructions into that block by way of this block context.
    // The block pointing to this one in the function's digraph.
    llbb: BasicBlockRef,
    terminated: bool,
    unreachable: bool,
    parent: Option<block>,
    // The 'kind' of basic block this is.
    kind: block_kind,
    // Is this block part of a landing pad?
    is_lpad: bool,
    // info about the AST node this block originated from, if any
    node_info: Option<NodeInfo>,
    // The function context for the function to which this block is
    // attached.
    fcx: fn_ctxt
}

pub fn block_(llbb: BasicBlockRef, parent: Option<block>, kind: block_kind,
              is_lpad: bool, node_info: Option<NodeInfo>, fcx: fn_ctxt)
    -> block_ {

    block_ {
        llbb: llbb,
        terminated: false,
        unreachable: false,
        parent: parent,
        kind: kind,
        is_lpad: is_lpad,
        node_info: node_info,
        fcx: fcx
    }
}

pub type block = @mut block_;

pub fn mk_block(llbb: BasicBlockRef, parent: Option<block>, kind: block_kind,
            is_lpad: bool, node_info: Option<NodeInfo>, fcx: fn_ctxt)
    -> block {
    @mut block_(llbb, parent, kind, is_lpad, node_info, fcx)
}

pub struct Result {
    bcx: block,
    val: ValueRef
}

pub fn rslt(bcx: block, val: ValueRef) -> Result {
    Result {bcx: bcx, val: val}
}

impl Result {
    pub fn unpack(&self, bcx: &mut block) -> ValueRef {
        *bcx = self.bcx;
        return self.val;
    }
}

pub fn val_ty(v: ValueRef) -> Type {
    unsafe {
        Type::from_ref(llvm::LLVMTypeOf(v))
    }
}

pub fn in_scope_cx(cx: block, f: &fn(si: &mut scope_info)) {
    let mut cur = cx;
    loop {
        match cur.kind {
            block_scope(inf) => {
                debug!("in_scope_cx: selected cur=%s (cx=%s)",
                       cur.to_str(), cx.to_str());
                f(inf);
                return;
            }
            _ => ()
        }
        cur = block_parent(cur);
    }
}

pub fn block_parent(cx: block) -> block {
    match cx.parent {
      Some(b) => b,
      None    => cx.sess().bug(fmt!("block_parent called on root block %?",
                                   cx))
    }
}

// Accessors

impl block_ {
    pub fn ccx(&self) -> @mut CrateContext { self.fcx.ccx }
    pub fn tcx(&self) -> ty::ctxt { self.fcx.ccx.tcx }
    pub fn sess(&self) -> Session { self.fcx.ccx.sess }

    pub fn node_id_to_str(&self, id: ast::node_id) -> ~str {
        ast_map::node_id_to_str(self.tcx().items, id, self.sess().intr())
    }

    pub fn expr_to_str(&self, e: @ast::expr) -> ~str {
        e.repr(self.tcx())
    }

    pub fn expr_is_lval(&self, e: &ast::expr) -> bool {
        ty::expr_is_lval(self.tcx(), self.ccx().maps.method_map, e)
    }

    pub fn expr_kind(&self, e: &ast::expr) -> ty::ExprKind {
        ty::expr_kind(self.tcx(), self.ccx().maps.method_map, e)
    }

    pub fn def(&self, nid: ast::node_id) -> ast::def {
        match self.tcx().def_map.find(&nid) {
            Some(&v) => v,
            None => {
                self.tcx().sess.bug(fmt!(
                    "No def associated with node id %?", nid));
            }
        }
    }

    pub fn val_to_str(&self, val: ValueRef) -> ~str {
        self.ccx().tn.val_to_str(val)
    }

    pub fn llty_str(&self, ty: Type) -> ~str {
        self.ccx().tn.type_to_str(ty)
    }

    pub fn ty_to_str(&self, t: ty::t) -> ~str {
        t.repr(self.tcx())
    }

    pub fn to_str(&self) -> ~str {
        unsafe {
            match self.node_info {
                Some(node_info) => fmt!("[block %d]", node_info.id),
                None => fmt!("[block %x]", transmute(&*self)),
            }
        }
    }
}

// Let T be the content of a box @T.  tuplify_box_ty(t) returns the
// representation of @T as a tuple (i.e., the ty::t version of what T_box()
// returns).
pub fn tuplify_box_ty(tcx: ty::ctxt, t: ty::t) -> ty::t {
    let ptr = ty::mk_ptr(
        tcx,
        ty::mt {ty: ty::mk_i8(), mutbl: ast::m_imm}
    );
    return ty::mk_tup(tcx, ~[ty::mk_uint(), ty::mk_type(tcx),
                         ptr, ptr,
                         t]);
}


// LLVM constant constructors.
pub fn C_null(t: Type) -> ValueRef {
    unsafe {
        llvm::LLVMConstNull(t.to_ref())
    }
}

pub fn C_undef(t: Type) -> ValueRef {
    unsafe {
        llvm::LLVMGetUndef(t.to_ref())
    }
}

pub fn C_integral(t: Type, u: u64, sign_extend: bool) -> ValueRef {
    unsafe {
        llvm::LLVMConstInt(t.to_ref(), u, sign_extend as Bool)
    }
}

pub fn C_floating(s: &str, t: Type) -> ValueRef {
    unsafe {
        do s.as_c_str |buf| {
            llvm::LLVMConstRealOfString(t.to_ref(), buf)
        }
    }
}

pub fn C_nil() -> ValueRef {
    return C_struct([]);
}

pub fn C_bool(val: bool) -> ValueRef {
    C_integral(Type::bool(), val as u64, false)
}

pub fn C_i1(val: bool) -> ValueRef {
    C_integral(Type::i1(), val as u64, false)
}

pub fn C_i32(i: i32) -> ValueRef {
    return C_integral(Type::i32(), i as u64, true);
}

pub fn C_i64(i: i64) -> ValueRef {
    return C_integral(Type::i64(), i as u64, true);
}

pub fn C_int(cx: &CrateContext, i: int) -> ValueRef {
    return C_integral(cx.int_type, i as u64, true);
}

pub fn C_uint(cx: &CrateContext, i: uint) -> ValueRef {
    return C_integral(cx.int_type, i as u64, false);
}

pub fn C_u8(i: uint) -> ValueRef {
    return C_integral(Type::i8(), i as u64, false);
}


// This is a 'c-like' raw string, which differs from
// our boxed-and-length-annotated strings.
pub fn C_cstr(cx: &mut CrateContext, s: @str) -> ValueRef {
    unsafe {
        match cx.const_cstr_cache.find_equiv(&s) {
            Some(&llval) => return llval,
            None => ()
        }

        let sc = do s.as_c_str |buf| {
            llvm::LLVMConstStringInContext(cx.llcx, buf, s.len() as c_uint, False)
        };

        let gsym = token::gensym("str");
        let g = do fmt!("str%u", gsym).as_c_str |buf| {
            llvm::LLVMAddGlobal(cx.llmod, val_ty(sc).to_ref(), buf)
        };
        llvm::LLVMSetInitializer(g, sc);
        llvm::LLVMSetGlobalConstant(g, True);
        lib::llvm::SetLinkage(g, lib::llvm::InternalLinkage);

        cx.const_cstr_cache.insert(s, g);

        return g;
    }
}

// NB: Do not use `do_spill_noroot` to make this into a constant string, or
// you will be kicked off fast isel. See issue #4352 for an example of this.
pub fn C_estr_slice(cx: &mut CrateContext, s: @str) -> ValueRef {
    unsafe {
        let len = s.len();
        let cs = llvm::LLVMConstPointerCast(C_cstr(cx, s), Type::i8p().to_ref());
        C_struct([cs, C_uint(cx, len + 1u /* +1 for null */)])
    }
}

// Returns a Plain Old LLVM String:
pub fn C_postr(s: &str) -> ValueRef {
    unsafe {
        do s.as_c_str |buf| {
            llvm::LLVMConstStringInContext(base::task_llcx(), buf, s.len() as c_uint, False)
        }
    }
}

pub fn C_zero_byte_arr(size: uint) -> ValueRef {
    unsafe {
        let mut i = 0u;
        let mut elts: ~[ValueRef] = ~[];
        while i < size { elts.push(C_u8(0u)); i += 1u; }
        return llvm::LLVMConstArray(Type::i8().to_ref(),
                                    vec::raw::to_ptr(elts), elts.len() as c_uint);
    }
}

pub fn C_struct(elts: &[ValueRef]) -> ValueRef {
    unsafe {
        do vec::as_imm_buf(elts) |ptr, len| {
            llvm::LLVMConstStructInContext(base::task_llcx(), ptr, len as c_uint, False)
        }
    }
}

pub fn C_packed_struct(elts: &[ValueRef]) -> ValueRef {
    unsafe {
        do vec::as_imm_buf(elts) |ptr, len| {
            llvm::LLVMConstStructInContext(base::task_llcx(), ptr, len as c_uint, True)
        }
    }
}

pub fn C_named_struct(T: Type, elts: &[ValueRef]) -> ValueRef {
    unsafe {
        do vec::as_imm_buf(elts) |ptr, len| {
            llvm::LLVMConstNamedStruct(T.to_ref(), ptr, len as c_uint)
        }
    }
}

pub fn C_array(ty: Type, elts: &[ValueRef]) -> ValueRef {
    unsafe {
        return llvm::LLVMConstArray(ty.to_ref(), vec::raw::to_ptr(elts), elts.len() as c_uint);
    }
}

pub fn C_bytes(bytes: &[u8]) -> ValueRef {
    unsafe {
        let ptr = cast::transmute(vec::raw::to_ptr(bytes));
        return llvm::LLVMConstStringInContext(base::task_llcx(), ptr, bytes.len() as c_uint, True);
    }
}

pub fn C_bytes_plus_null(bytes: &[u8]) -> ValueRef {
    unsafe {
        return llvm::LLVMConstStringInContext(base::task_llcx(),
            cast::transmute(vec::raw::to_ptr(bytes)),
            bytes.len() as c_uint, False);
    }
}

pub fn get_param(fndecl: ValueRef, param: uint) -> ValueRef {
    unsafe {
        llvm::LLVMGetParam(fndecl, param as c_uint)
    }
}

pub fn const_get_elt(cx: &CrateContext, v: ValueRef, us: &[c_uint])
                  -> ValueRef {
    unsafe {
        let r = do vec::as_imm_buf(us) |p, len| {
            llvm::LLVMConstExtractValue(v, p, len as c_uint)
        };

        debug!("const_get_elt(v=%s, us=%?, r=%s)",
               cx.tn.val_to_str(v), us, cx.tn.val_to_str(r));

        return r;
    }
}

pub fn const_to_int(v: ValueRef) -> c_longlong {
    unsafe {
        llvm::LLVMConstIntGetSExtValue(v)
    }
}

pub fn const_to_uint(v: ValueRef) -> c_ulonglong {
    unsafe {
        llvm::LLVMConstIntGetZExtValue(v)
    }
}

pub fn is_undef(val: ValueRef) -> bool {
    unsafe {
        llvm::LLVMIsUndef(val) != False
    }
}

pub fn is_null(val: ValueRef) -> bool {
    unsafe {
        llvm::LLVMIsNull(val) != False
    }
}

// Used to identify cached monomorphized functions and vtables
#[deriving(Eq)]
pub enum mono_param_id {
    mono_precise(ty::t, Option<@~[mono_id]>),
    mono_any,
    mono_repr(uint /* size */,
              uint /* align */,
              MonoDataClass,
              datum::DatumMode),
}

#[deriving(Eq)]
pub enum MonoDataClass {
    MonoBits,    // Anything not treated differently from arbitrary integer data
    MonoNonNull, // Non-null pointers (used for optional-pointer optimization)
    // FIXME(#3547)---scalars and floats are
    // treated differently in most ABIs.  But we
    // should be doing something more detailed
    // here.
    MonoFloat
}

pub fn mono_data_classify(t: ty::t) -> MonoDataClass {
    match ty::get(t).sty {
        ty::ty_float(_) => MonoFloat,
        ty::ty_rptr(*) | ty::ty_uniq(*) |
        ty::ty_box(*) | ty::ty_opaque_box(*) |
        ty::ty_estr(ty::vstore_uniq) | ty::ty_evec(_, ty::vstore_uniq) |
        ty::ty_estr(ty::vstore_box) | ty::ty_evec(_, ty::vstore_box) |
        ty::ty_bare_fn(*) => MonoNonNull,
        // Is that everything?  Would closures or slices qualify?
        _ => MonoBits
    }
}


#[deriving(Eq)]
pub struct mono_id_ {
    def: ast::def_id,
    params: ~[mono_param_id],
    impl_did_opt: Option<ast::def_id>
}

pub type mono_id = @mono_id_;

impl to_bytes::IterBytes for mono_param_id {
    fn iter_bytes(&self, lsb0: bool, f: to_bytes::Cb) -> bool {
        match *self {
            mono_precise(t, ref mids) => {
                0u8.iter_bytes(lsb0, f) &&
                ty::type_id(t).iter_bytes(lsb0, f) &&
                mids.iter_bytes(lsb0, f)
            }

            mono_any => 1u8.iter_bytes(lsb0, f),

            mono_repr(ref a, ref b, ref c, ref d) => {
                2u8.iter_bytes(lsb0, f) &&
                a.iter_bytes(lsb0, f) &&
                b.iter_bytes(lsb0, f) &&
                c.iter_bytes(lsb0, f) &&
                d.iter_bytes(lsb0, f)
            }
        }
    }
}

impl to_bytes::IterBytes for MonoDataClass {
    fn iter_bytes(&self, lsb0: bool, f:to_bytes::Cb) -> bool {
        (*self as u8).iter_bytes(lsb0, f)
    }
}

impl to_bytes::IterBytes for mono_id_ {
    fn iter_bytes(&self, lsb0: bool, f: to_bytes::Cb) -> bool {
        self.def.iter_bytes(lsb0, f) && self.params.iter_bytes(lsb0, f)
    }
}

pub fn umax(cx: block, a: ValueRef, b: ValueRef) -> ValueRef {
    let cond = build::ICmp(cx, lib::llvm::IntULT, a, b);
    return build::Select(cx, cond, b, a);
}

pub fn umin(cx: block, a: ValueRef, b: ValueRef) -> ValueRef {
    let cond = build::ICmp(cx, lib::llvm::IntULT, a, b);
    return build::Select(cx, cond, a, b);
}

pub fn align_to(cx: block, off: ValueRef, align: ValueRef) -> ValueRef {
    let mask = build::Sub(cx, align, C_int(cx.ccx(), 1));
    let bumped = build::Add(cx, off, mask);
    return build::And(cx, bumped, build::Not(cx, mask));
}

pub fn path_str(sess: session::Session, p: &[path_elt]) -> ~str {
    let mut r = ~"";
    let mut first = true;
    for p.iter().advance |e| {
        match *e {
            ast_map::path_name(s) | ast_map::path_mod(s) => {
                if first {
                    first = false
                } else {
                    r.push_str("::")
                }
                r.push_str(sess.str_of(s));
            }
        }
    }
    r
}

pub fn monomorphize_type(bcx: block, t: ty::t) -> ty::t {
    match bcx.fcx.param_substs {
        Some(substs) => {
            ty::subst_tps(bcx.tcx(), substs.tys, substs.self_ty, t)
        }
        _ => {
            assert!(!ty::type_has_params(t));
            assert!(!ty::type_has_self(t));
            t
        }
    }
}

pub fn node_id_type(bcx: block, id: ast::node_id) -> ty::t {
    let tcx = bcx.tcx();
    let t = ty::node_id_to_type(tcx, id);
    monomorphize_type(bcx, t)
}

pub fn expr_ty(bcx: block, ex: &ast::expr) -> ty::t {
    node_id_type(bcx, ex.id)
}

pub fn expr_ty_adjusted(bcx: block, ex: &ast::expr) -> ty::t {
    let tcx = bcx.tcx();
    let t = ty::expr_ty_adjusted(tcx, ex);
    monomorphize_type(bcx, t)
}

pub fn node_id_type_params(bcx: block, id: ast::node_id) -> ~[ty::t] {
    let tcx = bcx.tcx();
    let params = ty::node_id_to_type_params(tcx, id);

    if !params.iter().all(|t| !ty::type_needs_infer(*t)) {
        bcx.sess().bug(
            fmt!("Type parameters for node %d include inference types: %s",
                 id, params.map(|t| bcx.ty_to_str(*t)).connect(",")));
    }

    match bcx.fcx.param_substs {
      Some(substs) => {
        do vec::map(params) |t| {
            ty::subst_tps(tcx, substs.tys, substs.self_ty, *t)
        }
      }
      _ => params
    }
}

pub fn node_vtables(bcx: block, id: ast::node_id)
                 -> Option<typeck::vtable_res> {
    let raw_vtables = bcx.ccx().maps.vtable_map.find(&id);
    raw_vtables.map(
        |&vts| resolve_vtables_in_fn_ctxt(bcx.fcx, *vts))
}

pub fn resolve_vtables_in_fn_ctxt(fcx: fn_ctxt, vts: typeck::vtable_res)
    -> typeck::vtable_res {
    resolve_vtables_under_param_substs(fcx.ccx.tcx,
                                       fcx.param_substs,
                                       vts)
}

pub fn resolve_vtables_under_param_substs(tcx: ty::ctxt,
                                          param_substs: Option<@param_substs>,
                                          vts: typeck::vtable_res)
    -> typeck::vtable_res {
    @vec::map(*vts, |ds|
      @vec::map(**ds, |d|
                resolve_vtable_under_param_substs(tcx, param_substs, copy *d)))
}


// Apply the typaram substitutions in the fn_ctxt to a vtable. This should
// eliminate any vtable_params.
pub fn resolve_vtable_in_fn_ctxt(fcx: fn_ctxt, vt: typeck::vtable_origin)
    -> typeck::vtable_origin {
    resolve_vtable_under_param_substs(fcx.ccx.tcx,
                                      fcx.param_substs,
                                      vt)
}

pub fn resolve_vtable_under_param_substs(tcx: ty::ctxt,
                                         param_substs: Option<@param_substs>,
                                         vt: typeck::vtable_origin)
    -> typeck::vtable_origin {
    match vt {
        typeck::vtable_static(trait_id, tys, sub) => {
            let tys = match param_substs {
                Some(substs) => {
                    do vec::map(tys) |t| {
                        ty::subst_tps(tcx, substs.tys, substs.self_ty, *t)
                    }
                }
                _ => tys
            };
            typeck::vtable_static(
                trait_id, tys,
                resolve_vtables_under_param_substs(tcx, param_substs, sub))
        }
        typeck::vtable_param(n_param, n_bound) => {
            match param_substs {
                Some(substs) => {
                    find_vtable(tcx, substs, n_param, n_bound)
                }
                _ => {
                    tcx.sess.bug(fmt!(
                        "resolve_vtable_in_fn_ctxt: asked to lookup but \
                         no vtables in the fn_ctxt!"))
                }
            }
        }
        typeck::vtable_self(_trait_id) => {
            match param_substs {
                Some(@param_substs
                     {self_vtable: Some(ref self_vtable), _}) => {
                    copy *self_vtable
                }
                _ => {
                    tcx.sess.bug(fmt!(
                        "resolve_vtable_in_fn_ctxt: asked to lookup but \
                         no self_vtable in the fn_ctxt!"))
                }
            }
        }
    }
}

pub fn find_vtable(tcx: ty::ctxt, ps: &param_substs,
                   n_param: uint, n_bound: uint)
    -> typeck::vtable_origin {
    debug!("find_vtable(n_param=%u, n_bound=%u, ps=%s)",
           n_param, n_bound, ps.repr(tcx));

    /*bad*/ copy ps.vtables.get()[n_param][n_bound]
}

pub fn dummy_substs(tps: ~[ty::t]) -> ty::substs {
    substs {
        self_r: Some(ty::re_bound(ty::br_self)),
        self_ty: None,
        tps: tps
    }
}

pub fn filename_and_line_num_from_span(bcx: block,
                                       span: span) -> (ValueRef, ValueRef) {
    let loc = bcx.sess().parse_sess.cm.lookup_char_pos(span.lo);
    let filename_cstr = C_cstr(bcx.ccx(), loc.file.name);
    let filename = build::PointerCast(bcx, filename_cstr, Type::i8p());
    let line = C_int(bcx.ccx(), loc.line as int);
    (filename, line)
}

// Casts a Rust bool value to an i1.
pub fn bool_to_i1(bcx: block, llval: ValueRef) -> ValueRef {
    build::ICmp(bcx, lib::llvm::IntNE, llval, C_bool(false))
}
