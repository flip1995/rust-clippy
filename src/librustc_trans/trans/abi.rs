// Copyright 2012-2016 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

pub use self::ArgKind::*;

use llvm;
use trans::common::{return_type_is_void, type_is_fat_ptr};
use trans::context::CrateContext;
use trans::cabi_x86;
use trans::cabi_x86_64;
use trans::cabi_x86_win64;
use trans::cabi_arm;
use trans::cabi_aarch64;
use trans::cabi_powerpc;
use trans::cabi_powerpc64;
use trans::cabi_mips;
use trans::cabi_asmjs;
use trans::machine::{llsize_of_alloc, llsize_of_real};
use trans::type_::Type;
use trans::type_of;

use middle::ty::{self, Ty};

pub use syntax::abi::Abi;

/// The first half of a fat pointer.
/// - For a closure, this is the code address.
/// - For an object or trait instance, this is the address of the box.
/// - For a slice, this is the base address.
pub const FAT_PTR_ADDR: usize = 0;

/// The second half of a fat pointer.
/// - For a closure, this is the address of the environment.
/// - For an object or trait instance, this is the address of the vtable.
/// - For a slice, this is the length.
pub const FAT_PTR_EXTRA: usize = 1;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum ArgKind {
    /// Pass the argument directly using the normal converted
    /// LLVM type or by coercing to another specified type
    Direct,
    /// Pass the argument indirectly via a hidden pointer
    Indirect,
    /// Ignore the argument (useful for empty struct)
    Ignore,
}

/// Information about how a specific C type
/// should be passed to or returned from a function
///
/// This is borrowed from clang's ABIInfo.h
#[derive(Clone, Copy, Debug)]
pub struct ArgType {
    pub kind: ArgKind,
    /// Original LLVM type
    pub ty: Type,
    /// Coerced LLVM Type
    pub cast: Option<Type>,
    /// Dummy argument, which is emitted before the real argument
    pub pad: Option<Type>,
    /// LLVM attribute of argument
    pub attr: Option<llvm::Attribute>
}

impl ArgType {
    fn new(ty: Type) -> ArgType {
        ArgType {
            kind: Direct,
            ty: ty,
            cast: None,
            pad: None,
            attr: None
        }
    }

    pub fn is_indirect(&self) -> bool {
        self.kind == Indirect
    }

    pub fn is_ignore(&self) -> bool {
        self.kind == Ignore
    }
}

fn c_type_of<'a, 'tcx>(cx: &CrateContext<'a, 'tcx>, ty: Ty<'tcx>) -> Type {
    if ty.is_bool() {
        Type::i1(cx)
    } else {
        type_of::type_of(cx, ty)
    }
}

/// Metadata describing how the arguments to a native function
/// should be passed in order to respect the native ABI.
///
/// I will do my best to describe this structure, but these
/// comments are reverse-engineered and may be inaccurate. -NDM
pub struct FnType {
    /// The LLVM types of each argument.
    pub args: Vec<ArgType>,

    /// LLVM return type.
    pub ret: ArgType,

    pub variadic: bool,

    pub cconv: llvm::CallConv
}

impl FnType {
    pub fn new<'a, 'tcx>(ccx: &CrateContext<'a, 'tcx>,
                         abi: Abi,
                         sig: &ty::FnSig<'tcx>,
                         extra_args: &[Ty<'tcx>]) -> FnType {
        use self::Abi::*;
        let cconv = match ccx.sess().target.target.adjust_abi(abi) {
            RustIntrinsic => {
                // Intrinsics are emitted at the call site
                ccx.sess().bug("asked to compute FnType of intrinsic");
            }
            PlatformIntrinsic => {
                // Intrinsics are emitted at the call site
                ccx.sess().bug("asked to compute FnType of platform intrinsic");
            }

            Rust | RustCall => llvm::CCallConv,

            // It's the ABI's job to select this, not us.
            System => ccx.sess().bug("system abi should be selected elsewhere"),

            Stdcall => llvm::X86StdcallCallConv,
            Fastcall => llvm::X86FastcallCallConv,
            Vectorcall => llvm::X86_VectorCall,
            C => llvm::CCallConv,
            Win64 => llvm::X86_64_Win64,

            // These API constants ought to be more specific...
            Cdecl => llvm::CCallConv,
            Aapcs => llvm::CCallConv,
        };

        let rty = match sig.output {
            ty::FnConverging(ret_ty) if !return_type_is_void(ccx, ret_ty) => {
                c_type_of(ccx, ret_ty)
            }
            _ => Type::void(ccx)
        };

        let mut inputs = &sig.inputs[..];
        let extra_args = if abi == RustCall {
            assert!(!sig.variadic && extra_args.is_empty());

            match inputs[inputs.len() - 1].sty {
                ty::TyTuple(ref tupled_arguments) => {
                    inputs = &inputs[..inputs.len() - 1];
                    &tupled_arguments[..]
                }
                _ => {
                    unreachable!("argument to function with \"rust-call\" ABI \
                                  is not a tuple");
                }
            }
        } else {
            assert!(sig.variadic || extra_args.is_empty());
            extra_args
        };

        let mut args = Vec::with_capacity(inputs.len() + extra_args.len());
        for ty in inputs.iter().chain(extra_args.iter()) {
            let llty = c_type_of(ccx, ty);
            if type_is_fat_ptr(ccx.tcx(), ty) {
                args.extend(llty.field_types().into_iter().map(ArgType::new));
            } else {
                args.push(ArgType::new(llty));
            }
        }

        let mut fty = FnType {
            args: args,
            ret: ArgType::new(rty),
            variadic: sig.variadic,
            cconv: cconv
        };

        // Add ZExt attributes to i1 arguments and returns.
        for arg in Some(&mut fty.ret).into_iter().chain(&mut fty.args) {
            if arg.ty == Type::i1(ccx) {
                arg.attr = Some(llvm::Attribute::ZExt);
            }
        }

        if abi == Rust || abi == RustCall {
            let fixup = |arg: &mut ArgType| {
                if !arg.ty.is_aggregate() {
                    // Scalars and vectors, always immediate.
                    return;
                }
                let size = llsize_of_alloc(ccx, arg.ty);
                if size > llsize_of_alloc(ccx, ccx.int_type()) {
                    arg.kind = Indirect;
                } else if size > 0 {
                    // We want to pass small aggregates as immediates, but using
                    // a LLVM aggregate type for this leads to bad optimizations,
                    // so we pick an appropriately sized integer type instead.
                    arg.cast = Some(Type::ix(ccx, size * 8));
                }
            };
            if let ty::FnConverging(ret_ty) = sig.output {
                // Fat pointers are returned by-value.
                if !type_is_fat_ptr(ccx.tcx(), ret_ty) &&
                   fty.ret.ty != Type::void(ccx) {
                    fixup(&mut fty.ret);
                }
            };
            for arg in &mut fty.args {
                fixup(arg);
            }
            return fty;
        }

        match &ccx.sess().target.target.arch[..] {
            "x86" => cabi_x86::compute_abi_info(ccx, &mut fty),
            "x86_64" => if ccx.sess().target.target.options.is_like_windows {
                cabi_x86_win64::compute_abi_info(ccx, &mut fty);
            } else {
                cabi_x86_64::compute_abi_info(ccx, &mut fty);
            },
            "aarch64" => cabi_aarch64::compute_abi_info(ccx, &mut fty),
            "arm" => {
                let flavor = if ccx.sess().target.target.target_os == "ios" {
                    cabi_arm::Flavor::Ios
                } else {
                    cabi_arm::Flavor::General
                };
                cabi_arm::compute_abi_info(ccx, &mut fty, flavor);
            },
            "mips" => cabi_mips::compute_abi_info(ccx, &mut fty),
            "powerpc" => cabi_powerpc::compute_abi_info(ccx, &mut fty),
            "powerpc64" => cabi_powerpc64::compute_abi_info(ccx, &mut fty),
            "asmjs" => cabi_asmjs::compute_abi_info(ccx, &mut fty),
            a => ccx.sess().fatal(&format!("unrecognized arch \"{}\" in target specification", a))
        }

        fty
    }

    pub fn llvm_type(&self, ccx: &CrateContext) -> Type {
        let mut llargument_tys = Vec::new();

        let llreturn_ty = if self.ret.is_indirect() {
            llargument_tys.push(self.ret.ty.ptr_to());
            Type::void(ccx)
        } else {
            self.ret.cast.unwrap_or(self.ret.ty)
        };

        for arg in &self.args {
            if arg.is_ignore() {
                continue;
            }
            // add padding
            if let Some(ty) = arg.pad {
                llargument_tys.push(ty);
            }

            let llarg_ty = if arg.is_indirect() {
                arg.ty.ptr_to()
            } else {
                arg.cast.unwrap_or(arg.ty)
            };

            llargument_tys.push(llarg_ty);
        }

        if self.variadic {
            Type::variadic_func(&llargument_tys, &llreturn_ty)
        } else {
            Type::func(&llargument_tys, &llreturn_ty)
        }
    }

    pub fn llvm_attrs(&self, ccx: &CrateContext) -> llvm::AttrBuilder {
        let mut attrs = llvm::AttrBuilder::new();
        let mut i = if self.ret.is_indirect() { 1 } else { 0 };

        // Add attributes that are always applicable, independent of the concrete foreign ABI
        if self.ret.is_indirect() {
            let llret_sz = llsize_of_real(ccx, self.ret.ty);

            // The outptr can be noalias and nocapture because it's entirely
            // invisible to the program. We also know it's nonnull as well
            // as how many bytes we can dereference
            attrs.arg(i, llvm::Attribute::StructRet)
                 .arg(i, llvm::Attribute::NoAlias)
                 .arg(i, llvm::Attribute::NoCapture)
                 .arg(i, llvm::DereferenceableAttribute(llret_sz));
        };

        // Add attributes that depend on the concrete foreign ABI
        if let Some(attr) = self.ret.attr {
            attrs.arg(i, attr);
        }

        i += 1;
        for arg in &self.args {
            if arg.is_ignore() {
                continue;
            }
            // skip padding
            if arg.pad.is_some() { i += 1; }

            if let Some(attr) = arg.attr {
                attrs.arg(i, attr);
            }

            i += 1;
        }

        attrs
    }
}
