// Copyright 2012-2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![allow(non_camel_case_types, non_snake_case)]

//! Code that is useful in various codegen modules.

use llvm::{self, True, False, Bool, BasicBlock, OperandBundleDef};
use abi;
use consts;
use type_::Type;
use type_of::LayoutLlvmExt;
use value::Value;
use rustc_codegen_ssa::traits::*;

use rustc::ty::layout::{HasDataLayout, LayoutOf, self, TyLayout, Size};
use rustc::mir::interpret::{Scalar, AllocType, Allocation};
use consts::const_alloc_to_llvm;
use rustc_codegen_ssa::mir::place::PlaceRef;

use libc::{c_uint, c_char};

use syntax::symbol::LocalInternedString;
use syntax::ast::Mutability;

pub use context::CodegenCx;

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

/// A structure representing an active landing pad for the duration of a basic
/// block.
///
/// Each `Block` may contain an instance of this, indicating whether the block
/// is part of a landing pad or not. This is used to make decision about whether
/// to emit `invoke` instructions (e.g., in a landing pad we don't continue to
/// use `invoke`) and also about various function call metadata.
///
/// For GNU exceptions (`landingpad` + `resume` instructions) this structure is
/// just a bunch of `None` instances (not too interesting), but for MSVC
/// exceptions (`cleanuppad` + `cleanupret` instructions) this contains data.
/// When inside of a landing pad, each function call in LLVM IR needs to be
/// annotated with which landing pad it's a part of. This is accomplished via
/// the `OperandBundleDef` value created for MSVC landing pads.
pub struct Funclet<'ll> {
    cleanuppad: &'ll Value,
    operand: OperandBundleDef<'ll>,
}

impl Funclet<'ll> {
    pub fn new(cleanuppad: &'ll Value) -> Self {
        Funclet {
            cleanuppad,
            operand: OperandBundleDef::new("funclet", &[cleanuppad]),
        }
    }

    pub fn cleanuppad(&self) -> &'ll Value {
        self.cleanuppad
    }

    pub fn bundle(&self) -> &OperandBundleDef<'ll> {
        &self.operand
    }
}

impl BackendTypes for CodegenCx<'ll, 'tcx> {
    type Value = &'ll Value;
    type BasicBlock = &'ll BasicBlock;
    type Type = &'ll Type;
    type Funclet = Funclet<'ll>;

    type DIScope = &'ll llvm::debuginfo::DIScope;
}

impl ConstMethods<'tcx> for CodegenCx<'ll, 'tcx> {
    fn const_null(&self, t: &'ll Type) -> &'ll Value {
        unsafe {
            llvm::LLVMConstNull(t)
        }
    }

    fn const_undef(&self, t: &'ll Type) -> &'ll Value {
        unsafe {
            llvm::LLVMGetUndef(t)
        }
    }

    fn const_int(&self, t: &'ll Type, i: i64) -> &'ll Value {
        unsafe {
            llvm::LLVMConstInt(t, i as u64, True)
        }
    }

    fn const_uint(&self, t: &'ll Type, i: u64) -> &'ll Value {
        unsafe {
            llvm::LLVMConstInt(t, i, False)
        }
    }

    fn const_uint_big(&self, t: &'ll Type, u: u128) -> &'ll Value {
        unsafe {
            let words = [u as u64, (u >> 64) as u64];
            llvm::LLVMConstIntOfArbitraryPrecision(t, 2, words.as_ptr())
        }
    }

    fn const_bool(&self, val: bool) -> &'ll Value {
        self.const_uint(self.type_i1(), val as u64)
    }

    fn const_i32(&self, i: i32) -> &'ll Value {
        self.const_int(self.type_i32(), i as i64)
    }

    fn const_u32(&self, i: u32) -> &'ll Value {
        self.const_uint(self.type_i32(), i as u64)
    }

    fn const_u64(&self, i: u64) -> &'ll Value {
        self.const_uint(self.type_i64(), i)
    }

    fn const_usize(&self, i: u64) -> &'ll Value {
        let bit_size = self.data_layout().pointer_size.bits();
        if bit_size < 64 {
            // make sure it doesn't overflow
            assert!(i < (1<<bit_size));
        }

        self.const_uint(self.isize_ty, i)
    }

    fn const_u8(&self, i: u8) -> &'ll Value {
        self.const_uint(self.type_i8(), i as u64)
    }

    fn const_cstr(
        &self,
        s: LocalInternedString,
        null_terminated: bool,
    ) -> &'ll Value {
        unsafe {
            if let Some(&llval) = self.const_cstr_cache.borrow().get(&s) {
                return llval;
            }

            let sc = llvm::LLVMConstStringInContext(self.llcx,
                                                    s.as_ptr() as *const c_char,
                                                    s.len() as c_uint,
                                                    !null_terminated as Bool);
            let sym = self.generate_local_symbol_name("str");
            let g = self.define_global(&sym[..], self.val_ty(sc)).unwrap_or_else(||{
                bug!("symbol `{}` is already defined", sym);
            });
            llvm::LLVMSetInitializer(g, sc);
            llvm::LLVMSetGlobalConstant(g, True);
            llvm::LLVMRustSetLinkage(g, llvm::Linkage::InternalLinkage);

            self.const_cstr_cache.borrow_mut().insert(s, g);
            g
        }
    }

    fn const_str_slice(&self, s: LocalInternedString) -> &'ll Value {
        let len = s.len();
        let cs = consts::ptrcast(self.const_cstr(s, false),
            self.type_ptr_to(self.layout_of(self.tcx.mk_str()).llvm_type(self)));
        self.const_fat_ptr(cs, self.const_usize(len as u64))
    }

    fn const_fat_ptr(
        &self,
        ptr: &'ll Value,
        meta: &'ll Value
    ) -> &'ll Value {
        assert_eq!(abi::FAT_PTR_ADDR, 0);
        assert_eq!(abi::FAT_PTR_EXTRA, 1);
        self.const_struct(&[ptr, meta], false)
    }

    fn const_struct(
        &self,
        elts: &[&'ll Value],
        packed: bool
    ) -> &'ll Value {
        struct_in_context(self.llcx, elts, packed)
    }

    fn const_array(&self, ty: &'ll Type, elts: &[&'ll Value]) -> &'ll Value {
        unsafe {
            return llvm::LLVMConstArray(ty, elts.as_ptr(), elts.len() as c_uint);
        }
    }

    fn const_vector(&self, elts: &[&'ll Value]) -> &'ll Value {
        unsafe {
            return llvm::LLVMConstVector(elts.as_ptr(), elts.len() as c_uint);
        }
    }

    fn const_bytes(&self, bytes: &[u8]) -> &'ll Value {
        bytes_in_context(self.llcx, bytes)
    }

    fn const_get_elt(&self, v: &'ll Value, idx: u64) -> &'ll Value {
        unsafe {
            assert_eq!(idx as c_uint as u64, idx);
            let us = &[idx as c_uint];
            let r = llvm::LLVMConstExtractValue(v, us.as_ptr(), us.len() as c_uint);

            debug!("const_get_elt(v={:?}, idx={}, r={:?})",
                   v, idx, r);

            r
        }
    }

    fn const_get_real(&self, v: &'ll Value) -> Option<(f64, bool)> {
        unsafe {
            if self.is_const_real(v) {
                let mut loses_info: llvm::Bool = ::std::mem::uninitialized();
                let r = llvm::LLVMConstRealGetDouble(v, &mut loses_info);
                let loses_info = if loses_info == 1 { true } else { false };
                Some((r, loses_info))
            } else {
                None
            }
        }
    }

    fn const_to_uint(&self, v: &'ll Value) -> u64 {
        unsafe {
            llvm::LLVMConstIntGetZExtValue(v)
        }
    }

    fn is_const_integral(&self, v: &'ll Value) -> bool {
        unsafe {
            llvm::LLVMIsAConstantInt(v).is_some()
        }
    }

    fn is_const_real(&self, v: &'ll Value) -> bool {
        unsafe {
            llvm::LLVMIsAConstantFP(v).is_some()
        }
    }

    fn const_to_opt_u128(&self, v: &'ll Value, sign_ext: bool) -> Option<u128> {
        unsafe {
            if self.is_const_integral(v) {
                let (mut lo, mut hi) = (0u64, 0u64);
                let success = llvm::LLVMRustConstInt128Get(v, sign_ext,
                                                           &mut hi, &mut lo);
                if success {
                    Some(hi_lo_to_u128(lo, hi))
                } else {
                    None
                }
            } else {
                None
            }
        }
    }

    fn scalar_to_backend(
        &self,
        cv: Scalar,
        layout: &layout::Scalar,
        llty: &'ll Type,
    ) -> &'ll Value {
        let bitsize = if layout.is_bool() { 1 } else { layout.value.size(self).bits() };
        match cv {
            Scalar::Bits { size: 0, .. } => {
                assert_eq!(0, layout.value.size(self).bytes());
                self.const_undef(self.type_ix(0))
            },
            Scalar::Bits { bits, size } => {
                assert_eq!(size as u64, layout.value.size(self).bytes());
                let llval = self.const_uint_big(self.type_ix(bitsize), bits);
                if layout.value == layout::Pointer {
                    unsafe { llvm::LLVMConstIntToPtr(llval, llty) }
                } else {
                    self.const_bitcast(llval, llty)
                }
            },
            Scalar::Ptr(ptr) => {
                let alloc_type = self.tcx.alloc_map.lock().get(ptr.alloc_id);
                let base_addr = match alloc_type {
                    Some(AllocType::Memory(alloc)) => {
                        let init = const_alloc_to_llvm(self, alloc);
                        if alloc.mutability == Mutability::Mutable {
                            self.static_addr_of_mut(init, alloc.align, None)
                        } else {
                            self.static_addr_of(init, alloc.align, None)
                        }
                    }
                    Some(AllocType::Function(fn_instance)) => {
                        self.get_fn(fn_instance)
                    }
                    Some(AllocType::Static(def_id)) => {
                        assert!(self.tcx.is_static(def_id).is_some());
                        self.get_static(def_id)
                    }
                    None => bug!("missing allocation {:?}", ptr.alloc_id),
                };
                let llval = unsafe { llvm::LLVMConstInBoundsGEP(
                    self.const_bitcast(base_addr, self.type_i8p()),
                    &self.const_usize(ptr.offset.bytes()),
                    1,
                ) };
                if layout.value != layout::Pointer {
                    unsafe { llvm::LLVMConstPtrToInt(llval, llty) }
                } else {
                    self.const_bitcast(llval, llty)
                }
            }
        }
    }

    fn from_const_alloc(
        &self,
        layout: TyLayout<'tcx>,
        alloc: &Allocation,
        offset: Size,
    ) -> PlaceRef<'tcx, &'ll Value> {
        let init = const_alloc_to_llvm(self, alloc);
        let base_addr = self.static_addr_of(init, layout.align.abi, None);

        let llval = unsafe { llvm::LLVMConstInBoundsGEP(
            self.const_bitcast(base_addr, self.type_i8p()),
            &self.const_usize(offset.bytes()),
            1,
        )};
        let llval = self.const_bitcast(llval, self.type_ptr_to(layout.llvm_type(self)));
        PlaceRef::new_sized(llval, layout, alloc.align)
    }

    fn const_ptrcast(&self, val: &'ll Value, ty: &'ll Type) -> &'ll Value {
        consts::ptrcast(val, ty)
    }
}

pub fn val_ty(v: &'ll Value) -> &'ll Type {
    unsafe {
        llvm::LLVMTypeOf(v)
    }
}

pub fn bytes_in_context(llcx: &'ll llvm::Context, bytes: &[u8]) -> &'ll Value {
    unsafe {
        let ptr = bytes.as_ptr() as *const c_char;
        return llvm::LLVMConstStringInContext(llcx, ptr, bytes.len() as c_uint, True);
    }
}

pub fn struct_in_context(
    llcx: &'a llvm::Context,
    elts: &[&'a Value],
    packed: bool,
) -> &'a Value {
    unsafe {
        llvm::LLVMConstStructInContext(llcx,
                                       elts.as_ptr(), elts.len() as c_uint,
                                       packed as Bool)
    }
}

#[inline]
fn hi_lo_to_u128(lo: u64, hi: u64) -> u128 {
    ((hi as u128) << 64) | (lo as u128)
}
