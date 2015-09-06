// Copyright 2012-2015 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

pub use self::ImplOrTraitItemId::*;
pub use self::ClosureKind::*;
pub use self::Variance::*;
pub use self::AutoAdjustment::*;
pub use self::Representability::*;
pub use self::AutoRef::*;
pub use self::DtorKind::*;
pub use self::ExplicitSelfCategory::*;
pub use self::ImplOrTraitItemContainer::*;
pub use self::BorrowKind::*;
pub use self::ImplOrTraitItem::*;
pub use self::IntVarValue::*;
pub use self::CopyImplementationError::*;
pub use self::LvaluePreference::*;

use back::svh::Svh;
use front::map as ast_map;
use front::map::LinkedPath;
use metadata::csearch;
use middle;
use middle::const_eval::{self, ConstVal, ErrKind};
use middle::const_eval::EvalHint::UncheckedExprHint;
use middle::def::{self, ExportMap};
use middle::def_id::{DefId, LOCAL_CRATE};
use middle::lang_items::{FnTraitLangItem, FnMutTraitLangItem, FnOnceTraitLangItem};
use middle::infer;
use middle::pat_util;
use middle::subst::{self, ParamSpace, Subst, Substs, VecPerParamSpace};
use middle::traits;
use middle::ty;
use middle::ty::fold::TypeFolder;
use middle::ty::walk::TypeWalker;
use util::common::memoized;
use util::nodemap::{NodeMap, NodeSet, DefIdMap};
use util::nodemap::FnvHashMap;
use util::num::ToPrimitive;

use std::borrow::{Borrow, Cow};
use std::cell::{Cell, RefCell};
use std::cmp;
use std::hash::{Hash, SipHasher, Hasher};
use std::iter;
use std::rc::Rc;
use std::slice;
use std::vec::IntoIter;
use std::collections::{HashMap, HashSet};
use syntax::ast::{self, CrateNum, Name, NodeId};
use syntax::codemap::Span;
use syntax::parse::token::{InternedString, special_idents};

use rustc_front::hir;
use rustc_front::hir::{ItemImpl, ItemTrait};
use rustc_front::hir::{MutImmutable, MutMutable, Visibility};
use rustc_front::attr::{self, AttrMetaMethods, SignedInt, UnsignedInt};

pub use self::sty::{Binder, DebruijnIndex};
pub use self::sty::{BuiltinBound, BuiltinBounds, ExistentialBounds};
pub use self::sty::{BareFnTy, FnSig, PolyFnSig, FnOutput, PolyFnOutput};
pub use self::sty::{ClosureTy, InferTy, ParamTy, ProjectionTy, TraitTy};
pub use self::sty::{ClosureSubsts, TypeAndMut};
pub use self::sty::{TraitRef, TypeVariants, PolyTraitRef};
pub use self::sty::{BoundRegion, EarlyBoundRegion, FreeRegion, Region};
pub use self::sty::{TyVid, IntVid, FloatVid, RegionVid, SkolemizedRegionVid};
pub use self::sty::BoundRegion::*;
pub use self::sty::FnOutput::*;
pub use self::sty::InferTy::*;
pub use self::sty::Region::*;
pub use self::sty::TypeVariants::*;

pub use self::sty::BuiltinBound::Send as BoundSend;
pub use self::sty::BuiltinBound::Sized as BoundSized;
pub use self::sty::BuiltinBound::Copy as BoundCopy;
pub use self::sty::BuiltinBound::Sync as BoundSync;

pub use self::contents::TypeContents;
pub use self::context::{ctxt, tls};
pub use self::context::{CtxtArenas, Lift, Tables};

pub mod cast;
pub mod error;
pub mod fast_reject;
pub mod fold;
pub mod _match;
pub mod outlives;
pub mod relate;
pub mod walk;
pub mod wf;

mod contents;
mod context;
mod flags;
mod ivar;
mod structural_impls;
mod sty;

pub type Disr = u64;

pub const INITIAL_DISCRIMINANT_VALUE: Disr = 0;

// Data types

/// The complete set of all analyses described in this module. This is
/// produced by the driver and fed to trans and later passes.
pub struct CrateAnalysis {
    pub export_map: ExportMap,
    pub exported_items: middle::privacy::ExportedItems,
    pub public_items: middle::privacy::PublicItems,
    pub reachable: NodeSet,
    pub name: String,
    pub glob_map: Option<GlobMap>,
}


#[derive(Copy, Clone)]
pub enum DtorKind {
    NoDtor,
    TraitDtor(bool)
}

impl DtorKind {
    pub fn is_present(&self) -> bool {
        match *self {
            TraitDtor(..) => true,
            _ => false
        }
    }

    pub fn has_drop_flag(&self) -> bool {
        match self {
            &NoDtor => false,
            &TraitDtor(flag) => flag
        }
    }
}

pub trait IntTypeExt {
    fn to_ty<'tcx>(&self, cx: &ctxt<'tcx>) -> Ty<'tcx>;
    fn i64_to_disr(&self, val: i64) -> Option<Disr>;
    fn u64_to_disr(&self, val: u64) -> Option<Disr>;
    fn disr_incr(&self, val: Disr) -> Option<Disr>;
    fn disr_string(&self, val: Disr) -> String;
    fn disr_wrap_incr(&self, val: Option<Disr>) -> Disr;
}

impl IntTypeExt for attr::IntType {
    fn to_ty<'tcx>(&self, cx: &ctxt<'tcx>) -> Ty<'tcx> {
        match *self {
            SignedInt(hir::TyI8)      => cx.types.i8,
            SignedInt(hir::TyI16)     => cx.types.i16,
            SignedInt(hir::TyI32)     => cx.types.i32,
            SignedInt(hir::TyI64)     => cx.types.i64,
            SignedInt(hir::TyIs)   => cx.types.isize,
            UnsignedInt(hir::TyU8)    => cx.types.u8,
            UnsignedInt(hir::TyU16)   => cx.types.u16,
            UnsignedInt(hir::TyU32)   => cx.types.u32,
            UnsignedInt(hir::TyU64)   => cx.types.u64,
            UnsignedInt(hir::TyUs) => cx.types.usize,
        }
    }

    fn i64_to_disr(&self, val: i64) -> Option<Disr> {
        match *self {
            SignedInt(hir::TyI8)    => val.to_i8()  .map(|v| v as Disr),
            SignedInt(hir::TyI16)   => val.to_i16() .map(|v| v as Disr),
            SignedInt(hir::TyI32)   => val.to_i32() .map(|v| v as Disr),
            SignedInt(hir::TyI64)   => val.to_i64() .map(|v| v as Disr),
            UnsignedInt(hir::TyU8)  => val.to_u8()  .map(|v| v as Disr),
            UnsignedInt(hir::TyU16) => val.to_u16() .map(|v| v as Disr),
            UnsignedInt(hir::TyU32) => val.to_u32() .map(|v| v as Disr),
            UnsignedInt(hir::TyU64) => val.to_u64() .map(|v| v as Disr),

            UnsignedInt(hir::TyUs) |
            SignedInt(hir::TyIs) => unreachable!(),
        }
    }

    fn u64_to_disr(&self, val: u64) -> Option<Disr> {
        match *self {
            SignedInt(hir::TyI8)    => val.to_i8()  .map(|v| v as Disr),
            SignedInt(hir::TyI16)   => val.to_i16() .map(|v| v as Disr),
            SignedInt(hir::TyI32)   => val.to_i32() .map(|v| v as Disr),
            SignedInt(hir::TyI64)   => val.to_i64() .map(|v| v as Disr),
            UnsignedInt(hir::TyU8)  => val.to_u8()  .map(|v| v as Disr),
            UnsignedInt(hir::TyU16) => val.to_u16() .map(|v| v as Disr),
            UnsignedInt(hir::TyU32) => val.to_u32() .map(|v| v as Disr),
            UnsignedInt(hir::TyU64) => val.to_u64() .map(|v| v as Disr),

            UnsignedInt(hir::TyUs) |
            SignedInt(hir::TyIs) => unreachable!(),
        }
    }

    fn disr_incr(&self, val: Disr) -> Option<Disr> {
        macro_rules! add1 {
            ($e:expr) => { $e.and_then(|v|v.checked_add(1)).map(|v| v as Disr) }
        }
        match *self {
            // SignedInt repr means we *want* to reinterpret the bits
            // treating the highest bit of Disr as a sign-bit, so
            // cast to i64 before range-checking.
            SignedInt(hir::TyI8)    => add1!((val as i64).to_i8()),
            SignedInt(hir::TyI16)   => add1!((val as i64).to_i16()),
            SignedInt(hir::TyI32)   => add1!((val as i64).to_i32()),
            SignedInt(hir::TyI64)   => add1!(Some(val as i64)),

            UnsignedInt(hir::TyU8)  => add1!(val.to_u8()),
            UnsignedInt(hir::TyU16) => add1!(val.to_u16()),
            UnsignedInt(hir::TyU32) => add1!(val.to_u32()),
            UnsignedInt(hir::TyU64) => add1!(Some(val)),

            UnsignedInt(hir::TyUs) |
            SignedInt(hir::TyIs) => unreachable!(),
        }
    }

    // This returns a String because (1.) it is only used for
    // rendering an error message and (2.) a string can represent the
    // full range from `i64::MIN` through `u64::MAX`.
    fn disr_string(&self, val: Disr) -> String {
        match *self {
            SignedInt(hir::TyI8)    => format!("{}", val as i8 ),
            SignedInt(hir::TyI16)   => format!("{}", val as i16),
            SignedInt(hir::TyI32)   => format!("{}", val as i32),
            SignedInt(hir::TyI64)   => format!("{}", val as i64),
            UnsignedInt(hir::TyU8)  => format!("{}", val as u8 ),
            UnsignedInt(hir::TyU16) => format!("{}", val as u16),
            UnsignedInt(hir::TyU32) => format!("{}", val as u32),
            UnsignedInt(hir::TyU64) => format!("{}", val as u64),

            UnsignedInt(hir::TyUs) |
            SignedInt(hir::TyIs) => unreachable!(),
        }
    }

    fn disr_wrap_incr(&self, val: Option<Disr>) -> Disr {
        macro_rules! add1 {
            ($e:expr) => { ($e).wrapping_add(1) as Disr }
        }
        let val = val.unwrap_or(ty::INITIAL_DISCRIMINANT_VALUE);
        match *self {
            SignedInt(hir::TyI8)    => add1!(val as i8 ),
            SignedInt(hir::TyI16)   => add1!(val as i16),
            SignedInt(hir::TyI32)   => add1!(val as i32),
            SignedInt(hir::TyI64)   => add1!(val as i64),
            UnsignedInt(hir::TyU8)  => add1!(val as u8 ),
            UnsignedInt(hir::TyU16) => add1!(val as u16),
            UnsignedInt(hir::TyU32) => add1!(val as u32),
            UnsignedInt(hir::TyU64) => add1!(val as u64),

            UnsignedInt(hir::TyUs) |
            SignedInt(hir::TyIs) => unreachable!(),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ImplOrTraitItemContainer {
    TraitContainer(DefId),
    ImplContainer(DefId),
}

impl ImplOrTraitItemContainer {
    pub fn id(&self) -> DefId {
        match *self {
            TraitContainer(id) => id,
            ImplContainer(id) => id,
        }
    }
}

#[derive(Clone)]
pub enum ImplOrTraitItem<'tcx> {
    ConstTraitItem(Rc<AssociatedConst<'tcx>>),
    MethodTraitItem(Rc<Method<'tcx>>),
    TypeTraitItem(Rc<AssociatedType<'tcx>>),
}

impl<'tcx> ImplOrTraitItem<'tcx> {
    fn id(&self) -> ImplOrTraitItemId {
        match *self {
            ConstTraitItem(ref associated_const) => {
                ConstTraitItemId(associated_const.def_id)
            }
            MethodTraitItem(ref method) => MethodTraitItemId(method.def_id),
            TypeTraitItem(ref associated_type) => {
                TypeTraitItemId(associated_type.def_id)
            }
        }
    }

    pub fn def_id(&self) -> DefId {
        match *self {
            ConstTraitItem(ref associated_const) => associated_const.def_id,
            MethodTraitItem(ref method) => method.def_id,
            TypeTraitItem(ref associated_type) => associated_type.def_id,
        }
    }

    pub fn name(&self) -> Name {
        match *self {
            ConstTraitItem(ref associated_const) => associated_const.name,
            MethodTraitItem(ref method) => method.name,
            TypeTraitItem(ref associated_type) => associated_type.name,
        }
    }

    pub fn vis(&self) -> hir::Visibility {
        match *self {
            ConstTraitItem(ref associated_const) => associated_const.vis,
            MethodTraitItem(ref method) => method.vis,
            TypeTraitItem(ref associated_type) => associated_type.vis,
        }
    }

    pub fn container(&self) -> ImplOrTraitItemContainer {
        match *self {
            ConstTraitItem(ref associated_const) => associated_const.container,
            MethodTraitItem(ref method) => method.container,
            TypeTraitItem(ref associated_type) => associated_type.container,
        }
    }

    pub fn as_opt_method(&self) -> Option<Rc<Method<'tcx>>> {
        match *self {
            MethodTraitItem(ref m) => Some((*m).clone()),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum ImplOrTraitItemId {
    ConstTraitItemId(DefId),
    MethodTraitItemId(DefId),
    TypeTraitItemId(DefId),
}

impl ImplOrTraitItemId {
    pub fn def_id(&self) -> DefId {
        match *self {
            ConstTraitItemId(def_id) => def_id,
            MethodTraitItemId(def_id) => def_id,
            TypeTraitItemId(def_id) => def_id,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Method<'tcx> {
    pub name: Name,
    pub generics: Generics<'tcx>,
    pub predicates: GenericPredicates<'tcx>,
    pub fty: BareFnTy<'tcx>,
    pub explicit_self: ExplicitSelfCategory,
    pub vis: hir::Visibility,
    pub def_id: DefId,
    pub container: ImplOrTraitItemContainer,

    // If this method is provided, we need to know where it came from
    pub provided_source: Option<DefId>
}

impl<'tcx> Method<'tcx> {
    pub fn new(name: Name,
               generics: ty::Generics<'tcx>,
               predicates: GenericPredicates<'tcx>,
               fty: BareFnTy<'tcx>,
               explicit_self: ExplicitSelfCategory,
               vis: hir::Visibility,
               def_id: DefId,
               container: ImplOrTraitItemContainer,
               provided_source: Option<DefId>)
               -> Method<'tcx> {
       Method {
            name: name,
            generics: generics,
            predicates: predicates,
            fty: fty,
            explicit_self: explicit_self,
            vis: vis,
            def_id: def_id,
            container: container,
            provided_source: provided_source
        }
    }

    pub fn container_id(&self) -> DefId {
        match self.container {
            TraitContainer(id) => id,
            ImplContainer(id) => id,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct AssociatedConst<'tcx> {
    pub name: Name,
    pub ty: Ty<'tcx>,
    pub vis: hir::Visibility,
    pub def_id: DefId,
    pub container: ImplOrTraitItemContainer,
    pub default: Option<DefId>,
}

#[derive(Clone, Copy, Debug)]
pub struct AssociatedType<'tcx> {
    pub name: Name,
    pub ty: Option<Ty<'tcx>>,
    pub vis: hir::Visibility,
    pub def_id: DefId,
    pub container: ImplOrTraitItemContainer,
}

#[derive(Clone, PartialEq, RustcDecodable, RustcEncodable)]
pub struct ItemVariances {
    pub types: VecPerParamSpace<Variance>,
    pub regions: VecPerParamSpace<Variance>,
}

#[derive(Clone, PartialEq, RustcDecodable, RustcEncodable, Copy)]
pub enum Variance {
    Covariant,      // T<A> <: T<B> iff A <: B -- e.g., function return type
    Invariant,      // T<A> <: T<B> iff B == A -- e.g., type of mutable cell
    Contravariant,  // T<A> <: T<B> iff B <: A -- e.g., function param type
    Bivariant,      // T<A> <: T<B>            -- e.g., unused type parameter
}

#[derive(Copy, Clone)]
pub enum AutoAdjustment<'tcx> {
    AdjustReifyFnPointer,   // go from a fn-item type to a fn-pointer type
    AdjustUnsafeFnPointer,  // go from a safe fn pointer to an unsafe fn pointer
    AdjustDerefRef(AutoDerefRef<'tcx>),
}

/// Represents coercing a pointer to a different kind of pointer - where 'kind'
/// here means either or both of raw vs borrowed vs unique and fat vs thin.
///
/// We transform pointers by following the following steps in order:
/// 1. Deref the pointer `self.autoderefs` times (may be 0).
/// 2. If `autoref` is `Some(_)`, then take the address and produce either a
///    `&` or `*` pointer.
/// 3. If `unsize` is `Some(_)`, then apply the unsize transformation,
///    which will do things like convert thin pointers to fat
///    pointers, or convert structs containing thin pointers to
///    structs containing fat pointers, or convert between fat
///    pointers.  We don't store the details of how the transform is
///    done (in fact, we don't know that, because it might depend on
///    the precise type parameters). We just store the target
///    type. Trans figures out what has to be done at monomorphization
///    time based on the precise source/target type at hand.
///
/// To make that more concrete, here are some common scenarios:
///
/// 1. The simplest cases are where the pointer is not adjusted fat vs thin.
/// Here the pointer will be dereferenced N times (where a dereference can
/// happen to to raw or borrowed pointers or any smart pointer which implements
/// Deref, including Box<_>). The number of dereferences is given by
/// `autoderefs`.  It can then be auto-referenced zero or one times, indicated
/// by `autoref`, to either a raw or borrowed pointer. In these cases unsize is
/// None.
///
/// 2. A thin-to-fat coercon involves unsizing the underlying data. We start
/// with a thin pointer, deref a number of times, unsize the underlying data,
/// then autoref. The 'unsize' phase may change a fixed length array to a
/// dynamically sized one, a concrete object to a trait object, or statically
/// sized struct to a dyncamically sized one. E.g., &[i32; 4] -> &[i32] is
/// represented by:
///
/// ```
/// AutoDerefRef {
///     autoderefs: 1,          // &[i32; 4] -> [i32; 4]
///     autoref: Some(AutoPtr), // [i32] -> &[i32]
///     unsize: Some([i32]),    // [i32; 4] -> [i32]
/// }
/// ```
///
/// Note that for a struct, the 'deep' unsizing of the struct is not recorded.
/// E.g., `struct Foo<T> { x: T }` we can coerce &Foo<[i32; 4]> to &Foo<[i32]>
/// The autoderef and -ref are the same as in the above example, but the type
/// stored in `unsize` is `Foo<[i32]>`, we don't store any further detail about
/// the underlying conversions from `[i32; 4]` to `[i32]`.
///
/// 3. Coercing a `Box<T>` to `Box<Trait>` is an interesting special case.  In
/// that case, we have the pointer we need coming in, so there are no
/// autoderefs, and no autoref. Instead we just do the `Unsize` transformation.
/// At some point, of course, `Box` should move out of the compiler, in which
/// case this is analogous to transformating a struct. E.g., Box<[i32; 4]> ->
/// Box<[i32]> is represented by:
///
/// ```
/// AutoDerefRef {
///     autoderefs: 0,
///     autoref: None,
///     unsize: Some(Box<[i32]>),
/// }
/// ```
#[derive(Copy, Clone)]
pub struct AutoDerefRef<'tcx> {
    /// Step 1. Apply a number of dereferences, producing an lvalue.
    pub autoderefs: usize,

    /// Step 2. Optionally produce a pointer/reference from the value.
    pub autoref: Option<AutoRef<'tcx>>,

    /// Step 3. Unsize a pointer/reference value, e.g. `&[T; n]` to
    /// `&[T]`. The stored type is the target pointer type. Note that
    /// the source could be a thin or fat pointer.
    pub unsize: Option<Ty<'tcx>>,
}

#[derive(Copy, Clone, PartialEq, Debug)]
pub enum AutoRef<'tcx> {
    /// Convert from T to &T.
    AutoPtr(&'tcx Region, hir::Mutability),

    /// Convert from T to *T.
    /// Value to thin pointer.
    AutoUnsafe(hir::Mutability),
}

#[derive(Clone, Copy, RustcEncodable, RustcDecodable, Debug)]
pub enum CustomCoerceUnsized {
    /// Records the index of the field being coerced.
    Struct(usize)
}

#[derive(Clone, Copy, Debug)]
pub struct MethodCallee<'tcx> {
    /// Impl method ID, for inherent methods, or trait method ID, otherwise.
    pub def_id: DefId,
    pub ty: Ty<'tcx>,
    pub substs: &'tcx subst::Substs<'tcx>
}

/// With method calls, we store some extra information in
/// side tables (i.e method_map). We use
/// MethodCall as a key to index into these tables instead of
/// just directly using the expression's NodeId. The reason
/// for this being that we may apply adjustments (coercions)
/// with the resulting expression also needing to use the
/// side tables. The problem with this is that we don't
/// assign a separate NodeId to this new expression
/// and so it would clash with the base expression if both
/// needed to add to the side tables. Thus to disambiguate
/// we also keep track of whether there's an adjustment in
/// our key.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct MethodCall {
    pub expr_id: NodeId,
    pub autoderef: u32
}

impl MethodCall {
    pub fn expr(id: NodeId) -> MethodCall {
        MethodCall {
            expr_id: id,
            autoderef: 0
        }
    }

    pub fn autoderef(expr_id: NodeId, autoderef: u32) -> MethodCall {
        MethodCall {
            expr_id: expr_id,
            autoderef: 1 + autoderef
        }
    }
}

// maps from an expression id that corresponds to a method call to the details
// of the method to be invoked
pub type MethodMap<'tcx> = FnvHashMap<MethodCall, MethodCallee<'tcx>>;

// Contains information needed to resolve types and (in the future) look up
// the types of AST nodes.
#[derive(Copy, Clone, PartialEq, Eq, Hash)]
pub struct CReaderCacheKey {
    pub cnum: CrateNum,
    pub pos: usize,
    pub len: usize
}

/// A restriction that certain types must be the same size. The use of
/// `transmute` gives rise to these restrictions. These generally
/// cannot be checked until trans; therefore, each call to `transmute`
/// will push one or more such restriction into the
/// `transmute_restrictions` vector during `intrinsicck`. They are
/// then checked during `trans` by the fn `check_intrinsics`.
#[derive(Copy, Clone)]
pub struct TransmuteRestriction<'tcx> {
    /// The span whence the restriction comes.
    pub span: Span,

    /// The type being transmuted from.
    pub original_from: Ty<'tcx>,

    /// The type being transmuted to.
    pub original_to: Ty<'tcx>,

    /// The type being transmuted from, with all type parameters
    /// substituted for an arbitrary representative. Not to be shown
    /// to the end user.
    pub substituted_from: Ty<'tcx>,

    /// The type being transmuted to, with all type parameters
    /// substituted for an arbitrary representative. Not to be shown
    /// to the end user.
    pub substituted_to: Ty<'tcx>,

    /// NodeId of the transmute intrinsic.
    pub id: NodeId,
}

/// Describes the fragment-state associated with a NodeId.
///
/// Currently only unfragmented paths have entries in the table,
/// but longer-term this enum is expected to expand to also
/// include data for fragmented paths.
#[derive(Copy, Clone, Debug)]
pub enum FragmentInfo {
    Moved { var: NodeId, move_expr: NodeId },
    Assigned { var: NodeId, assign_expr: NodeId, assignee_id: NodeId },
}

// Flags that we track on types. These flags are propagated upwards
// through the type during type construction, so that we can quickly
// check whether the type has various kinds of types in it without
// recursing over the type itself.
bitflags! {
    flags TypeFlags: u32 {
        const HAS_PARAMS         = 1 << 0,
        const HAS_SELF           = 1 << 1,
        const HAS_TY_INFER       = 1 << 2,
        const HAS_RE_INFER       = 1 << 3,
        const HAS_RE_EARLY_BOUND = 1 << 4,
        const HAS_FREE_REGIONS   = 1 << 5,
        const HAS_TY_ERR         = 1 << 6,
        const HAS_PROJECTION     = 1 << 7,
        const HAS_TY_CLOSURE     = 1 << 8,

        // true if there are "names" of types and regions and so forth
        // that are local to a particular fn
        const HAS_LOCAL_NAMES   = 1 << 9,

        const NEEDS_SUBST        = TypeFlags::HAS_PARAMS.bits |
                                   TypeFlags::HAS_SELF.bits |
                                   TypeFlags::HAS_RE_EARLY_BOUND.bits,

        // Flags representing the nominal content of a type,
        // computed by FlagsComputation. If you add a new nominal
        // flag, it should be added here too.
        const NOMINAL_FLAGS     = TypeFlags::HAS_PARAMS.bits |
                                  TypeFlags::HAS_SELF.bits |
                                  TypeFlags::HAS_TY_INFER.bits |
                                  TypeFlags::HAS_RE_INFER.bits |
                                  TypeFlags::HAS_RE_EARLY_BOUND.bits |
                                  TypeFlags::HAS_FREE_REGIONS.bits |
                                  TypeFlags::HAS_TY_ERR.bits |
                                  TypeFlags::HAS_PROJECTION.bits |
                                  TypeFlags::HAS_TY_CLOSURE.bits |
                                  TypeFlags::HAS_LOCAL_NAMES.bits,

        // Caches for type_is_sized, type_moves_by_default
        const SIZEDNESS_CACHED  = 1 << 16,
        const IS_SIZED          = 1 << 17,
        const MOVENESS_CACHED   = 1 << 18,
        const MOVES_BY_DEFAULT  = 1 << 19,
    }
}

pub struct TyS<'tcx> {
    pub sty: TypeVariants<'tcx>,
    pub flags: Cell<TypeFlags>,

    // the maximal depth of any bound regions appearing in this type.
    region_depth: u32,
}

impl<'tcx> PartialEq for TyS<'tcx> {
    #[inline]
    fn eq(&self, other: &TyS<'tcx>) -> bool {
        // (self as *const _) == (other as *const _)
        (self as *const TyS<'tcx>) == (other as *const TyS<'tcx>)
    }
}
impl<'tcx> Eq for TyS<'tcx> {}

impl<'tcx> Hash for TyS<'tcx> {
    fn hash<H: Hasher>(&self, s: &mut H) {
        (self as *const TyS).hash(s)
    }
}

pub type Ty<'tcx> = &'tcx TyS<'tcx>;

/// Upvars do not get their own node-id. Instead, we use the pair of
/// the original var id (that is, the root variable that is referenced
/// by the upvar) and the id of the closure expression.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct UpvarId {
    pub var_id: NodeId,
    pub closure_expr_id: NodeId,
}

#[derive(Clone, PartialEq, Eq, Hash, Debug, RustcEncodable, RustcDecodable, Copy)]
pub enum BorrowKind {
    /// Data must be immutable and is aliasable.
    ImmBorrow,

    /// Data must be immutable but not aliasable.  This kind of borrow
    /// cannot currently be expressed by the user and is used only in
    /// implicit closure bindings. It is needed when you the closure
    /// is borrowing or mutating a mutable referent, e.g.:
    ///
    ///    let x: &mut isize = ...;
    ///    let y = || *x += 5;
    ///
    /// If we were to try to translate this closure into a more explicit
    /// form, we'd encounter an error with the code as written:
    ///
    ///    struct Env { x: & &mut isize }
    ///    let x: &mut isize = ...;
    ///    let y = (&mut Env { &x }, fn_ptr);  // Closure is pair of env and fn
    ///    fn fn_ptr(env: &mut Env) { **env.x += 5; }
    ///
    /// This is then illegal because you cannot mutate a `&mut` found
    /// in an aliasable location. To solve, you'd have to translate with
    /// an `&mut` borrow:
    ///
    ///    struct Env { x: & &mut isize }
    ///    let x: &mut isize = ...;
    ///    let y = (&mut Env { &mut x }, fn_ptr); // changed from &x to &mut x
    ///    fn fn_ptr(env: &mut Env) { **env.x += 5; }
    ///
    /// Now the assignment to `**env.x` is legal, but creating a
    /// mutable pointer to `x` is not because `x` is not mutable. We
    /// could fix this by declaring `x` as `let mut x`. This is ok in
    /// user code, if awkward, but extra weird for closures, since the
    /// borrow is hidden.
    ///
    /// So we introduce a "unique imm" borrow -- the referent is
    /// immutable, but not aliasable. This solves the problem. For
    /// simplicity, we don't give users the way to express this
    /// borrow, it's just used when translating closures.
    UniqueImmBorrow,

    /// Data is mutable and not aliasable.
    MutBorrow
}

/// Information describing the capture of an upvar. This is computed
/// during `typeck`, specifically by `regionck`.
#[derive(PartialEq, Clone, Debug, Copy)]
pub enum UpvarCapture {
    /// Upvar is captured by value. This is always true when the
    /// closure is labeled `move`, but can also be true in other cases
    /// depending on inference.
    ByValue,

    /// Upvar is captured by reference.
    ByRef(UpvarBorrow),
}

#[derive(PartialEq, Clone, Copy)]
pub struct UpvarBorrow {
    /// The kind of borrow: by-ref upvars have access to shared
    /// immutable borrows, which are not part of the normal language
    /// syntax.
    pub kind: BorrowKind,

    /// Region of the resulting reference.
    pub region: ty::Region,
}

pub type UpvarCaptureMap = FnvHashMap<UpvarId, UpvarCapture>;

#[derive(Copy, Clone)]
pub struct ClosureUpvar<'tcx> {
    pub def: def::Def,
    pub span: Span,
    pub ty: Ty<'tcx>,
}

#[derive(Clone, Copy, PartialEq)]
pub enum IntVarValue {
    IntType(hir::IntTy),
    UintType(hir::UintTy),
}

/// Default region to use for the bound of objects that are
/// supplied as the value for this type parameter. This is derived
/// from `T:'a` annotations appearing in the type definition.  If
/// this is `None`, then the default is inherited from the
/// surrounding context. See RFC #599 for details.
#[derive(Copy, Clone)]
pub enum ObjectLifetimeDefault {
    /// Require an explicit annotation. Occurs when multiple
    /// `T:'a` constraints are found.
    Ambiguous,

    /// Use the base default, typically 'static, but in a fn body it is a fresh variable
    BaseDefault,

    /// Use the given region as the default.
    Specific(Region),
}

#[derive(Clone)]
pub struct TypeParameterDef<'tcx> {
    pub name: Name,
    pub def_id: DefId,
    pub space: subst::ParamSpace,
    pub index: u32,
    pub default_def_id: DefId, // for use in error reporing about defaults
    pub default: Option<Ty<'tcx>>,
    pub object_lifetime_default: ObjectLifetimeDefault,
}

#[derive(Clone)]
pub struct RegionParameterDef {
    pub name: Name,
    pub def_id: DefId,
    pub space: subst::ParamSpace,
    pub index: u32,
    pub bounds: Vec<ty::Region>,
}

impl RegionParameterDef {
    pub fn to_early_bound_region(&self) -> ty::Region {
        ty::ReEarlyBound(ty::EarlyBoundRegion {
            param_id: self.def_id.node,
            space: self.space,
            index: self.index,
            name: self.name,
        })
    }
    pub fn to_bound_region(&self) -> ty::BoundRegion {
        ty::BoundRegion::BrNamed(self.def_id, self.name)
    }
}

/// Information about the formal type/lifetime parameters associated
/// with an item or method. Analogous to hir::Generics.
#[derive(Clone, Debug)]
pub struct Generics<'tcx> {
    pub types: VecPerParamSpace<TypeParameterDef<'tcx>>,
    pub regions: VecPerParamSpace<RegionParameterDef>,
}

impl<'tcx> Generics<'tcx> {
    pub fn empty() -> Generics<'tcx> {
        Generics {
            types: VecPerParamSpace::empty(),
            regions: VecPerParamSpace::empty(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.types.is_empty() && self.regions.is_empty()
    }

    pub fn has_type_params(&self, space: subst::ParamSpace) -> bool {
        !self.types.is_empty_in(space)
    }

    pub fn has_region_params(&self, space: subst::ParamSpace) -> bool {
        !self.regions.is_empty_in(space)
    }
}

/// Bounds on generics.
#[derive(Clone)]
pub struct GenericPredicates<'tcx> {
    pub predicates: VecPerParamSpace<Predicate<'tcx>>,
}

impl<'tcx> GenericPredicates<'tcx> {
    pub fn empty() -> GenericPredicates<'tcx> {
        GenericPredicates {
            predicates: VecPerParamSpace::empty(),
        }
    }

    pub fn instantiate(&self, tcx: &ctxt<'tcx>, substs: &Substs<'tcx>)
                       -> InstantiatedPredicates<'tcx> {
        InstantiatedPredicates {
            predicates: self.predicates.subst(tcx, substs),
        }
    }

    pub fn instantiate_supertrait(&self,
                                  tcx: &ctxt<'tcx>,
                                  poly_trait_ref: &ty::PolyTraitRef<'tcx>)
                                  -> InstantiatedPredicates<'tcx>
    {
        InstantiatedPredicates {
            predicates: self.predicates.map(|pred| pred.subst_supertrait(tcx, poly_trait_ref))
        }
    }
}

#[derive(Clone, PartialEq, Eq, Hash)]
pub enum Predicate<'tcx> {
    /// Corresponds to `where Foo : Bar<A,B,C>`. `Foo` here would be
    /// the `Self` type of the trait reference and `A`, `B`, and `C`
    /// would be the parameters in the `TypeSpace`.
    Trait(PolyTraitPredicate<'tcx>),

    /// where `T1 == T2`.
    Equate(PolyEquatePredicate<'tcx>),

    /// where 'a : 'b
    RegionOutlives(PolyRegionOutlivesPredicate),

    /// where T : 'a
    TypeOutlives(PolyTypeOutlivesPredicate<'tcx>),

    /// where <T as TraitRef>::Name == X, approximately.
    /// See `ProjectionPredicate` struct for details.
    Projection(PolyProjectionPredicate<'tcx>),

    /// no syntax: T WF
    WellFormed(Ty<'tcx>),

    /// trait must be object-safe
    ObjectSafe(DefId),
}

impl<'tcx> Predicate<'tcx> {
    /// Performs a substitution suitable for going from a
    /// poly-trait-ref to supertraits that must hold if that
    /// poly-trait-ref holds. This is slightly different from a normal
    /// substitution in terms of what happens with bound regions.  See
    /// lengthy comment below for details.
    pub fn subst_supertrait(&self,
                            tcx: &ctxt<'tcx>,
                            trait_ref: &ty::PolyTraitRef<'tcx>)
                            -> ty::Predicate<'tcx>
    {
        // The interaction between HRTB and supertraits is not entirely
        // obvious. Let me walk you (and myself) through an example.
        //
        // Let's start with an easy case. Consider two traits:
        //
        //     trait Foo<'a> : Bar<'a,'a> { }
        //     trait Bar<'b,'c> { }
        //
        // Now, if we have a trait reference `for<'x> T : Foo<'x>`, then
        // we can deduce that `for<'x> T : Bar<'x,'x>`. Basically, if we
        // knew that `Foo<'x>` (for any 'x) then we also know that
        // `Bar<'x,'x>` (for any 'x). This more-or-less falls out from
        // normal substitution.
        //
        // In terms of why this is sound, the idea is that whenever there
        // is an impl of `T:Foo<'a>`, it must show that `T:Bar<'a,'a>`
        // holds.  So if there is an impl of `T:Foo<'a>` that applies to
        // all `'a`, then we must know that `T:Bar<'a,'a>` holds for all
        // `'a`.
        //
        // Another example to be careful of is this:
        //
        //     trait Foo1<'a> : for<'b> Bar1<'a,'b> { }
        //     trait Bar1<'b,'c> { }
        //
        // Here, if we have `for<'x> T : Foo1<'x>`, then what do we know?
        // The answer is that we know `for<'x,'b> T : Bar1<'x,'b>`. The
        // reason is similar to the previous example: any impl of
        // `T:Foo1<'x>` must show that `for<'b> T : Bar1<'x, 'b>`.  So
        // basically we would want to collapse the bound lifetimes from
        // the input (`trait_ref`) and the supertraits.
        //
        // To achieve this in practice is fairly straightforward. Let's
        // consider the more complicated scenario:
        //
        // - We start out with `for<'x> T : Foo1<'x>`. In this case, `'x`
        //   has a De Bruijn index of 1. We want to produce `for<'x,'b> T : Bar1<'x,'b>`,
        //   where both `'x` and `'b` would have a DB index of 1.
        //   The substitution from the input trait-ref is therefore going to be
        //   `'a => 'x` (where `'x` has a DB index of 1).
        // - The super-trait-ref is `for<'b> Bar1<'a,'b>`, where `'a` is an
        //   early-bound parameter and `'b' is a late-bound parameter with a
        //   DB index of 1.
        // - If we replace `'a` with `'x` from the input, it too will have
        //   a DB index of 1, and thus we'll have `for<'x,'b> Bar1<'x,'b>`
        //   just as we wanted.
        //
        // There is only one catch. If we just apply the substitution `'a
        // => 'x` to `for<'b> Bar1<'a,'b>`, the substitution code will
        // adjust the DB index because we substituting into a binder (it
        // tries to be so smart...) resulting in `for<'x> for<'b>
        // Bar1<'x,'b>` (we have no syntax for this, so use your
        // imagination). Basically the 'x will have DB index of 2 and 'b
        // will have DB index of 1. Not quite what we want. So we apply
        // the substitution to the *contents* of the trait reference,
        // rather than the trait reference itself (put another way, the
        // substitution code expects equal binding levels in the values
        // from the substitution and the value being substituted into, and
        // this trick achieves that).

        let substs = &trait_ref.0.substs;
        match *self {
            Predicate::Trait(ty::Binder(ref data)) =>
                Predicate::Trait(ty::Binder(data.subst(tcx, substs))),
            Predicate::Equate(ty::Binder(ref data)) =>
                Predicate::Equate(ty::Binder(data.subst(tcx, substs))),
            Predicate::RegionOutlives(ty::Binder(ref data)) =>
                Predicate::RegionOutlives(ty::Binder(data.subst(tcx, substs))),
            Predicate::TypeOutlives(ty::Binder(ref data)) =>
                Predicate::TypeOutlives(ty::Binder(data.subst(tcx, substs))),
            Predicate::Projection(ty::Binder(ref data)) =>
                Predicate::Projection(ty::Binder(data.subst(tcx, substs))),
            Predicate::WellFormed(data) =>
                Predicate::WellFormed(data.subst(tcx, substs)),
            Predicate::ObjectSafe(trait_def_id) =>
                Predicate::ObjectSafe(trait_def_id),
        }
    }
}

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct TraitPredicate<'tcx> {
    pub trait_ref: TraitRef<'tcx>
}
pub type PolyTraitPredicate<'tcx> = ty::Binder<TraitPredicate<'tcx>>;

impl<'tcx> TraitPredicate<'tcx> {
    pub fn def_id(&self) -> DefId {
        self.trait_ref.def_id
    }

    pub fn input_types(&self) -> &[Ty<'tcx>] {
        self.trait_ref.substs.types.as_slice()
    }

    pub fn self_ty(&self) -> Ty<'tcx> {
        self.trait_ref.self_ty()
    }
}

impl<'tcx> PolyTraitPredicate<'tcx> {
    pub fn def_id(&self) -> DefId {
        self.0.def_id()
    }
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct EquatePredicate<'tcx>(pub Ty<'tcx>, pub Ty<'tcx>); // `0 == 1`
pub type PolyEquatePredicate<'tcx> = ty::Binder<EquatePredicate<'tcx>>;

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct OutlivesPredicate<A,B>(pub A, pub B); // `A : B`
pub type PolyOutlivesPredicate<A,B> = ty::Binder<OutlivesPredicate<A,B>>;
pub type PolyRegionOutlivesPredicate = PolyOutlivesPredicate<ty::Region, ty::Region>;
pub type PolyTypeOutlivesPredicate<'tcx> = PolyOutlivesPredicate<Ty<'tcx>, ty::Region>;

/// This kind of predicate has no *direct* correspondent in the
/// syntax, but it roughly corresponds to the syntactic forms:
///
/// 1. `T : TraitRef<..., Item=Type>`
/// 2. `<T as TraitRef<...>>::Item == Type` (NYI)
///
/// In particular, form #1 is "desugared" to the combination of a
/// normal trait predicate (`T : TraitRef<...>`) and one of these
/// predicates. Form #2 is a broader form in that it also permits
/// equality between arbitrary types. Processing an instance of Form
/// #2 eventually yields one of these `ProjectionPredicate`
/// instances to normalize the LHS.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct ProjectionPredicate<'tcx> {
    pub projection_ty: ProjectionTy<'tcx>,
    pub ty: Ty<'tcx>,
}

pub type PolyProjectionPredicate<'tcx> = Binder<ProjectionPredicate<'tcx>>;

impl<'tcx> PolyProjectionPredicate<'tcx> {
    pub fn item_name(&self) -> Name {
        self.0.projection_ty.item_name // safe to skip the binder to access a name
    }

    pub fn sort_key(&self) -> (DefId, Name) {
        self.0.projection_ty.sort_key()
    }
}

pub trait ToPolyTraitRef<'tcx> {
    fn to_poly_trait_ref(&self) -> PolyTraitRef<'tcx>;
}

impl<'tcx> ToPolyTraitRef<'tcx> for TraitRef<'tcx> {
    fn to_poly_trait_ref(&self) -> PolyTraitRef<'tcx> {
        assert!(!self.has_escaping_regions());
        ty::Binder(self.clone())
    }
}

impl<'tcx> ToPolyTraitRef<'tcx> for PolyTraitPredicate<'tcx> {
    fn to_poly_trait_ref(&self) -> PolyTraitRef<'tcx> {
        self.map_bound_ref(|trait_pred| trait_pred.trait_ref.clone())
    }
}

impl<'tcx> ToPolyTraitRef<'tcx> for PolyProjectionPredicate<'tcx> {
    fn to_poly_trait_ref(&self) -> PolyTraitRef<'tcx> {
        // Note: unlike with TraitRef::to_poly_trait_ref(),
        // self.0.trait_ref is permitted to have escaping regions.
        // This is because here `self` has a `Binder` and so does our
        // return value, so we are preserving the number of binding
        // levels.
        ty::Binder(self.0.projection_ty.trait_ref.clone())
    }
}

pub trait ToPredicate<'tcx> {
    fn to_predicate(&self) -> Predicate<'tcx>;
}

impl<'tcx> ToPredicate<'tcx> for TraitRef<'tcx> {
    fn to_predicate(&self) -> Predicate<'tcx> {
        // we're about to add a binder, so let's check that we don't
        // accidentally capture anything, or else that might be some
        // weird debruijn accounting.
        assert!(!self.has_escaping_regions());

        ty::Predicate::Trait(ty::Binder(ty::TraitPredicate {
            trait_ref: self.clone()
        }))
    }
}

impl<'tcx> ToPredicate<'tcx> for PolyTraitRef<'tcx> {
    fn to_predicate(&self) -> Predicate<'tcx> {
        ty::Predicate::Trait(self.to_poly_trait_predicate())
    }
}

impl<'tcx> ToPredicate<'tcx> for PolyEquatePredicate<'tcx> {
    fn to_predicate(&self) -> Predicate<'tcx> {
        Predicate::Equate(self.clone())
    }
}

impl<'tcx> ToPredicate<'tcx> for PolyRegionOutlivesPredicate {
    fn to_predicate(&self) -> Predicate<'tcx> {
        Predicate::RegionOutlives(self.clone())
    }
}

impl<'tcx> ToPredicate<'tcx> for PolyTypeOutlivesPredicate<'tcx> {
    fn to_predicate(&self) -> Predicate<'tcx> {
        Predicate::TypeOutlives(self.clone())
    }
}

impl<'tcx> ToPredicate<'tcx> for PolyProjectionPredicate<'tcx> {
    fn to_predicate(&self) -> Predicate<'tcx> {
        Predicate::Projection(self.clone())
    }
}

impl<'tcx> Predicate<'tcx> {
    /// Iterates over the types in this predicate. Note that in all
    /// cases this is skipping over a binder, so late-bound regions
    /// with depth 0 are bound by the predicate.
    pub fn walk_tys(&self) -> IntoIter<Ty<'tcx>> {
        let vec: Vec<_> = match *self {
            ty::Predicate::Trait(ref data) => {
                data.0.trait_ref.substs.types.as_slice().to_vec()
            }
            ty::Predicate::Equate(ty::Binder(ref data)) => {
                vec![data.0, data.1]
            }
            ty::Predicate::TypeOutlives(ty::Binder(ref data)) => {
                vec![data.0]
            }
            ty::Predicate::RegionOutlives(..) => {
                vec![]
            }
            ty::Predicate::Projection(ref data) => {
                let trait_inputs = data.0.projection_ty.trait_ref.substs.types.as_slice();
                trait_inputs.iter()
                            .cloned()
                            .chain(Some(data.0.ty))
                            .collect()
            }
            ty::Predicate::WellFormed(data) => {
                vec![data]
            }
            ty::Predicate::ObjectSafe(_trait_def_id) => {
                vec![]
            }
        };

        // The only reason to collect into a vector here is that I was
        // too lazy to make the full (somewhat complicated) iterator
        // type that would be needed here. But I wanted this fn to
        // return an iterator conceptually, rather than a `Vec`, so as
        // to be closer to `Ty::walk`.
        vec.into_iter()
    }

    pub fn has_escaping_regions(&self) -> bool {
        match *self {
            Predicate::Trait(ref trait_ref) => trait_ref.has_escaping_regions(),
            Predicate::Equate(ref p) => p.has_escaping_regions(),
            Predicate::RegionOutlives(ref p) => p.has_escaping_regions(),
            Predicate::TypeOutlives(ref p) => p.has_escaping_regions(),
            Predicate::Projection(ref p) => p.has_escaping_regions(),
            Predicate::WellFormed(p) => p.has_escaping_regions(),
            Predicate::ObjectSafe(_trait_def_id) => false,
        }
    }

    pub fn to_opt_poly_trait_ref(&self) -> Option<PolyTraitRef<'tcx>> {
        match *self {
            Predicate::Trait(ref t) => {
                Some(t.to_poly_trait_ref())
            }
            Predicate::Projection(..) |
            Predicate::Equate(..) |
            Predicate::RegionOutlives(..) |
            Predicate::WellFormed(..) |
            Predicate::ObjectSafe(..) |
            Predicate::TypeOutlives(..) => {
                None
            }
        }
    }
}

/// Represents the bounds declared on a particular set of type
/// parameters.  Should eventually be generalized into a flag list of
/// where clauses.  You can obtain a `InstantiatedPredicates` list from a
/// `GenericPredicates` by using the `instantiate` method. Note that this method
/// reflects an important semantic invariant of `InstantiatedPredicates`: while
/// the `GenericPredicates` are expressed in terms of the bound type
/// parameters of the impl/trait/whatever, an `InstantiatedPredicates` instance
/// represented a set of bounds for some particular instantiation,
/// meaning that the generic parameters have been substituted with
/// their values.
///
/// Example:
///
///     struct Foo<T,U:Bar<T>> { ... }
///
/// Here, the `GenericPredicates` for `Foo` would contain a list of bounds like
/// `[[], [U:Bar<T>]]`.  Now if there were some particular reference
/// like `Foo<isize,usize>`, then the `InstantiatedPredicates` would be `[[],
/// [usize:Bar<isize>]]`.
#[derive(Clone)]
pub struct InstantiatedPredicates<'tcx> {
    pub predicates: VecPerParamSpace<Predicate<'tcx>>,
}

impl<'tcx> InstantiatedPredicates<'tcx> {
    pub fn empty() -> InstantiatedPredicates<'tcx> {
        InstantiatedPredicates { predicates: VecPerParamSpace::empty() }
    }

    pub fn has_escaping_regions(&self) -> bool {
        self.predicates.any(|p| p.has_escaping_regions())
    }

    pub fn is_empty(&self) -> bool {
        self.predicates.is_empty()
    }
}

impl<'tcx> TraitRef<'tcx> {
    pub fn new(def_id: DefId, substs: &'tcx Substs<'tcx>) -> TraitRef<'tcx> {
        TraitRef { def_id: def_id, substs: substs }
    }

    pub fn self_ty(&self) -> Ty<'tcx> {
        self.substs.self_ty().unwrap()
    }

    pub fn input_types(&self) -> &[Ty<'tcx>] {
        // Select only the "input types" from a trait-reference. For
        // now this is all the types that appear in the
        // trait-reference, but it should eventually exclude
        // associated types.
        self.substs.types.as_slice()
    }
}

/// When type checking, we use the `ParameterEnvironment` to track
/// details about the type/lifetime parameters that are in scope.
/// It primarily stores the bounds information.
///
/// Note: This information might seem to be redundant with the data in
/// `tcx.ty_param_defs`, but it is not. That table contains the
/// parameter definitions from an "outside" perspective, but this
/// struct will contain the bounds for a parameter as seen from inside
/// the function body. Currently the only real distinction is that
/// bound lifetime parameters are replaced with free ones, but in the
/// future I hope to refine the representation of types so as to make
/// more distinctions clearer.
#[derive(Clone)]
pub struct ParameterEnvironment<'a, 'tcx:'a> {
    pub tcx: &'a ctxt<'tcx>,

    /// See `construct_free_substs` for details.
    pub free_substs: Substs<'tcx>,

    /// Each type parameter has an implicit region bound that
    /// indicates it must outlive at least the function body (the user
    /// may specify stronger requirements). This field indicates the
    /// region of the callee.
    pub implicit_region_bound: ty::Region,

    /// Obligations that the caller must satisfy. This is basically
    /// the set of bounds on the in-scope type parameters, translated
    /// into Obligations, and elaborated and normalized.
    pub caller_bounds: Vec<ty::Predicate<'tcx>>,

    /// Caches the results of trait selection. This cache is used
    /// for things that have to do with the parameters in scope.
    pub selection_cache: traits::SelectionCache<'tcx>,

    /// Scope that is attached to free regions for this scope. This
    /// is usually the id of the fn body, but for more abstract scopes
    /// like structs we often use the node-id of the struct.
    ///
    /// FIXME(#3696). It would be nice to refactor so that free
    /// regions don't have this implicit scope and instead introduce
    /// relationships in the environment.
    pub free_id: ast::NodeId,
}

impl<'a, 'tcx> ParameterEnvironment<'a, 'tcx> {
    pub fn with_caller_bounds(&self,
                              caller_bounds: Vec<ty::Predicate<'tcx>>)
                              -> ParameterEnvironment<'a,'tcx>
    {
        ParameterEnvironment {
            tcx: self.tcx,
            free_substs: self.free_substs.clone(),
            implicit_region_bound: self.implicit_region_bound,
            caller_bounds: caller_bounds,
            selection_cache: traits::SelectionCache::new(),
            free_id: self.free_id,
        }
    }

    pub fn for_item(cx: &'a ctxt<'tcx>, id: NodeId) -> ParameterEnvironment<'a, 'tcx> {
        match cx.map.find(id) {
            Some(ast_map::NodeImplItem(ref impl_item)) => {
                match impl_item.node {
                    hir::TypeImplItem(_) => {
                        // associated types don't have their own entry (for some reason),
                        // so for now just grab environment for the impl
                        let impl_id = cx.map.get_parent(id);
                        let impl_def_id = DefId::local(impl_id);
                        let scheme = cx.lookup_item_type(impl_def_id);
                        let predicates = cx.lookup_predicates(impl_def_id);
                        cx.construct_parameter_environment(impl_item.span,
                                                           &scheme.generics,
                                                           &predicates,
                                                           id)
                    }
                    hir::ConstImplItem(_, _) => {
                        let def_id = DefId::local(id);
                        let scheme = cx.lookup_item_type(def_id);
                        let predicates = cx.lookup_predicates(def_id);
                        cx.construct_parameter_environment(impl_item.span,
                                                           &scheme.generics,
                                                           &predicates,
                                                           id)
                    }
                    hir::MethodImplItem(_, ref body) => {
                        let method_def_id = DefId::local(id);
                        match cx.impl_or_trait_item(method_def_id) {
                            MethodTraitItem(ref method_ty) => {
                                let method_generics = &method_ty.generics;
                                let method_bounds = &method_ty.predicates;
                                cx.construct_parameter_environment(
                                    impl_item.span,
                                    method_generics,
                                    method_bounds,
                                    body.id)
                            }
                            _ => {
                                cx.sess
                                  .bug("ParameterEnvironment::for_item(): \
                                        got non-method item from impl method?!")
                            }
                        }
                    }
                }
            }
            Some(ast_map::NodeTraitItem(trait_item)) => {
                match trait_item.node {
                    hir::TypeTraitItem(..) => {
                        // associated types don't have their own entry (for some reason),
                        // so for now just grab environment for the trait
                        let trait_id = cx.map.get_parent(id);
                        let trait_def_id = DefId::local(trait_id);
                        let trait_def = cx.lookup_trait_def(trait_def_id);
                        let predicates = cx.lookup_predicates(trait_def_id);
                        cx.construct_parameter_environment(trait_item.span,
                                                           &trait_def.generics,
                                                           &predicates,
                                                           id)
                    }
                    hir::ConstTraitItem(..) => {
                        let def_id = DefId::local(id);
                        let scheme = cx.lookup_item_type(def_id);
                        let predicates = cx.lookup_predicates(def_id);
                        cx.construct_parameter_environment(trait_item.span,
                                                           &scheme.generics,
                                                           &predicates,
                                                           id)
                    }
                    hir::MethodTraitItem(_, ref body) => {
                        // for the body-id, use the id of the body
                        // block, unless this is a trait method with
                        // no default, then fallback to the method id.
                        let body_id = body.as_ref().map(|b| b.id).unwrap_or(id);
                        let method_def_id = DefId::local(id);

                        match cx.impl_or_trait_item(method_def_id) {
                            MethodTraitItem(ref method_ty) => {
                                let method_generics = &method_ty.generics;
                                let method_bounds = &method_ty.predicates;
                                cx.construct_parameter_environment(
                                    trait_item.span,
                                    method_generics,
                                    method_bounds,
                                    body_id)
                            }
                            _ => {
                                cx.sess
                                  .bug("ParameterEnvironment::for_item(): \
                                        got non-method item from provided \
                                        method?!")
                            }
                        }
                    }
                }
            }
            Some(ast_map::NodeItem(item)) => {
                match item.node {
                    hir::ItemFn(_, _, _, _, _, ref body) => {
                        // We assume this is a function.
                        let fn_def_id = DefId::local(id);
                        let fn_scheme = cx.lookup_item_type(fn_def_id);
                        let fn_predicates = cx.lookup_predicates(fn_def_id);

                        cx.construct_parameter_environment(item.span,
                                                           &fn_scheme.generics,
                                                           &fn_predicates,
                                                           body.id)
                    }
                    hir::ItemEnum(..) |
                    hir::ItemStruct(..) |
                    hir::ItemImpl(..) |
                    hir::ItemConst(..) |
                    hir::ItemStatic(..) => {
                        let def_id = DefId::local(id);
                        let scheme = cx.lookup_item_type(def_id);
                        let predicates = cx.lookup_predicates(def_id);
                        cx.construct_parameter_environment(item.span,
                                                           &scheme.generics,
                                                           &predicates,
                                                           id)
                    }
                    hir::ItemTrait(..) => {
                        let def_id = DefId::local(id);
                        let trait_def = cx.lookup_trait_def(def_id);
                        let predicates = cx.lookup_predicates(def_id);
                        cx.construct_parameter_environment(item.span,
                                                           &trait_def.generics,
                                                           &predicates,
                                                           id)
                    }
                    _ => {
                        cx.sess.span_bug(item.span,
                                         "ParameterEnvironment::from_item():
                                          can't create a parameter \
                                          environment for this kind of item")
                    }
                }
            }
            Some(ast_map::NodeExpr(..)) => {
                // This is a convenience to allow closures to work.
                ParameterEnvironment::for_item(cx, cx.map.get_parent(id))
            }
            _ => {
                cx.sess.bug(&format!("ParameterEnvironment::from_item(): \
                                     `{}` is not an item",
                                    cx.map.node_to_string(id)))
            }
        }
    }

    pub fn can_type_implement_copy(&self, self_type: Ty<'tcx>, span: Span)
                                   -> Result<(),CopyImplementationError> {
        let tcx = self.tcx;

        // FIXME: (@jroesch) float this code up
        let infcx = infer::new_infer_ctxt(tcx, &tcx.tables, Some(self.clone()), false);

        let adt = match self_type.sty {
            ty::TyStruct(struct_def, substs) => {
                for field in struct_def.all_fields() {
                    let field_ty = field.ty(tcx, substs);
                    if infcx.type_moves_by_default(field_ty, span) {
                        return Err(FieldDoesNotImplementCopy(field.name))
                    }
                }
                struct_def
            }
            ty::TyEnum(enum_def, substs) => {
                for variant in &enum_def.variants {
                    for field in &variant.fields {
                        let field_ty = field.ty(tcx, substs);
                        if infcx.type_moves_by_default(field_ty, span) {
                            return Err(VariantDoesNotImplementCopy(variant.name))
                        }
                    }
                }
                enum_def
            }
            _ => return Err(TypeIsStructural),
        };

        if adt.has_dtor() {
            return Err(TypeHasDestructor)
        }

        Ok(())
    }
}

#[derive(Copy, Clone)]
pub enum CopyImplementationError {
    FieldDoesNotImplementCopy(Name),
    VariantDoesNotImplementCopy(Name),
    TypeIsStructural,
    TypeHasDestructor,
}

/// A "type scheme", in ML terminology, is a type combined with some
/// set of generic types that the type is, well, generic over. In Rust
/// terms, it is the "type" of a fn item or struct -- this type will
/// include various generic parameters that must be substituted when
/// the item/struct is referenced. That is called converting the type
/// scheme to a monotype.
///
/// - `generics`: the set of type parameters and their bounds
/// - `ty`: the base types, which may reference the parameters defined
///   in `generics`
///
/// Note that TypeSchemes are also sometimes called "polytypes" (and
/// in fact this struct used to carry that name, so you may find some
/// stray references in a comment or something). We try to reserve the
/// "poly" prefix to refer to higher-ranked things, as in
/// `PolyTraitRef`.
///
/// Note that each item also comes with predicates, see
/// `lookup_predicates`.
#[derive(Clone, Debug)]
pub struct TypeScheme<'tcx> {
    pub generics: Generics<'tcx>,
    pub ty: Ty<'tcx>,
}

bitflags! {
    flags TraitFlags: u32 {
        const NO_TRAIT_FLAGS        = 0,
        const HAS_DEFAULT_IMPL      = 1 << 0,
        const IS_OBJECT_SAFE        = 1 << 1,
        const OBJECT_SAFETY_VALID   = 1 << 2,
        const IMPLS_VALID           = 1 << 3,
    }
}

/// As `TypeScheme` but for a trait ref.
pub struct TraitDef<'tcx> {
    pub unsafety: hir::Unsafety,

    /// If `true`, then this trait had the `#[rustc_paren_sugar]`
    /// attribute, indicating that it should be used with `Foo()`
    /// sugar. This is a temporary thing -- eventually any trait wil
    /// be usable with the sugar (or without it).
    pub paren_sugar: bool,

    /// Generic type definitions. Note that `Self` is listed in here
    /// as having a single bound, the trait itself (e.g., in the trait
    /// `Eq`, there is a single bound `Self : Eq`). This is so that
    /// default methods get to assume that the `Self` parameters
    /// implements the trait.
    pub generics: Generics<'tcx>,

    pub trait_ref: TraitRef<'tcx>,

    /// A list of the associated types defined in this trait. Useful
    /// for resolving `X::Foo` type markers.
    pub associated_type_names: Vec<Name>,

    // Impls of this trait. To allow for quicker lookup, the impls are indexed
    // by a simplified version of their Self type: impls with a simplifiable
    // Self are stored in nonblanket_impls keyed by it, while all other impls
    // are stored in blanket_impls.

    /// Impls of the trait.
    pub nonblanket_impls: RefCell<
        FnvHashMap<fast_reject::SimplifiedType, Vec<DefId>>
    >,

    /// Blanket impls associated with the trait.
    pub blanket_impls: RefCell<Vec<DefId>>,

    /// Various flags
    pub flags: Cell<TraitFlags>
}

impl<'tcx> TraitDef<'tcx> {
    // returns None if not yet calculated
    pub fn object_safety(&self) -> Option<bool> {
        if self.flags.get().intersects(TraitFlags::OBJECT_SAFETY_VALID) {
            Some(self.flags.get().intersects(TraitFlags::IS_OBJECT_SAFE))
        } else {
            None
        }
    }

    pub fn set_object_safety(&self, is_safe: bool) {
        assert!(self.object_safety().map(|cs| cs == is_safe).unwrap_or(true));
        self.flags.set(
            self.flags.get() | if is_safe {
                TraitFlags::OBJECT_SAFETY_VALID | TraitFlags::IS_OBJECT_SAFE
            } else {
                TraitFlags::OBJECT_SAFETY_VALID
            }
        );
    }

    /// Records a trait-to-implementation mapping.
    pub fn record_impl(&self,
                       tcx: &ctxt<'tcx>,
                       impl_def_id: DefId,
                       impl_trait_ref: TraitRef<'tcx>) {
        debug!("TraitDef::record_impl for {:?}, from {:?}",
               self, impl_trait_ref);

        // We don't want to borrow_mut after we already populated all impls,
        // so check if an impl is present with an immutable borrow first.
        if let Some(sty) = fast_reject::simplify_type(tcx,
                                                      impl_trait_ref.self_ty(), false) {
            if let Some(is) = self.nonblanket_impls.borrow().get(&sty) {
                if is.contains(&impl_def_id) {
                    return // duplicate - skip
                }
            }

            self.nonblanket_impls.borrow_mut().entry(sty).or_insert(vec![]).push(impl_def_id)
        } else {
            if self.blanket_impls.borrow().contains(&impl_def_id) {
                return // duplicate - skip
            }
            self.blanket_impls.borrow_mut().push(impl_def_id)
        }
    }


    pub fn for_each_impl<F: FnMut(DefId)>(&self, tcx: &ctxt<'tcx>, mut f: F)  {
        tcx.populate_implementations_for_trait_if_necessary(self.trait_ref.def_id);

        for &impl_def_id in self.blanket_impls.borrow().iter() {
            f(impl_def_id);
        }

        for v in self.nonblanket_impls.borrow().values() {
            for &impl_def_id in v {
                f(impl_def_id);
            }
        }
    }

    /// Iterate over every impl that could possibly match the
    /// self-type `self_ty`.
    pub fn for_each_relevant_impl<F: FnMut(DefId)>(&self,
                                                   tcx: &ctxt<'tcx>,
                                                   self_ty: Ty<'tcx>,
                                                   mut f: F)
    {
        tcx.populate_implementations_for_trait_if_necessary(self.trait_ref.def_id);

        for &impl_def_id in self.blanket_impls.borrow().iter() {
            f(impl_def_id);
        }

        // simplify_type(.., false) basically replaces type parameters and
        // projections with infer-variables. This is, of course, done on
        // the impl trait-ref when it is instantiated, but not on the
        // predicate trait-ref which is passed here.
        //
        // for example, if we match `S: Copy` against an impl like
        // `impl<T:Copy> Copy for Option<T>`, we replace the type variable
        // in `Option<T>` with an infer variable, to `Option<_>` (this
        // doesn't actually change fast_reject output), but we don't
        // replace `S` with anything - this impl of course can't be
        // selected, and as there are hundreds of similar impls,
        // considering them would significantly harm performance.
        if let Some(simp) = fast_reject::simplify_type(tcx, self_ty, true) {
            if let Some(impls) = self.nonblanket_impls.borrow().get(&simp) {
                for &impl_def_id in impls {
                    f(impl_def_id);
                }
            }
        } else {
            for v in self.nonblanket_impls.borrow().values() {
                for &impl_def_id in v {
                    f(impl_def_id);
                }
            }
        }
    }

}

bitflags! {
    flags AdtFlags: u32 {
        const NO_ADT_FLAGS        = 0,
        const IS_ENUM             = 1 << 0,
        const IS_DTORCK           = 1 << 1, // is this a dtorck type?
        const IS_DTORCK_VALID     = 1 << 2,
        const IS_PHANTOM_DATA     = 1 << 3,
        const IS_SIMD             = 1 << 4,
        const IS_FUNDAMENTAL      = 1 << 5,
        const IS_NO_DROP_FLAG     = 1 << 6,
    }
}

pub type AdtDef<'tcx> = &'tcx AdtDefData<'tcx, 'static>;
pub type VariantDef<'tcx> = &'tcx VariantDefData<'tcx, 'static>;
pub type FieldDef<'tcx> = &'tcx FieldDefData<'tcx, 'static>;

// See comment on AdtDefData for explanation
pub type AdtDefMaster<'tcx> = &'tcx AdtDefData<'tcx, 'tcx>;
pub type VariantDefMaster<'tcx> = &'tcx VariantDefData<'tcx, 'tcx>;
pub type FieldDefMaster<'tcx> = &'tcx FieldDefData<'tcx, 'tcx>;

pub struct VariantDefData<'tcx, 'container: 'tcx> {
    pub did: DefId,
    pub name: Name, // struct's name if this is a struct
    pub disr_val: Disr,
    pub fields: Vec<FieldDefData<'tcx, 'container>>
}

pub struct FieldDefData<'tcx, 'container: 'tcx> {
    /// The field's DefId. NOTE: the fields of tuple-like enum variants
    /// are not real items, and don't have entries in tcache etc.
    pub did: DefId,
    /// special_idents::unnamed_field.name
    /// if this is a tuple-like field
    pub name: Name,
    pub vis: hir::Visibility,
    /// TyIVar is used here to allow for variance (see the doc at
    /// AdtDefData).
    ty: ivar::TyIVar<'tcx, 'container>
}

/// The definition of an abstract data type - a struct or enum.
///
/// These are all interned (by intern_adt_def) into the adt_defs
/// table.
///
/// Because of the possibility of nested tcx-s, this type
/// needs 2 lifetimes: the traditional variant lifetime ('tcx)
/// bounding the lifetime of the inner types is of course necessary.
/// However, it is not sufficient - types from a child tcx must
/// not be leaked into the master tcx by being stored in an AdtDefData.
///
/// The 'container lifetime ensures that by outliving the container
/// tcx and preventing shorter-lived types from being inserted. When
/// write access is not needed, the 'container lifetime can be
/// erased to 'static, which can be done by the AdtDef wrapper.
pub struct AdtDefData<'tcx, 'container: 'tcx> {
    pub did: DefId,
    pub variants: Vec<VariantDefData<'tcx, 'container>>,
    destructor: Cell<Option<DefId>>,
    flags: Cell<AdtFlags>,
}

impl<'tcx, 'container> PartialEq for AdtDefData<'tcx, 'container> {
    // AdtDefData are always interned and this is part of TyS equality
    #[inline]
    fn eq(&self, other: &Self) -> bool { self as *const _ == other as *const _ }
}

impl<'tcx, 'container> Eq for AdtDefData<'tcx, 'container> {}

impl<'tcx, 'container> Hash for AdtDefData<'tcx, 'container> {
    #[inline]
    fn hash<H: Hasher>(&self, s: &mut H) {
        (self as *const AdtDefData).hash(s)
    }
}


#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum AdtKind { Struct, Enum }

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum VariantKind { Dict, Tuple, Unit }

impl<'tcx, 'container> AdtDefData<'tcx, 'container> {
    fn new(tcx: &ctxt<'tcx>,
           did: DefId,
           kind: AdtKind,
           variants: Vec<VariantDefData<'tcx, 'container>>) -> Self {
        let mut flags = AdtFlags::NO_ADT_FLAGS;
        let attrs = tcx.get_attrs(did);
        if attr::contains_name(&attrs, "fundamental") {
            flags = flags | AdtFlags::IS_FUNDAMENTAL;
        }
        if attr::contains_name(&attrs, "unsafe_no_drop_flag") {
            flags = flags | AdtFlags::IS_NO_DROP_FLAG;
        }
        if tcx.lookup_simd(did) {
            flags = flags | AdtFlags::IS_SIMD;
        }
        if Some(did) == tcx.lang_items.phantom_data() {
            flags = flags | AdtFlags::IS_PHANTOM_DATA;
        }
        if let AdtKind::Enum = kind {
            flags = flags | AdtFlags::IS_ENUM;
        }
        AdtDefData {
            did: did,
            variants: variants,
            flags: Cell::new(flags),
            destructor: Cell::new(None)
        }
    }

    fn calculate_dtorck(&'tcx self, tcx: &ctxt<'tcx>) {
        if tcx.is_adt_dtorck(self) {
            self.flags.set(self.flags.get() | AdtFlags::IS_DTORCK);
        }
        self.flags.set(self.flags.get() | AdtFlags::IS_DTORCK_VALID)
    }

    /// Returns the kind of the ADT - Struct or Enum.
    #[inline]
    pub fn adt_kind(&self) -> AdtKind {
        if self.flags.get().intersects(AdtFlags::IS_ENUM) {
            AdtKind::Enum
        } else {
            AdtKind::Struct
        }
    }

    /// Returns whether this is a dtorck type. If this returns
    /// true, this type being safe for destruction requires it to be
    /// alive; Otherwise, only the contents are required to be.
    #[inline]
    pub fn is_dtorck(&'tcx self, tcx: &ctxt<'tcx>) -> bool {
        if !self.flags.get().intersects(AdtFlags::IS_DTORCK_VALID) {
            self.calculate_dtorck(tcx)
        }
        self.flags.get().intersects(AdtFlags::IS_DTORCK)
    }

    /// Returns whether this type is #[fundamental] for the purposes
    /// of coherence checking.
    #[inline]
    pub fn is_fundamental(&self) -> bool {
        self.flags.get().intersects(AdtFlags::IS_FUNDAMENTAL)
    }

    #[inline]
    pub fn is_simd(&self) -> bool {
        self.flags.get().intersects(AdtFlags::IS_SIMD)
    }

    /// Returns true if this is PhantomData<T>.
    #[inline]
    pub fn is_phantom_data(&self) -> bool {
        self.flags.get().intersects(AdtFlags::IS_PHANTOM_DATA)
    }

    /// Returns whether this type has a destructor.
    pub fn has_dtor(&self) -> bool {
        match self.dtor_kind() {
            NoDtor => false,
            TraitDtor(..) => true
        }
    }

    /// Asserts this is a struct and returns the struct's unique
    /// variant.
    pub fn struct_variant(&self) -> &VariantDefData<'tcx, 'container> {
        assert!(self.adt_kind() == AdtKind::Struct);
        &self.variants[0]
    }

    #[inline]
    pub fn type_scheme(&self, tcx: &ctxt<'tcx>) -> TypeScheme<'tcx> {
        tcx.lookup_item_type(self.did)
    }

    #[inline]
    pub fn predicates(&self, tcx: &ctxt<'tcx>) -> GenericPredicates<'tcx> {
        tcx.lookup_predicates(self.did)
    }

    /// Returns an iterator over all fields contained
    /// by this ADT.
    #[inline]
    pub fn all_fields(&self) ->
            iter::FlatMap<
                slice::Iter<VariantDefData<'tcx, 'container>>,
                slice::Iter<FieldDefData<'tcx, 'container>>,
                for<'s> fn(&'s VariantDefData<'tcx, 'container>)
                    -> slice::Iter<'s, FieldDefData<'tcx, 'container>>
            > {
        self.variants.iter().flat_map(VariantDefData::fields_iter)
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.variants.is_empty()
    }

    #[inline]
    pub fn is_univariant(&self) -> bool {
        self.variants.len() == 1
    }

    pub fn is_payloadfree(&self) -> bool {
        !self.variants.is_empty() &&
            self.variants.iter().all(|v| v.fields.is_empty())
    }

    pub fn variant_with_id(&self, vid: DefId) -> &VariantDefData<'tcx, 'container> {
        self.variants
            .iter()
            .find(|v| v.did == vid)
            .expect("variant_with_id: unknown variant")
    }

    pub fn variant_index_with_id(&self, vid: DefId) -> usize {
        self.variants
            .iter()
            .position(|v| v.did == vid)
            .expect("variant_index_with_id: unknown variant")
    }

    pub fn variant_of_def(&self, def: def::Def) -> &VariantDefData<'tcx, 'container> {
        match def {
            def::DefVariant(_, vid, _) => self.variant_with_id(vid),
            def::DefStruct(..) | def::DefTy(..) => self.struct_variant(),
            _ => panic!("unexpected def {:?} in variant_of_def", def)
        }
    }

    pub fn destructor(&self) -> Option<DefId> {
        self.destructor.get()
    }

    pub fn set_destructor(&self, dtor: DefId) {
        assert!(self.destructor.get().is_none());
        self.destructor.set(Some(dtor));
    }

    pub fn dtor_kind(&self) -> DtorKind {
        match self.destructor.get() {
            Some(_) => {
                TraitDtor(!self.flags.get().intersects(AdtFlags::IS_NO_DROP_FLAG))
            }
            None => NoDtor,
        }
    }
}

impl<'tcx, 'container> VariantDefData<'tcx, 'container> {
    #[inline]
    fn fields_iter(&self) -> slice::Iter<FieldDefData<'tcx, 'container>> {
        self.fields.iter()
    }

    pub fn kind(&self) -> VariantKind {
        match self.fields.get(0) {
            None => VariantKind::Unit,
            Some(&FieldDefData { name, .. }) if name == special_idents::unnamed_field.name => {
                VariantKind::Tuple
            }
            Some(_) => VariantKind::Dict
        }
    }

    pub fn is_tuple_struct(&self) -> bool {
        self.kind() == VariantKind::Tuple
    }

    #[inline]
    pub fn find_field_named(&self,
                            name: ast::Name)
                            -> Option<&FieldDefData<'tcx, 'container>> {
        self.fields.iter().find(|f| f.name == name)
    }

    #[inline]
    pub fn field_named(&self, name: ast::Name) -> &FieldDefData<'tcx, 'container> {
        self.find_field_named(name).unwrap()
    }
}

impl<'tcx, 'container> FieldDefData<'tcx, 'container> {
    pub fn new(did: DefId,
               name: Name,
               vis: hir::Visibility) -> Self {
        FieldDefData {
            did: did,
            name: name,
            vis: vis,
            ty: ivar::TyIVar::new()
        }
    }

    pub fn ty(&self, tcx: &ctxt<'tcx>, subst: &Substs<'tcx>) -> Ty<'tcx> {
        self.unsubst_ty().subst(tcx, subst)
    }

    pub fn unsubst_ty(&self) -> Ty<'tcx> {
        self.ty.unwrap()
    }

    pub fn fulfill_ty(&self, ty: Ty<'container>) {
        self.ty.fulfill(ty);
    }
}

/// Records the substitutions used to translate the polytype for an
/// item into the monotype of an item reference.
#[derive(Clone)]
pub struct ItemSubsts<'tcx> {
    pub substs: Substs<'tcx>,
}

#[derive(Clone, Copy, PartialOrd, Ord, PartialEq, Eq, Debug, RustcEncodable, RustcDecodable)]
pub enum ClosureKind {
    // Warning: Ordering is significant here! The ordering is chosen
    // because the trait Fn is a subtrait of FnMut and so in turn, and
    // hence we order it so that Fn < FnMut < FnOnce.
    FnClosureKind,
    FnMutClosureKind,
    FnOnceClosureKind,
}

impl ClosureKind {
    pub fn trait_did(&self, cx: &ctxt) -> DefId {
        let result = match *self {
            FnClosureKind => cx.lang_items.require(FnTraitLangItem),
            FnMutClosureKind => {
                cx.lang_items.require(FnMutTraitLangItem)
            }
            FnOnceClosureKind => {
                cx.lang_items.require(FnOnceTraitLangItem)
            }
        };
        match result {
            Ok(trait_did) => trait_did,
            Err(err) => cx.sess.fatal(&err[..]),
        }
    }

    /// True if this a type that impls this closure kind
    /// must also implement `other`.
    pub fn extends(self, other: ty::ClosureKind) -> bool {
        match (self, other) {
            (FnClosureKind, FnClosureKind) => true,
            (FnClosureKind, FnMutClosureKind) => true,
            (FnClosureKind, FnOnceClosureKind) => true,
            (FnMutClosureKind, FnMutClosureKind) => true,
            (FnMutClosureKind, FnOnceClosureKind) => true,
            (FnOnceClosureKind, FnOnceClosureKind) => true,
            _ => false,
        }
    }
}

impl<'tcx> ctxt<'tcx> {
    pub fn pat_contains_ref_binding(&self, pat: &hir::Pat) -> Option<hir::Mutability> {
        pat_util::pat_contains_ref_binding(&self.def_map, pat)
    }

    pub fn arm_contains_ref_binding(&self, arm: &hir::Arm) -> Option<hir::Mutability> {
        pat_util::arm_contains_ref_binding(&self.def_map, arm)
    }
}

impl<'tcx> TyS<'tcx> {
    /// Iterator that walks `self` and any types reachable from
    /// `self`, in depth-first order. Note that just walks the types
    /// that appear in `self`, it does not descend into the fields of
    /// structs or variants. For example:
    ///
    /// ```notrust
    /// isize => { isize }
    /// Foo<Bar<isize>> => { Foo<Bar<isize>>, Bar<isize>, isize }
    /// [isize] => { [isize], isize }
    /// ```
    pub fn walk(&'tcx self) -> TypeWalker<'tcx> {
        TypeWalker::new(self)
    }

    /// Iterator that walks the immediate children of `self`.  Hence
    /// `Foo<Bar<i32>, u32>` yields the sequence `[Bar<i32>, u32]`
    /// (but not `i32`, like `walk`).
    pub fn walk_shallow(&'tcx self) -> IntoIter<Ty<'tcx>> {
        walk::walk_shallow(self)
    }

    pub fn as_opt_param_ty(&self) -> Option<ty::ParamTy> {
        match self.sty {
            ty::TyParam(ref d) => Some(d.clone()),
            _ => None,
        }
    }

    pub fn is_param(&self, space: ParamSpace, index: u32) -> bool {
        match self.sty {
            ty::TyParam(ref data) => data.space == space && data.idx == index,
            _ => false,
        }
    }

    /// Returns the regions directly referenced from this type (but
    /// not types reachable from this type via `walk_tys`). This
    /// ignores late-bound regions binders.
    pub fn regions(&self) -> Vec<ty::Region> {
        match self.sty {
            TyRef(region, _) => {
                vec![*region]
            }
            TyTrait(ref obj) => {
                let mut v = vec![obj.bounds.region_bound];
                v.push_all(obj.principal.skip_binder().substs.regions().as_slice());
                v
            }
            TyEnum(_, substs) |
            TyStruct(_, substs) => {
                substs.regions().as_slice().to_vec()
            }
            TyClosure(_, ref substs) => {
                substs.func_substs.regions().as_slice().to_vec()
            }
            TyProjection(ref data) => {
                data.trait_ref.substs.regions().as_slice().to_vec()
            }
            TyBareFn(..) |
            TyBool |
            TyChar |
            TyInt(_) |
            TyUint(_) |
            TyFloat(_) |
            TyBox(_) |
            TyStr |
            TyArray(_, _) |
            TySlice(_) |
            TyRawPtr(_) |
            TyTuple(_) |
            TyParam(_) |
            TyInfer(_) |
            TyError => {
                vec![]
            }
        }
    }

    /// Walks `ty` and any types appearing within `ty`, invoking the
    /// callback `f` on each type. If the callback returns false, then the
    /// children of the current type are ignored.
    ///
    /// Note: prefer `ty.walk()` where possible.
    pub fn maybe_walk<F>(&'tcx self, mut f: F)
        where F : FnMut(Ty<'tcx>) -> bool
    {
        let mut walker = self.walk();
        while let Some(ty) = walker.next() {
            if !f(ty) {
                walker.skip_current_subtree();
            }
        }
    }
}

impl ParamTy {
    pub fn new(space: subst::ParamSpace,
               index: u32,
               name: Name)
               -> ParamTy {
        ParamTy { space: space, idx: index, name: name }
    }

    pub fn for_self() -> ParamTy {
        ParamTy::new(subst::SelfSpace, 0, special_idents::type_self.name)
    }

    pub fn for_def(def: &TypeParameterDef) -> ParamTy {
        ParamTy::new(def.space, def.index, def.name)
    }

    pub fn to_ty<'tcx>(self, tcx: &ctxt<'tcx>) -> Ty<'tcx> {
        tcx.mk_param(self.space, self.idx, self.name)
    }

    pub fn is_self(&self) -> bool {
        self.space == subst::SelfSpace && self.idx == 0
    }
}

impl<'tcx> ItemSubsts<'tcx> {
    pub fn empty() -> ItemSubsts<'tcx> {
        ItemSubsts { substs: Substs::empty() }
    }

    pub fn is_noop(&self) -> bool {
        self.substs.is_noop()
    }
}

impl<'tcx> TyS<'tcx> {
    fn impls_bound<'a>(&'tcx self, param_env: &ParameterEnvironment<'a,'tcx>,
                       bound: ty::BuiltinBound,
                       span: Span)
                       -> bool
    {
        let tcx = param_env.tcx;
        let infcx = infer::new_infer_ctxt(tcx, &tcx.tables, Some(param_env.clone()), false);

        let is_impld = traits::type_known_to_meet_builtin_bound(&infcx,
                                                                self, bound, span);

        debug!("Ty::impls_bound({:?}, {:?}) = {:?}",
               self, bound, is_impld);

        is_impld
    }

    // FIXME (@jroesch): I made this public to use it, not sure if should be private
    pub fn moves_by_default<'a>(&'tcx self, param_env: &ParameterEnvironment<'a,'tcx>,
                           span: Span) -> bool {
        if self.flags.get().intersects(TypeFlags::MOVENESS_CACHED) {
            return self.flags.get().intersects(TypeFlags::MOVES_BY_DEFAULT);
        }

        assert!(!self.needs_infer());

        // Fast-path for primitive types
        let result = match self.sty {
            TyBool | TyChar | TyInt(..) | TyUint(..) | TyFloat(..) |
            TyRawPtr(..) | TyBareFn(..) | TyRef(_, TypeAndMut {
                mutbl: hir::MutImmutable, ..
            }) => Some(false),

            TyStr | TyBox(..) | TyRef(_, TypeAndMut {
                mutbl: hir::MutMutable, ..
            }) => Some(true),

            TyArray(..) | TySlice(_) | TyTrait(..) | TyTuple(..) |
            TyClosure(..) | TyEnum(..) | TyStruct(..) |
            TyProjection(..) | TyParam(..) | TyInfer(..) | TyError => None
        }.unwrap_or_else(|| !self.impls_bound(param_env, ty::BoundCopy, span));

        if !self.has_param_types() && !self.has_self_ty() {
            self.flags.set(self.flags.get() | if result {
                TypeFlags::MOVENESS_CACHED | TypeFlags::MOVES_BY_DEFAULT
            } else {
                TypeFlags::MOVENESS_CACHED
            });
        }

        result
    }

    #[inline]
    pub fn is_sized<'a>(&'tcx self, param_env: &ParameterEnvironment<'a,'tcx>,
                        span: Span) -> bool
    {
        if self.flags.get().intersects(TypeFlags::SIZEDNESS_CACHED) {
            return self.flags.get().intersects(TypeFlags::IS_SIZED);
        }

        self.is_sized_uncached(param_env, span)
    }

    fn is_sized_uncached<'a>(&'tcx self, param_env: &ParameterEnvironment<'a,'tcx>,
                             span: Span) -> bool {
        assert!(!self.needs_infer());

        // Fast-path for primitive types
        let result = match self.sty {
            TyBool | TyChar | TyInt(..) | TyUint(..) | TyFloat(..) |
            TyBox(..) | TyRawPtr(..) | TyRef(..) | TyBareFn(..) |
            TyArray(..) | TyTuple(..) | TyClosure(..) => Some(true),

            TyStr | TyTrait(..) | TySlice(_) => Some(false),

            TyEnum(..) | TyStruct(..) | TyProjection(..) | TyParam(..) |
            TyInfer(..) | TyError => None
        }.unwrap_or_else(|| self.impls_bound(param_env, ty::BoundSized, span));

        if !self.has_param_types() && !self.has_self_ty() {
            self.flags.set(self.flags.get() | if result {
                TypeFlags::SIZEDNESS_CACHED | TypeFlags::IS_SIZED
            } else {
                TypeFlags::SIZEDNESS_CACHED
            });
        }

        result
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum LvaluePreference {
    PreferMutLvalue,
    NoPreference
}

impl LvaluePreference {
    pub fn from_mutbl(m: hir::Mutability) -> Self {
        match m {
            hir::MutMutable => PreferMutLvalue,
            hir::MutImmutable => NoPreference,
        }
    }
}

/// Describes whether a type is representable. For types that are not
/// representable, 'SelfRecursive' and 'ContainsRecursive' are used to
/// distinguish between types that are recursive with themselves and types that
/// contain a different recursive type. These cases can therefore be treated
/// differently when reporting errors.
///
/// The ordering of the cases is significant. They are sorted so that cmp::max
/// will keep the "more erroneous" of two values.
#[derive(Copy, Clone, PartialOrd, Ord, Eq, PartialEq, Debug)]
pub enum Representability {
    Representable,
    ContainsRecursive,
    SelfRecursive,
}

impl<'tcx> TyS<'tcx> {
    /// Check whether a type is representable. This means it cannot contain unboxed
    /// structural recursion. This check is needed for structs and enums.
    pub fn is_representable(&'tcx self, cx: &ctxt<'tcx>, sp: Span) -> Representability {

        // Iterate until something non-representable is found
        fn find_nonrepresentable<'tcx, It: Iterator<Item=Ty<'tcx>>>(cx: &ctxt<'tcx>, sp: Span,
                                                                    seen: &mut Vec<Ty<'tcx>>,
                                                                    iter: It)
                                                                    -> Representability {
            iter.fold(Representable,
                      |r, ty| cmp::max(r, is_type_structurally_recursive(cx, sp, seen, ty)))
        }

        fn are_inner_types_recursive<'tcx>(cx: &ctxt<'tcx>, sp: Span,
                                           seen: &mut Vec<Ty<'tcx>>, ty: Ty<'tcx>)
                                           -> Representability {
            match ty.sty {
                TyTuple(ref ts) => {
                    find_nonrepresentable(cx, sp, seen, ts.iter().cloned())
                }
                // Fixed-length vectors.
                // FIXME(#11924) Behavior undecided for zero-length vectors.
                TyArray(ty, _) => {
                    is_type_structurally_recursive(cx, sp, seen, ty)
                }
                TyStruct(def, substs) | TyEnum(def, substs) => {
                    find_nonrepresentable(cx,
                                          sp,
                                          seen,
                                          def.all_fields().map(|f| f.ty(cx, substs)))
                }
                TyClosure(..) => {
                    // this check is run on type definitions, so we don't expect
                    // to see closure types
                    cx.sess.bug(&format!("requires check invoked on inapplicable type: {:?}", ty))
                }
                _ => Representable,
            }
        }

        fn same_struct_or_enum<'tcx>(ty: Ty<'tcx>, def: AdtDef<'tcx>) -> bool {
            match ty.sty {
                TyStruct(ty_def, _) | TyEnum(ty_def, _) => {
                     ty_def == def
                }
                _ => false
            }
        }

        fn same_type<'tcx>(a: Ty<'tcx>, b: Ty<'tcx>) -> bool {
            match (&a.sty, &b.sty) {
                (&TyStruct(did_a, ref substs_a), &TyStruct(did_b, ref substs_b)) |
                (&TyEnum(did_a, ref substs_a), &TyEnum(did_b, ref substs_b)) => {
                    if did_a != did_b {
                        return false;
                    }

                    let types_a = substs_a.types.get_slice(subst::TypeSpace);
                    let types_b = substs_b.types.get_slice(subst::TypeSpace);

                    let mut pairs = types_a.iter().zip(types_b);

                    pairs.all(|(&a, &b)| same_type(a, b))
                }
                _ => {
                    a == b
                }
            }
        }

        // Does the type `ty` directly (without indirection through a pointer)
        // contain any types on stack `seen`?
        fn is_type_structurally_recursive<'tcx>(cx: &ctxt<'tcx>, sp: Span,
                                                seen: &mut Vec<Ty<'tcx>>,
                                                ty: Ty<'tcx>) -> Representability {
            debug!("is_type_structurally_recursive: {:?}", ty);

            match ty.sty {
                TyStruct(def, _) | TyEnum(def, _) => {
                    {
                        // Iterate through stack of previously seen types.
                        let mut iter = seen.iter();

                        // The first item in `seen` is the type we are actually curious about.
                        // We want to return SelfRecursive if this type contains itself.
                        // It is important that we DON'T take generic parameters into account
                        // for this check, so that Bar<T> in this example counts as SelfRecursive:
                        //
                        // struct Foo;
                        // struct Bar<T> { x: Bar<Foo> }

                        match iter.next() {
                            Some(&seen_type) => {
                                if same_struct_or_enum(seen_type, def) {
                                    debug!("SelfRecursive: {:?} contains {:?}",
                                           seen_type,
                                           ty);
                                    return SelfRecursive;
                                }
                            }
                            None => {}
                        }

                        // We also need to know whether the first item contains other types
                        // that are structurally recursive. If we don't catch this case, we
                        // will recurse infinitely for some inputs.
                        //
                        // It is important that we DO take generic parameters into account
                        // here, so that code like this is considered SelfRecursive, not
                        // ContainsRecursive:
                        //
                        // struct Foo { Option<Option<Foo>> }

                        for &seen_type in iter {
                            if same_type(ty, seen_type) {
                                debug!("ContainsRecursive: {:?} contains {:?}",
                                       seen_type,
                                       ty);
                                return ContainsRecursive;
                            }
                        }
                    }

                    // For structs and enums, track all previously seen types by pushing them
                    // onto the 'seen' stack.
                    seen.push(ty);
                    let out = are_inner_types_recursive(cx, sp, seen, ty);
                    seen.pop();
                    out
                }
                _ => {
                    // No need to push in other cases.
                    are_inner_types_recursive(cx, sp, seen, ty)
                }
            }
        }

        debug!("is_type_representable: {:?}", self);

        // To avoid a stack overflow when checking an enum variant or struct that
        // contains a different, structurally recursive type, maintain a stack
        // of seen types and check recursion for each of them (issues #3008, #3779).
        let mut seen: Vec<Ty> = Vec::new();
        let r = is_type_structurally_recursive(cx, sp, &mut seen, self);
        debug!("is_type_representable: {:?} is {:?}", self, r);
        r
    }

    /// See `expr_ty_adjusted`
    pub fn adjust<F>(&'tcx self, cx: &ctxt<'tcx>,
                     span: Span,
                     expr_id: NodeId,
                     adjustment: Option<&AutoAdjustment<'tcx>>,
                     mut method_type: F)
                     -> Ty<'tcx> where
        F: FnMut(MethodCall) -> Option<Ty<'tcx>>,
    {
        if let TyError = self.sty {
            return self;
        }

        return match adjustment {
            Some(adjustment) => {
                match *adjustment {
                   AdjustReifyFnPointer => {
                        match self.sty {
                            ty::TyBareFn(Some(_), b) => {
                                cx.mk_fn(None, b)
                            }
                            _ => {
                                cx.sess.bug(
                                    &format!("AdjustReifyFnPointer adjustment on non-fn-item: \
                                              {:?}", self));
                            }
                        }
                    }

                   AdjustUnsafeFnPointer => {
                        match self.sty {
                            ty::TyBareFn(None, b) => cx.safe_to_unsafe_fn_ty(b),
                            ref b => {
                                cx.sess.bug(
                                    &format!("AdjustReifyFnPointer adjustment on non-fn-item: \
                                             {:?}",
                                            b));
                            }
                        }
                   }

                    AdjustDerefRef(ref adj) => {
                        let mut adjusted_ty = self;

                        if !adjusted_ty.references_error() {
                            for i in 0..adj.autoderefs {
                                adjusted_ty =
                                    adjusted_ty.adjust_for_autoderef(cx,
                                                                     expr_id,
                                                                     span,
                                                                     i as u32,
                                                                     &mut method_type);
                            }
                        }

                        if let Some(target) = adj.unsize {
                            target
                        } else {
                            adjusted_ty.adjust_for_autoref(cx, adj.autoref)
                        }
                    }
                }
            }
            None => self
        };
    }

    pub fn adjust_for_autoderef<F>(&'tcx self,
                                   cx: &ctxt<'tcx>,
                                   expr_id: ast::NodeId,
                                   expr_span: Span,
                                   autoderef: u32, // how many autoderefs so far?
                                   mut method_type: F)
                                   -> Ty<'tcx> where
        F: FnMut(MethodCall) -> Option<Ty<'tcx>>,
    {
        let method_call = MethodCall::autoderef(expr_id, autoderef);
        let mut adjusted_ty = self;
        if let Some(method_ty) = method_type(method_call) {
            // Method calls always have all late-bound regions
            // fully instantiated.
            let fn_ret = cx.no_late_bound_regions(&method_ty.fn_ret()).unwrap();
            adjusted_ty = fn_ret.unwrap();
        }
        match adjusted_ty.builtin_deref(true, NoPreference) {
            Some(mt) => mt.ty,
            None => {
                cx.sess.span_bug(
                    expr_span,
                    &format!("the {}th autoderef failed: {}",
                             autoderef,
                             adjusted_ty)
                        );
            }
        }
    }

    pub fn adjust_for_autoref(&'tcx self, cx: &ctxt<'tcx>,
                              autoref: Option<AutoRef<'tcx>>)
                              -> Ty<'tcx> {
        match autoref {
            None => self,
            Some(AutoPtr(r, m)) => {
                cx.mk_ref(r, TypeAndMut { ty: self, mutbl: m })
            }
            Some(AutoUnsafe(m)) => {
                cx.mk_ptr(TypeAndMut { ty: self, mutbl: m })
            }
        }
    }

}

/// Helper for looking things up in the various maps that are populated during
/// typeck::collect (e.g., `cx.impl_or_trait_items`, `cx.tcache`, etc).  All of
/// these share the pattern that if the id is local, it should have been loaded
/// into the map by the `typeck::collect` phase.  If the def-id is external,
/// then we have to go consult the crate loading code (and cache the result for
/// the future).
fn lookup_locally_or_in_crate_store<V, F>(descr: &str,
                                          def_id: DefId,
                                          map: &RefCell<DefIdMap<V>>,
                                          load_external: F) -> V where
    V: Clone,
    F: FnOnce() -> V,
{
    match map.borrow().get(&def_id).cloned() {
        Some(v) => { return v; }
        None => { }
    }

    if def_id.is_local() {
        panic!("No def'n found for {:?} in tcx.{}", def_id, descr);
    }
    let v = load_external();
    map.borrow_mut().insert(def_id, v.clone());
    v
}

impl BorrowKind {
    pub fn from_mutbl(m: hir::Mutability) -> BorrowKind {
        match m {
            hir::MutMutable => MutBorrow,
            hir::MutImmutable => ImmBorrow,
        }
    }

    /// Returns a mutability `m` such that an `&m T` pointer could be used to obtain this borrow
    /// kind. Because borrow kinds are richer than mutabilities, we sometimes have to pick a
    /// mutability that is stronger than necessary so that it at least *would permit* the borrow in
    /// question.
    pub fn to_mutbl_lossy(self) -> hir::Mutability {
        match self {
            MutBorrow => hir::MutMutable,
            ImmBorrow => hir::MutImmutable,

            // We have no type corresponding to a unique imm borrow, so
            // use `&mut`. It gives all the capabilities of an `&uniq`
            // and hence is a safe "over approximation".
            UniqueImmBorrow => hir::MutMutable,
        }
    }

    pub fn to_user_str(&self) -> &'static str {
        match *self {
            MutBorrow => "mutable",
            ImmBorrow => "immutable",
            UniqueImmBorrow => "uniquely immutable",
        }
    }
}

impl<'tcx> ctxt<'tcx> {
    /// Returns the type of element at index `i` in tuple or tuple-like type `t`.
    /// For an enum `t`, `variant` is None only if `t` is a univariant enum.
    pub fn positional_element_ty(&self,
                                 ty: Ty<'tcx>,
                                 i: usize,
                                 variant: Option<DefId>) -> Option<Ty<'tcx>> {
        match (&ty.sty, variant) {
            (&TyStruct(def, substs), None) => {
                def.struct_variant().fields.get(i).map(|f| f.ty(self, substs))
            }
            (&TyEnum(def, substs), Some(vid)) => {
                def.variant_with_id(vid).fields.get(i).map(|f| f.ty(self, substs))
            }
            (&TyEnum(def, substs), None) => {
                assert!(def.is_univariant());
                def.variants[0].fields.get(i).map(|f| f.ty(self, substs))
            }
            (&TyTuple(ref v), None) => v.get(i).cloned(),
            _ => None
        }
    }

    /// Returns the type of element at field `n` in struct or struct-like type `t`.
    /// For an enum `t`, `variant` must be some def id.
    pub fn named_element_ty(&self,
                            ty: Ty<'tcx>,
                            n: Name,
                            variant: Option<DefId>) -> Option<Ty<'tcx>> {
        match (&ty.sty, variant) {
            (&TyStruct(def, substs), None) => {
                def.struct_variant().find_field_named(n).map(|f| f.ty(self, substs))
            }
            (&TyEnum(def, substs), Some(vid)) => {
                def.variant_with_id(vid).find_field_named(n).map(|f| f.ty(self, substs))
            }
            _ => return None
        }
    }

    pub fn node_id_to_type(&self, id: NodeId) -> Ty<'tcx> {
        match self.node_id_to_type_opt(id) {
           Some(ty) => ty,
           None => self.sess.bug(
               &format!("node_id_to_type: no type for node `{}`",
                        self.map.node_to_string(id)))
        }
    }

    pub fn node_id_to_type_opt(&self, id: NodeId) -> Option<Ty<'tcx>> {
        self.tables.borrow().node_types.get(&id).cloned()
    }

    pub fn node_id_item_substs(&self, id: NodeId) -> ItemSubsts<'tcx> {
        match self.tables.borrow().item_substs.get(&id) {
            None => ItemSubsts::empty(),
            Some(ts) => ts.clone(),
        }
    }

    // Returns the type of a pattern as a monotype. Like @expr_ty, this function
    // doesn't provide type parameter substitutions.
    pub fn pat_ty(&self, pat: &hir::Pat) -> Ty<'tcx> {
        self.node_id_to_type(pat.id)
    }
    pub fn pat_ty_opt(&self, pat: &hir::Pat) -> Option<Ty<'tcx>> {
        self.node_id_to_type_opt(pat.id)
    }

    // Returns the type of an expression as a monotype.
    //
    // NB (1): This is the PRE-ADJUSTMENT TYPE for the expression.  That is, in
    // some cases, we insert `AutoAdjustment` annotations such as auto-deref or
    // auto-ref.  The type returned by this function does not consider such
    // adjustments.  See `expr_ty_adjusted()` instead.
    //
    // NB (2): This type doesn't provide type parameter substitutions; e.g. if you
    // ask for the type of "id" in "id(3)", it will return "fn(&isize) -> isize"
    // instead of "fn(ty) -> T with T = isize".
    pub fn expr_ty(&self, expr: &hir::Expr) -> Ty<'tcx> {
        self.node_id_to_type(expr.id)
    }

    pub fn expr_ty_opt(&self, expr: &hir::Expr) -> Option<Ty<'tcx>> {
        self.node_id_to_type_opt(expr.id)
    }

    /// Returns the type of `expr`, considering any `AutoAdjustment`
    /// entry recorded for that expression.
    ///
    /// It would almost certainly be better to store the adjusted ty in with
    /// the `AutoAdjustment`, but I opted not to do this because it would
    /// require serializing and deserializing the type and, although that's not
    /// hard to do, I just hate that code so much I didn't want to touch it
    /// unless it was to fix it properly, which seemed a distraction from the
    /// thread at hand! -nmatsakis
    pub fn expr_ty_adjusted(&self, expr: &hir::Expr) -> Ty<'tcx> {
        self.expr_ty(expr)
            .adjust(self, expr.span, expr.id,
                    self.tables.borrow().adjustments.get(&expr.id),
                    |method_call| {
            self.tables.borrow().method_map.get(&method_call).map(|method| method.ty)
        })
    }

    pub fn expr_span(&self, id: NodeId) -> Span {
        match self.map.find(id) {
            Some(ast_map::NodeExpr(e)) => {
                e.span
            }
            Some(f) => {
                self.sess.bug(&format!("Node id {} is not an expr: {:?}",
                                       id, f));
            }
            None => {
                self.sess.bug(&format!("Node id {} is not present \
                                        in the node map", id));
            }
        }
    }

    pub fn local_var_name_str(&self, id: NodeId) -> InternedString {
        match self.map.find(id) {
            Some(ast_map::NodeLocal(pat)) => {
                match pat.node {
                    hir::PatIdent(_, ref path1, _) => path1.node.name.as_str(),
                    _ => {
                        self.sess.bug(&format!("Variable id {} maps to {:?}, not local", id, pat));
                    },
                }
            },
            r => self.sess.bug(&format!("Variable id {} maps to {:?}, not local", id, r)),
        }
    }

    pub fn resolve_expr(&self, expr: &hir::Expr) -> def::Def {
        match self.def_map.borrow().get(&expr.id) {
            Some(def) => def.full_def(),
            None => {
                self.sess.span_bug(expr.span, &format!(
                    "no def-map entry for expr {}", expr.id));
            }
        }
    }

    pub fn expr_is_lval(&self, expr: &hir::Expr) -> bool {
         match expr.node {
            hir::ExprPath(..) => {
                // We can't use resolve_expr here, as this needs to run on broken
                // programs. We don't need to through - associated items are all
                // rvalues.
                match self.def_map.borrow().get(&expr.id) {
                    Some(&def::PathResolution {
                        base_def: def::DefStatic(..), ..
                    }) | Some(&def::PathResolution {
                        base_def: def::DefUpvar(..), ..
                    }) | Some(&def::PathResolution {
                        base_def: def::DefLocal(..), ..
                    }) => {
                        true
                    }

                    Some(..) => false,

                    None => self.sess.span_bug(expr.span, &format!(
                        "no def for path {}", expr.id))
                }
            }

            hir::ExprUnary(hir::UnDeref, _) |
            hir::ExprField(..) |
            hir::ExprTupField(..) |
            hir::ExprIndex(..) => {
                true
            }

            hir::ExprCall(..) |
            hir::ExprMethodCall(..) |
            hir::ExprStruct(..) |
            hir::ExprRange(..) |
            hir::ExprTup(..) |
            hir::ExprIf(..) |
            hir::ExprMatch(..) |
            hir::ExprClosure(..) |
            hir::ExprBlock(..) |
            hir::ExprRepeat(..) |
            hir::ExprVec(..) |
            hir::ExprBreak(..) |
            hir::ExprAgain(..) |
            hir::ExprRet(..) |
            hir::ExprWhile(..) |
            hir::ExprLoop(..) |
            hir::ExprAssign(..) |
            hir::ExprInlineAsm(..) |
            hir::ExprAssignOp(..) |
            hir::ExprLit(_) |
            hir::ExprUnary(..) |
            hir::ExprBox(..) |
            hir::ExprAddrOf(..) |
            hir::ExprBinary(..) |
            hir::ExprCast(..) => {
                false
            }

            hir::ExprParen(ref e) => self.expr_is_lval(e),
        }
    }

    pub fn provided_source(&self, id: DefId) -> Option<DefId> {
        self.provided_method_sources.borrow().get(&id).cloned()
    }

    pub fn provided_trait_methods(&self, id: DefId) -> Vec<Rc<Method<'tcx>>> {
        if id.is_local() {
            if let ItemTrait(_, _, _, ref ms) = self.map.expect_item(id.node).node {
                ms.iter().filter_map(|ti| {
                    if let hir::MethodTraitItem(_, Some(_)) = ti.node {
                        match self.impl_or_trait_item(DefId::local(ti.id)) {
                            MethodTraitItem(m) => Some(m),
                            _ => {
                                self.sess.bug("provided_trait_methods(): \
                                               non-method item found from \
                                               looking up provided method?!")
                            }
                        }
                    } else {
                        None
                    }
                }).collect()
            } else {
                self.sess.bug(&format!("provided_trait_methods: `{:?}` is not a trait", id))
            }
        } else {
            csearch::get_provided_trait_methods(self, id)
        }
    }

    pub fn associated_consts(&self, id: DefId) -> Vec<Rc<AssociatedConst<'tcx>>> {
        if id.is_local() {
            match self.map.expect_item(id.node).node {
                ItemTrait(_, _, _, ref tis) => {
                    tis.iter().filter_map(|ti| {
                        if let hir::ConstTraitItem(_, _) = ti.node {
                            match self.impl_or_trait_item(DefId::local(ti.id)) {
                                ConstTraitItem(ac) => Some(ac),
                                _ => {
                                    self.sess.bug("associated_consts(): \
                                                   non-const item found from \
                                                   looking up a constant?!")
                                }
                            }
                        } else {
                            None
                        }
                    }).collect()
                }
                ItemImpl(_, _, _, _, _, ref iis) => {
                    iis.iter().filter_map(|ii| {
                        if let hir::ConstImplItem(_, _) = ii.node {
                            match self.impl_or_trait_item(DefId::local(ii.id)) {
                                ConstTraitItem(ac) => Some(ac),
                                _ => {
                                    self.sess.bug("associated_consts(): \
                                                   non-const item found from \
                                                   looking up a constant?!")
                                }
                            }
                        } else {
                            None
                        }
                    }).collect()
                }
                _ => {
                    self.sess.bug(&format!("associated_consts: `{:?}` is not a trait \
                                            or impl", id))
                }
            }
        } else {
            csearch::get_associated_consts(self, id)
        }
    }

    pub fn trait_items(&self, trait_did: DefId) -> Rc<Vec<ImplOrTraitItem<'tcx>>> {
        let mut trait_items = self.trait_items_cache.borrow_mut();
        match trait_items.get(&trait_did).cloned() {
            Some(trait_items) => trait_items,
            None => {
                let def_ids = self.trait_item_def_ids(trait_did);
                let items: Rc<Vec<ImplOrTraitItem>> =
                    Rc::new(def_ids.iter()
                                   .map(|d| self.impl_or_trait_item(d.def_id()))
                                   .collect());
                trait_items.insert(trait_did, items.clone());
                items
            }
        }
    }

    pub fn trait_impl_polarity(&self, id: DefId) -> Option<hir::ImplPolarity> {
        if id.is_local() {
            match self.map.find(id.node) {
                Some(ast_map::NodeItem(item)) => {
                    match item.node {
                        hir::ItemImpl(_, polarity, _, _, _, _) => Some(polarity),
                        _ => None
                    }
                }
                _ => None
            }
        } else {
            csearch::get_impl_polarity(self, id)
        }
    }

    pub fn custom_coerce_unsized_kind(&self, did: DefId) -> CustomCoerceUnsized {
        memoized(&self.custom_coerce_unsized_kinds, did, |did: DefId| {
            let (kind, src) = if did.krate != LOCAL_CRATE {
                (csearch::get_custom_coerce_unsized_kind(self, did), "external")
            } else {
                (None, "local")
            };

            match kind {
                Some(kind) => kind,
                None => {
                    self.sess.bug(&format!("custom_coerce_unsized_kind: \
                                            {} impl `{}` is missing its kind",
                                           src, self.item_path_str(did)));
                }
            }
        })
    }

    pub fn impl_or_trait_item(&self, id: DefId) -> ImplOrTraitItem<'tcx> {
        lookup_locally_or_in_crate_store(
            "impl_or_trait_items", id, &self.impl_or_trait_items,
            || csearch::get_impl_or_trait_item(self, id))
    }

    pub fn trait_item_def_ids(&self, id: DefId) -> Rc<Vec<ImplOrTraitItemId>> {
        lookup_locally_or_in_crate_store(
            "trait_item_def_ids", id, &self.trait_item_def_ids,
            || Rc::new(csearch::get_trait_item_def_ids(&self.sess.cstore, id)))
    }

    /// Returns the trait-ref corresponding to a given impl, or None if it is
    /// an inherent impl.
    pub fn impl_trait_ref(&self, id: DefId) -> Option<TraitRef<'tcx>> {
        lookup_locally_or_in_crate_store(
            "impl_trait_refs", id, &self.impl_trait_refs,
            || csearch::get_impl_trait(self, id))
    }

    /// Returns whether this DefId refers to an impl
    pub fn is_impl(&self, id: DefId) -> bool {
        if id.is_local() {
            if let Some(ast_map::NodeItem(
                &hir::Item { node: hir::ItemImpl(..), .. })) = self.map.find(id.node) {
                true
            } else {
                false
            }
        } else {
            csearch::is_impl(&self.sess.cstore, id)
        }
    }

    pub fn trait_ref_to_def_id(&self, tr: &hir::TraitRef) -> DefId {
        self.def_map.borrow().get(&tr.ref_id).expect("no def-map entry for trait").def_id()
    }

    pub fn item_path_str(&self, id: DefId) -> String {
        self.with_path(id, |path| ast_map::path_to_string(path))
    }

    pub fn with_path<T, F>(&self, id: DefId, f: F) -> T where
        F: FnOnce(ast_map::PathElems) -> T,
    {
        if id.is_local() {
            self.map.with_path(id.node, f)
        } else {
            f(csearch::get_item_path(self, id).iter().cloned().chain(LinkedPath::empty()))
        }
    }

    pub fn item_name(&self, id: DefId) -> ast::Name {
        if id.is_local() {
            self.map.get_path_elem(id.node).name()
        } else {
            csearch::get_item_name(self, id)
        }
    }

    /// Returns `(normalized_type, ty)`, where `normalized_type` is the
    /// IntType representation of one of {i64,i32,i16,i8,u64,u32,u16,u8},
    /// and `ty` is the original type (i.e. may include `isize` or
    /// `usize`).
    pub fn enum_repr_type(&self, opt_hint: Option<&attr::ReprAttr>)
                          -> (attr::IntType, Ty<'tcx>) {
        let repr_type = match opt_hint {
            // Feed in the given type
            Some(&attr::ReprInt(_, int_t)) => int_t,
            // ... but provide sensible default if none provided
            //
            // NB. Historically `fn enum_variants` generate i64 here, while
            // rustc_typeck::check would generate isize.
            _ => SignedInt(hir::TyIs),
        };

        let repr_type_ty = repr_type.to_ty(self);
        let repr_type = match repr_type {
            SignedInt(hir::TyIs) =>
                SignedInt(self.sess.target.int_type),
            UnsignedInt(hir::TyUs) =>
                UnsignedInt(self.sess.target.uint_type),
            other => other
        };

        (repr_type, repr_type_ty)
    }


    // Register a given item type
    pub fn register_item_type(&self, did: DefId, ty: TypeScheme<'tcx>) {
        self.tcache.borrow_mut().insert(did, ty);
    }

    // If the given item is in an external crate, looks up its type and adds it to
    // the type cache. Returns the type parameters and type.
    pub fn lookup_item_type(&self, did: DefId) -> TypeScheme<'tcx> {
        lookup_locally_or_in_crate_store(
            "tcache", did, &self.tcache,
            || csearch::get_type(self, did))
    }

    /// Given the did of a trait, returns its canonical trait ref.
    pub fn lookup_trait_def(&self, did: DefId) -> &'tcx TraitDef<'tcx> {
        lookup_locally_or_in_crate_store(
            "trait_defs", did, &self.trait_defs,
            || self.alloc_trait_def(csearch::get_trait_def(self, did))
        )
    }

    /// Given the did of an ADT, return a master reference to its
    /// definition. Unless you are planning on fulfilling the ADT's fields,
    /// use lookup_adt_def instead.
    pub fn lookup_adt_def_master(&self, did: DefId) -> AdtDefMaster<'tcx> {
        lookup_locally_or_in_crate_store(
            "adt_defs", did, &self.adt_defs,
            || csearch::get_adt_def(self, did)
        )
    }

    /// Given the did of an ADT, return a reference to its definition.
    pub fn lookup_adt_def(&self, did: DefId) -> AdtDef<'tcx> {
        // when reverse-variance goes away, a transmute::<AdtDefMaster,AdtDef>
        // woud be needed here.
        self.lookup_adt_def_master(did)
    }

    /// Return the list of all interned ADT definitions
    pub fn adt_defs(&self) -> Vec<AdtDef<'tcx>> {
        self.adt_defs.borrow().values().cloned().collect()
    }

    /// Given the did of an item, returns its full set of predicates.
    pub fn lookup_predicates(&self, did: DefId) -> GenericPredicates<'tcx> {
        lookup_locally_or_in_crate_store(
            "predicates", did, &self.predicates,
            || csearch::get_predicates(self, did))
    }

    /// Given the did of a trait, returns its superpredicates.
    pub fn lookup_super_predicates(&self, did: DefId) -> GenericPredicates<'tcx> {
        lookup_locally_or_in_crate_store(
            "super_predicates", did, &self.super_predicates,
            || csearch::get_super_predicates(self, did))
    }

    /// Get the attributes of a definition.
    pub fn get_attrs(&self, did: DefId) -> Cow<'tcx, [hir::Attribute]> {
        if did.is_local() {
            Cow::Borrowed(self.map.attrs(did.node))
        } else {
            Cow::Owned(csearch::get_item_attrs(&self.sess.cstore, did))
        }
    }

    /// Determine whether an item is annotated with an attribute
    pub fn has_attr(&self, did: DefId, attr: &str) -> bool {
        self.get_attrs(did).iter().any(|item| item.check_name(attr))
    }

    /// Determine whether an item is annotated with `#[repr(packed)]`
    pub fn lookup_packed(&self, did: DefId) -> bool {
        self.lookup_repr_hints(did).contains(&attr::ReprPacked)
    }

    /// Determine whether an item is annotated with `#[simd]`
    pub fn lookup_simd(&self, did: DefId) -> bool {
        self.has_attr(did, "simd")
            || self.lookup_repr_hints(did).contains(&attr::ReprSimd)
    }

    /// Obtain the representation annotation for a struct definition.
    pub fn lookup_repr_hints(&self, did: DefId) -> Rc<Vec<attr::ReprAttr>> {
        memoized(&self.repr_hint_cache, did, |did: DefId| {
            Rc::new(if did.is_local() {
                self.get_attrs(did).iter().flat_map(|meta| {
                    attr::find_repr_attrs(self.sess.diagnostic(), meta).into_iter()
                }).collect()
            } else {
                csearch::get_repr_attrs(&self.sess.cstore, did)
            })
        })
    }


    /// Returns the deeply last field of nested structures, or the same type,
    /// if not a structure at all. Corresponds to the only possible unsized
    /// field, and its type can be used to determine unsizing strategy.
    pub fn struct_tail(&self, mut ty: Ty<'tcx>) -> Ty<'tcx> {
        while let TyStruct(def, substs) = ty.sty {
            match def.struct_variant().fields.last() {
                Some(f) => ty = f.ty(self, substs),
                None => break
            }
        }
        ty
    }

    /// Same as applying struct_tail on `source` and `target`, but only
    /// keeps going as long as the two types are instances of the same
    /// structure definitions.
    /// For `(Foo<Foo<T>>, Foo<Trait>)`, the result will be `(Foo<T>, Trait)`,
    /// whereas struct_tail produces `T`, and `Trait`, respectively.
    pub fn struct_lockstep_tails(&self,
                                 source: Ty<'tcx>,
                                 target: Ty<'tcx>)
                                 -> (Ty<'tcx>, Ty<'tcx>) {
        let (mut a, mut b) = (source, target);
        while let (&TyStruct(a_def, a_substs), &TyStruct(b_def, b_substs)) = (&a.sty, &b.sty) {
            if a_def != b_def {
                break;
            }
            if let Some(f) = a_def.struct_variant().fields.last() {
                a = f.ty(self, a_substs);
                b = f.ty(self, b_substs);
            } else {
                break;
            }
        }
        (a, b)
    }

    // Returns the repeat count for a repeating vector expression.
    pub fn eval_repeat_count(&self, count_expr: &hir::Expr) -> usize {
        let hint = UncheckedExprHint(self.types.usize);
        match const_eval::eval_const_expr_partial(self, count_expr, hint) {
            Ok(val) => {
                let found = match val {
                    ConstVal::Uint(count) => return count as usize,
                    ConstVal::Int(count) if count >= 0 => return count as usize,
                    const_val => const_val.description(),
                };
                span_err!(self.sess, count_expr.span, E0306,
                    "expected positive integer for repeat count, found {}",
                    found);
            }
            Err(err) => {
                let err_msg = match count_expr.node {
                    hir::ExprPath(None, hir::Path {
                        global: false,
                        ref segments,
                        ..
                    }) if segments.len() == 1 =>
                        format!("found variable"),
                    _ => match err.kind {
                        ErrKind::MiscCatchAll => format!("but found {}", err.description()),
                        _ => format!("but {}", err.description())
                    }
                };
                span_err!(self.sess, count_expr.span, E0307,
                    "expected constant integer for repeat count, {}", err_msg);
            }
        }
        0
    }

    // Iterate over a type parameter's bounded traits and any supertraits
    // of those traits, ignoring kinds.
    // Here, the supertraits are the transitive closure of the supertrait
    // relation on the supertraits from each bounded trait's constraint
    // list.
    pub fn each_bound_trait_and_supertraits<F>(&self,
                                               bounds: &[PolyTraitRef<'tcx>],
                                               mut f: F)
                                               -> bool where
        F: FnMut(PolyTraitRef<'tcx>) -> bool,
    {
        for bound_trait_ref in traits::transitive_bounds(self, bounds) {
            if !f(bound_trait_ref) {
                return false;
            }
        }
        return true;
    }

    /// Given a set of predicates that apply to an object type, returns
    /// the region bounds that the (erased) `Self` type must
    /// outlive. Precisely *because* the `Self` type is erased, the
    /// parameter `erased_self_ty` must be supplied to indicate what type
    /// has been used to represent `Self` in the predicates
    /// themselves. This should really be a unique type; `FreshTy(0)` is a
    /// popular choice.
    ///
    /// NB: in some cases, particularly around higher-ranked bounds,
    /// this function returns a kind of conservative approximation.
    /// That is, all regions returned by this function are definitely
    /// required, but there may be other region bounds that are not
    /// returned, as well as requirements like `for<'a> T: 'a`.
    ///
    /// Requires that trait definitions have been processed so that we can
    /// elaborate predicates and walk supertraits.
    pub fn required_region_bounds(&self,
                                  erased_self_ty: Ty<'tcx>,
                                  predicates: Vec<ty::Predicate<'tcx>>)
                                  -> Vec<ty::Region>    {
        debug!("required_region_bounds(erased_self_ty={:?}, predicates={:?})",
               erased_self_ty,
               predicates);

        assert!(!erased_self_ty.has_escaping_regions());

        traits::elaborate_predicates(self, predicates)
            .filter_map(|predicate| {
                match predicate {
                    ty::Predicate::Projection(..) |
                    ty::Predicate::Trait(..) |
                    ty::Predicate::Equate(..) |
                    ty::Predicate::WellFormed(..) |
                    ty::Predicate::ObjectSafe(..) |
                    ty::Predicate::RegionOutlives(..) => {
                        None
                    }
                    ty::Predicate::TypeOutlives(ty::Binder(ty::OutlivesPredicate(t, r))) => {
                        // Search for a bound of the form `erased_self_ty
                        // : 'a`, but be wary of something like `for<'a>
                        // erased_self_ty : 'a` (we interpret a
                        // higher-ranked bound like that as 'static,
                        // though at present the code in `fulfill.rs`
                        // considers such bounds to be unsatisfiable, so
                        // it's kind of a moot point since you could never
                        // construct such an object, but this seems
                        // correct even if that code changes).
                        if t == erased_self_ty && !r.has_escaping_regions() {
                            Some(r)
                        } else {
                            None
                        }
                    }
                }
            })
            .collect()
    }

    pub fn item_variances(&self, item_id: DefId) -> Rc<ItemVariances> {
        lookup_locally_or_in_crate_store(
            "item_variance_map", item_id, &self.item_variance_map,
            || Rc::new(csearch::get_item_variances(&self.sess.cstore, item_id)))
    }

    pub fn trait_has_default_impl(&self, trait_def_id: DefId) -> bool {
        self.populate_implementations_for_trait_if_necessary(trait_def_id);

        let def = self.lookup_trait_def(trait_def_id);
        def.flags.get().intersects(TraitFlags::HAS_DEFAULT_IMPL)
    }

    /// Records a trait-to-implementation mapping.
    pub fn record_trait_has_default_impl(&self, trait_def_id: DefId) {
        let def = self.lookup_trait_def(trait_def_id);
        def.flags.set(def.flags.get() | TraitFlags::HAS_DEFAULT_IMPL)
    }

    /// Load primitive inherent implementations if necessary
    pub fn populate_implementations_for_primitive_if_necessary(&self,
                                                               primitive_def_id: DefId) {
        if primitive_def_id.is_local() {
            return
        }

        if self.populated_external_primitive_impls.borrow().contains(&primitive_def_id) {
            return
        }

        debug!("populate_implementations_for_primitive_if_necessary: searching for {:?}",
               primitive_def_id);

        let impl_items = csearch::get_impl_items(&self.sess.cstore, primitive_def_id);

        // Store the implementation info.
        self.impl_items.borrow_mut().insert(primitive_def_id, impl_items);
        self.populated_external_primitive_impls.borrow_mut().insert(primitive_def_id);
    }

    /// Populates the type context with all the inherent implementations for
    /// the given type if necessary.
    pub fn populate_inherent_implementations_for_type_if_necessary(&self,
                                                                   type_id: DefId) {
        if type_id.is_local() {
            return
        }

        if self.populated_external_types.borrow().contains(&type_id) {
            return
        }

        debug!("populate_inherent_implementations_for_type_if_necessary: searching for {:?}",
               type_id);

        let mut inherent_impls = Vec::new();
        csearch::each_inherent_implementation_for_type(&self.sess.cstore, type_id, |impl_def_id| {
            // Record the implementation.
            inherent_impls.push(impl_def_id);

            // Store the implementation info.
            let impl_items = csearch::get_impl_items(&self.sess.cstore, impl_def_id);
            self.impl_items.borrow_mut().insert(impl_def_id, impl_items);
        });

        self.inherent_impls.borrow_mut().insert(type_id, Rc::new(inherent_impls));
        self.populated_external_types.borrow_mut().insert(type_id);
    }

    /// Populates the type context with all the implementations for the given
    /// trait if necessary.
    pub fn populate_implementations_for_trait_if_necessary(&self, trait_id: DefId) {
        if trait_id.is_local() {
            return
        }

        let def = self.lookup_trait_def(trait_id);
        if def.flags.get().intersects(TraitFlags::IMPLS_VALID) {
            return;
        }

        debug!("populate_implementations_for_trait_if_necessary: searching for {:?}", def);

        if csearch::is_defaulted_trait(&self.sess.cstore, trait_id) {
            self.record_trait_has_default_impl(trait_id);
        }

        csearch::each_implementation_for_trait(&self.sess.cstore, trait_id, |impl_def_id| {
            let impl_items = csearch::get_impl_items(&self.sess.cstore, impl_def_id);
            let trait_ref = self.impl_trait_ref(impl_def_id).unwrap();
            // Record the trait->implementation mapping.
            def.record_impl(self, impl_def_id, trait_ref);

            // For any methods that use a default implementation, add them to
            // the map. This is a bit unfortunate.
            for impl_item_def_id in &impl_items {
                let method_def_id = impl_item_def_id.def_id();
                match self.impl_or_trait_item(method_def_id) {
                    MethodTraitItem(method) => {
                        if let Some(source) = method.provided_source {
                            self.provided_method_sources
                                .borrow_mut()
                                .insert(method_def_id, source);
                        }
                    }
                    _ => {}
                }
            }

            // Store the implementation info.
            self.impl_items.borrow_mut().insert(impl_def_id, impl_items);
        });

        def.flags.set(def.flags.get() | TraitFlags::IMPLS_VALID);
    }

    /// Given the def_id of an impl, return the def_id of the trait it implements.
    /// If it implements no trait, return `None`.
    pub fn trait_id_of_impl(&self, def_id: DefId) -> Option<DefId> {
        self.impl_trait_ref(def_id).map(|tr| tr.def_id)
    }

    /// If the given def ID describes a method belonging to an impl, return the
    /// ID of the impl that the method belongs to. Otherwise, return `None`.
    pub fn impl_of_method(&self, def_id: DefId) -> Option<DefId> {
        if def_id.krate != LOCAL_CRATE {
            return match csearch::get_impl_or_trait_item(self,
                                                         def_id).container() {
                TraitContainer(_) => None,
                ImplContainer(def_id) => Some(def_id),
            };
        }
        match self.impl_or_trait_items.borrow().get(&def_id).cloned() {
            Some(trait_item) => {
                match trait_item.container() {
                    TraitContainer(_) => None,
                    ImplContainer(def_id) => Some(def_id),
                }
            }
            None => None
        }
    }

    /// If the given def ID describes an item belonging to a trait (either a
    /// default method or an implementation of a trait method), return the ID of
    /// the trait that the method belongs to. Otherwise, return `None`.
    pub fn trait_of_item(&self, def_id: DefId) -> Option<DefId> {
        if def_id.krate != LOCAL_CRATE {
            return csearch::get_trait_of_item(&self.sess.cstore, def_id, self);
        }
        match self.impl_or_trait_items.borrow().get(&def_id).cloned() {
            Some(impl_or_trait_item) => {
                match impl_or_trait_item.container() {
                    TraitContainer(def_id) => Some(def_id),
                    ImplContainer(def_id) => self.trait_id_of_impl(def_id),
                }
            }
            None => None
        }
    }

    /// If the given def ID describes an item belonging to a trait, (either a
    /// default method or an implementation of a trait method), return the ID of
    /// the method inside trait definition (this means that if the given def ID
    /// is already that of the original trait method, then the return value is
    /// the same).
    /// Otherwise, return `None`.
    pub fn trait_item_of_item(&self, def_id: DefId) -> Option<ImplOrTraitItemId> {
        let impl_item = match self.impl_or_trait_items.borrow().get(&def_id) {
            Some(m) => m.clone(),
            None => return None,
        };
        let name = impl_item.name();
        match self.trait_of_item(def_id) {
            Some(trait_did) => {
                self.trait_items(trait_did).iter()
                    .find(|item| item.name() == name)
                    .map(|item| item.id())
            }
            None => None
        }
    }

    /// Creates a hash of the type `Ty` which will be the same no matter what crate
    /// context it's calculated within. This is used by the `type_id` intrinsic.
    pub fn hash_crate_independent(&self, ty: Ty<'tcx>, svh: &Svh) -> u64 {
        let mut state = SipHasher::new();
        helper(self, ty, svh, &mut state);
        return state.finish();

        fn helper<'tcx>(tcx: &ctxt<'tcx>, ty: Ty<'tcx>, svh: &Svh,
                        state: &mut SipHasher) {
            macro_rules! byte { ($b:expr) => { ($b as u8).hash(state) } }
            macro_rules! hash { ($e:expr) => { $e.hash(state) }  }

            let region = |state: &mut SipHasher, r: Region| {
                match r {
                    ReStatic => {}
                    ReLateBound(db, BrAnon(i)) => {
                        db.hash(state);
                        i.hash(state);
                    }
                    ReEmpty |
                    ReEarlyBound(..) |
                    ReLateBound(..) |
                    ReFree(..) |
                    ReScope(..) |
                    ReVar(..) |
                    ReSkolemized(..) => {
                        tcx.sess.bug("unexpected region found when hashing a type")
                    }
                }
            };
            let did = |state: &mut SipHasher, did: DefId| {
                let h = if did.is_local() {
                    svh.clone()
                } else {
                    tcx.sess.cstore.get_crate_hash(did.krate)
                };
                h.as_str().hash(state);
                did.node.hash(state);
            };
            let mt = |state: &mut SipHasher, mt: TypeAndMut| {
                mt.mutbl.hash(state);
            };
            let fn_sig = |state: &mut SipHasher, sig: &Binder<FnSig<'tcx>>| {
                let sig = tcx.anonymize_late_bound_regions(sig).0;
                for a in &sig.inputs { helper(tcx, *a, svh, state); }
                if let ty::FnConverging(output) = sig.output {
                    helper(tcx, output, svh, state);
                }
            };
            ty.maybe_walk(|ty| {
                match ty.sty {
                    TyBool => byte!(2),
                    TyChar => byte!(3),
                    TyInt(i) => {
                        byte!(4);
                        hash!(i);
                    }
                    TyUint(u) => {
                        byte!(5);
                        hash!(u);
                    }
                    TyFloat(f) => {
                        byte!(6);
                        hash!(f);
                    }
                    TyStr => {
                        byte!(7);
                    }
                    TyEnum(d, _) => {
                        byte!(8);
                        did(state, d.did);
                    }
                    TyBox(_) => {
                        byte!(9);
                    }
                    TyArray(_, n) => {
                        byte!(10);
                        n.hash(state);
                    }
                    TySlice(_) => {
                        byte!(11);
                    }
                    TyRawPtr(m) => {
                        byte!(12);
                        mt(state, m);
                    }
                    TyRef(r, m) => {
                        byte!(13);
                        region(state, *r);
                        mt(state, m);
                    }
                    TyBareFn(opt_def_id, ref b) => {
                        byte!(14);
                        hash!(opt_def_id);
                        hash!(b.unsafety);
                        hash!(b.abi);
                        fn_sig(state, &b.sig);
                        return false;
                    }
                    TyTrait(ref data) => {
                        byte!(17);
                        did(state, data.principal_def_id());
                        hash!(data.bounds);

                        let principal = tcx.anonymize_late_bound_regions(&data.principal).0;
                        for subty in &principal.substs.types {
                            helper(tcx, subty, svh, state);
                        }

                        return false;
                    }
                    TyStruct(d, _) => {
                        byte!(18);
                        did(state, d.did);
                    }
                    TyTuple(ref inner) => {
                        byte!(19);
                        hash!(inner.len());
                    }
                    TyParam(p) => {
                        byte!(20);
                        hash!(p.space);
                        hash!(p.idx);
                        hash!(p.name.as_str());
                    }
                    TyInfer(_) => unreachable!(),
                    TyError => byte!(21),
                    TyClosure(d, _) => {
                        byte!(22);
                        did(state, d);
                    }
                    TyProjection(ref data) => {
                        byte!(23);
                        did(state, data.trait_ref.def_id);
                        hash!(data.item_name.as_str());
                    }
                }
                true
            });
        }
    }

    /// Construct a parameter environment suitable for static contexts or other contexts where there
    /// are no free type/lifetime parameters in scope.
    pub fn empty_parameter_environment<'a>(&'a self)
                                           -> ParameterEnvironment<'a,'tcx> {
        ty::ParameterEnvironment { tcx: self,
                                   free_substs: Substs::empty(),
                                   caller_bounds: Vec::new(),
                                   implicit_region_bound: ty::ReEmpty,
                                   selection_cache: traits::SelectionCache::new(),

                                   // for an empty parameter
                                   // environment, there ARE no free
                                   // regions, so it shouldn't matter
                                   // what we use for the free id
                                   free_id: ast::DUMMY_NODE_ID }
    }

    /// Constructs and returns a substitution that can be applied to move from
    /// the "outer" view of a type or method to the "inner" view.
    /// In general, this means converting from bound parameters to
    /// free parameters. Since we currently represent bound/free type
    /// parameters in the same way, this only has an effect on regions.
    pub fn construct_free_substs(&self, generics: &Generics<'tcx>,
                                 free_id: NodeId) -> Substs<'tcx> {
        // map T => T
        let mut types = VecPerParamSpace::empty();
        for def in generics.types.as_slice() {
            debug!("construct_parameter_environment(): push_types_from_defs: def={:?}",
                    def);
            types.push(def.space, self.mk_param_from_def(def));
        }

        let free_id_outlive = self.region_maps.item_extent(free_id);

        // map bound 'a => free 'a
        let mut regions = VecPerParamSpace::empty();
        for def in generics.regions.as_slice() {
            let region =
                ReFree(FreeRegion { scope: free_id_outlive,
                                    bound_region: BrNamed(def.def_id, def.name) });
            debug!("push_region_params {:?}", region);
            regions.push(def.space, region);
        }

        Substs {
            types: types,
            regions: subst::NonerasedRegions(regions)
        }
    }

    /// See `ParameterEnvironment` struct def'n for details
    pub fn construct_parameter_environment<'a>(&'a self,
                                               span: Span,
                                               generics: &ty::Generics<'tcx>,
                                               generic_predicates: &ty::GenericPredicates<'tcx>,
                                               free_id: NodeId)
                                               -> ParameterEnvironment<'a, 'tcx>
    {
        //
        // Construct the free substs.
        //

        let free_substs = self.construct_free_substs(generics, free_id);
        let free_id_outlive = self.region_maps.item_extent(free_id);

        //
        // Compute the bounds on Self and the type parameters.
        //

        let bounds = generic_predicates.instantiate(self, &free_substs);
        let bounds = self.liberate_late_bound_regions(free_id_outlive, &ty::Binder(bounds));
        let predicates = bounds.predicates.into_vec();

        debug!("construct_parameter_environment: free_id={:?} free_subst={:?} predicates={:?}",
               free_id,
               free_substs,
               predicates);

        //
        // Finally, we have to normalize the bounds in the environment, in
        // case they contain any associated type projections. This process
        // can yield errors if the put in illegal associated types, like
        // `<i32 as Foo>::Bar` where `i32` does not implement `Foo`. We
        // report these errors right here; this doesn't actually feel
        // right to me, because constructing the environment feels like a
        // kind of a "idempotent" action, but I'm not sure where would be
        // a better place. In practice, we construct environments for
        // every fn once during type checking, and we'll abort if there
        // are any errors at that point, so after type checking you can be
        // sure that this will succeed without errors anyway.
        //

        let unnormalized_env = ty::ParameterEnvironment {
            tcx: self,
            free_substs: free_substs,
            implicit_region_bound: ty::ReScope(free_id_outlive),
            caller_bounds: predicates,
            selection_cache: traits::SelectionCache::new(),
            free_id: free_id,
        };

        let cause = traits::ObligationCause::misc(span, free_id);
        traits::normalize_param_env_or_error(unnormalized_env, cause)
    }

    pub fn is_method_call(&self, expr_id: NodeId) -> bool {
        self.tables.borrow().method_map.contains_key(&MethodCall::expr(expr_id))
    }

    pub fn is_overloaded_autoderef(&self, expr_id: NodeId, autoderefs: u32) -> bool {
        self.tables.borrow().method_map.contains_key(&MethodCall::autoderef(expr_id,
                                                                            autoderefs))
    }

    pub fn upvar_capture(&self, upvar_id: ty::UpvarId) -> Option<ty::UpvarCapture> {
        Some(self.tables.borrow().upvar_capture_map.get(&upvar_id).unwrap().clone())
    }


    /// Returns true if this ADT is a dtorck type, i.e. whether it being
    /// safe for destruction requires it to be alive
    fn is_adt_dtorck(&self, adt: AdtDef<'tcx>) -> bool {
        let dtor_method = match adt.destructor() {
            Some(dtor) => dtor,
            None => return false
        };
        let impl_did = self.impl_of_method(dtor_method).unwrap_or_else(|| {
            self.sess.bug(&format!("no Drop impl for the dtor of `{:?}`", adt))
        });
        let generics = adt.type_scheme(self).generics;

        // In `impl<'a> Drop ...`, we automatically assume
        // `'a` is meaningful and thus represents a bound
        // through which we could reach borrowed data.
        //
        // FIXME (pnkfelix): In the future it would be good to
        // extend the language to allow the user to express,
        // in the impl signature, that a lifetime is not
        // actually used (something like `where 'a: ?Live`).
        if generics.has_region_params(subst::TypeSpace) {
            debug!("typ: {:?} has interesting dtor due to region params",
                   adt);
            return true;
        }

        let mut seen_items = Vec::new();
        let mut items_to_inspect = vec![impl_did];
        while let Some(item_def_id) = items_to_inspect.pop() {
            if seen_items.contains(&item_def_id) {
                continue;
            }

            for pred in self.lookup_predicates(item_def_id).predicates {
                let result = match pred {
                    ty::Predicate::Equate(..) |
                    ty::Predicate::RegionOutlives(..) |
                    ty::Predicate::TypeOutlives(..) |
                    ty::Predicate::WellFormed(..) |
                    ty::Predicate::ObjectSafe(..) |
                    ty::Predicate::Projection(..) => {
                        // For now, assume all these where-clauses
                        // may give drop implementation capabilty
                        // to access borrowed data.
                        true
                    }

                    ty::Predicate::Trait(ty::Binder(ref t_pred)) => {
                        let def_id = t_pred.trait_ref.def_id;
                        if self.trait_items(def_id).len() != 0 {
                            // If trait has items, assume it adds
                            // capability to access borrowed data.
                            true
                        } else {
                            // Trait without items is itself
                            // uninteresting from POV of dropck.
                            //
                            // However, may have parent w/ items;
                            // so schedule checking of predicates,
                            items_to_inspect.push(def_id);
                            // and say "no capability found" for now.
                            false
                        }
                    }
                };

                if result {
                    debug!("typ: {:?} has interesting dtor due to generic preds, e.g. {:?}",
                           adt, pred);
                    return true;
                }
            }

            seen_items.push(item_def_id);
        }

        debug!("typ: {:?} is dtorck-safe", adt);
        false
    }
}

/// The category of explicit self.
#[derive(Clone, Copy, Eq, PartialEq, Debug)]
pub enum ExplicitSelfCategory {
    StaticExplicitSelfCategory,
    ByValueExplicitSelfCategory,
    ByReferenceExplicitSelfCategory(Region, hir::Mutability),
    ByBoxExplicitSelfCategory,
}

/// A free variable referred to in a function.
#[derive(Copy, Clone, RustcEncodable, RustcDecodable)]
pub struct Freevar {
    /// The variable being accessed free.
    pub def: def::Def,

    // First span where it is accessed (there can be multiple).
    pub span: Span
}

pub type FreevarMap = NodeMap<Vec<Freevar>>;

pub type CaptureModeMap = NodeMap<hir::CaptureClause>;

// Trait method resolution
pub type TraitMap = NodeMap<Vec<DefId>>;

// Map from the NodeId of a glob import to a list of items which are actually
// imported.
pub type GlobMap = HashMap<NodeId, HashSet<Name>>;

impl<'tcx> AutoAdjustment<'tcx> {
    pub fn is_identity(&self) -> bool {
        match *self {
            AdjustReifyFnPointer |
            AdjustUnsafeFnPointer => false,
            AdjustDerefRef(ref r) => r.is_identity(),
        }
    }
}

impl<'tcx> AutoDerefRef<'tcx> {
    pub fn is_identity(&self) -> bool {
        self.autoderefs == 0 && self.unsize.is_none() && self.autoref.is_none()
    }
}

impl<'tcx> ctxt<'tcx> {
    pub fn with_freevars<T, F>(&self, fid: NodeId, f: F) -> T where
        F: FnOnce(&[Freevar]) -> T,
    {
        match self.freevars.borrow().get(&fid) {
            None => f(&[]),
            Some(d) => f(&d[..])
        }
    }

    pub fn make_substs_for_receiver_types(&self,
                                          trait_ref: &ty::TraitRef<'tcx>,
                                          method: &ty::Method<'tcx>)
                                          -> subst::Substs<'tcx>
    {
        /*!
         * Substitutes the values for the receiver's type parameters
         * that are found in method, leaving the method's type parameters
         * intact.
         */

        let meth_tps: Vec<Ty> =
            method.generics.types.get_slice(subst::FnSpace)
                  .iter()
                  .map(|def| self.mk_param_from_def(def))
                  .collect();
        let meth_regions: Vec<ty::Region> =
            method.generics.regions.get_slice(subst::FnSpace)
                  .iter()
                  .map(|def| def.to_early_bound_region())
                  .collect();
        trait_ref.substs.clone().with_method(meth_tps, meth_regions)
    }
}

/// An "escaping region" is a bound region whose binder is not part of `t`.
///
/// So, for example, consider a type like the following, which has two binders:
///
///    for<'a> fn(x: for<'b> fn(&'a isize, &'b isize))
///    ^~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~ outer scope
///                  ^~~~~~~~~~~~~~~~~~~~~~~~~~~~  inner scope
///
/// This type has *bound regions* (`'a`, `'b`), but it does not have escaping regions, because the
/// binders of both `'a` and `'b` are part of the type itself. However, if we consider the *inner
/// fn type*, that type has an escaping region: `'a`.
///
/// Note that what I'm calling an "escaping region" is often just called a "free region". However,
/// we already use the term "free region". It refers to the regions that we use to represent bound
/// regions on a fn definition while we are typechecking its body.
///
/// To clarify, conceptually there is no particular difference between an "escaping" region and a
/// "free" region. However, there is a big difference in practice. Basically, when "entering" a
/// binding level, one is generally required to do some sort of processing to a bound region, such
/// as replacing it with a fresh/skolemized region, or making an entry in the environment to
/// represent the scope to which it is attached, etc. An escaping region represents a bound region
/// for which this processing has not yet been done.
pub trait RegionEscape {
    fn has_escaping_regions(&self) -> bool {
        self.has_regions_escaping_depth(0)
    }

    fn has_regions_escaping_depth(&self, depth: u32) -> bool;
}

pub trait HasTypeFlags {
    fn has_type_flags(&self, flags: TypeFlags) -> bool;
    fn has_projection_types(&self) -> bool {
        self.has_type_flags(TypeFlags::HAS_PROJECTION)
    }
    fn references_error(&self) -> bool {
        self.has_type_flags(TypeFlags::HAS_TY_ERR)
    }
    fn has_param_types(&self) -> bool {
        self.has_type_flags(TypeFlags::HAS_PARAMS)
    }
    fn has_self_ty(&self) -> bool {
        self.has_type_flags(TypeFlags::HAS_SELF)
    }
    fn has_infer_types(&self) -> bool {
        self.has_type_flags(TypeFlags::HAS_TY_INFER)
    }
    fn needs_infer(&self) -> bool {
        self.has_type_flags(TypeFlags::HAS_TY_INFER | TypeFlags::HAS_RE_INFER)
    }
    fn needs_subst(&self) -> bool {
        self.has_type_flags(TypeFlags::NEEDS_SUBST)
    }
    fn has_closure_types(&self) -> bool {
        self.has_type_flags(TypeFlags::HAS_TY_CLOSURE)
    }
    fn has_erasable_regions(&self) -> bool {
        self.has_type_flags(TypeFlags::HAS_RE_EARLY_BOUND |
                            TypeFlags::HAS_RE_INFER |
                            TypeFlags::HAS_FREE_REGIONS)
    }
    /// Indicates whether this value references only 'global'
    /// types/lifetimes that are the same regardless of what fn we are
    /// in. This is used for caching. Errs on the side of returning
    /// false.
    fn is_global(&self) -> bool {
        !self.has_type_flags(TypeFlags::HAS_LOCAL_NAMES)
    }
}
