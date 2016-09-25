// Copyright 2012-2016 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use self::Constructor::*;
use self::Usefulness::*;
use self::WitnessPreference::*;

use rustc::middle::const_val::ConstVal;
use eval::{compare_const_vals};

use rustc_data_structures::indexed_vec::Idx;

use pattern::{FieldPattern, Pattern, PatternKind};
use pattern::{PatternFoldable, PatternFolder};

use rustc::hir::def_id::{DefId};
use rustc::hir::pat_util::def_to_path;
use rustc::ty::{self, Ty, TyCtxt, TypeFoldable};

use rustc::hir;
use rustc::hir::def::CtorKind;
use rustc::hir::{Pat, PatKind};
use rustc::util::common::ErrorReported;

use syntax::ast::{self, DUMMY_NODE_ID};
use syntax::codemap::Spanned;
use syntax::ptr::P;
use syntax_pos::{Span, DUMMY_SP};

use arena::TypedArena;

use std::cmp::Ordering;
use std::fmt;
use std::iter::{FromIterator, IntoIterator, repeat};

pub fn lower_pat<'a, 'tcx>(cx: &MatchCheckCtxt<'a, 'tcx>, pat: &Pat)
                           -> &'a Pattern<'tcx>
{
    cx.pattern_arena.alloc(
        LiteralExpander.fold_pattern(&Pattern::from_hir(cx.tcx, pat))
    )
}

struct LiteralExpander;
impl<'tcx> PatternFolder<'tcx> for LiteralExpander {
    fn fold_pattern(&mut self, pat: &Pattern<'tcx>) -> Pattern<'tcx> {
        match (&pat.ty.sty, &*pat.kind) {
            (&ty::TyRef(_, mt), &PatternKind::Constant { ref value }) => {
                Pattern {
                    ty: pat.ty,
                    span: pat.span,
                    kind: box PatternKind::Deref {
                        subpattern: Pattern {
                            ty: mt.ty,
                            span: pat.span,
                            kind: box PatternKind::Constant { value: value.clone() },
                        }
                    }
                }
            }
            (_, &PatternKind::Binding { subpattern: Some(ref s), .. }) => {
                s.fold_with(self)
            }
            _ => pat.super_fold_with(self)
        }
    }
}

pub const DUMMY_WILD_PAT: &'static Pat = &Pat {
    id: DUMMY_NODE_ID,
    node: PatKind::Wild,
    span: DUMMY_SP
};

impl<'tcx> Pattern<'tcx> {
    fn is_wildcard(&self) -> bool {
        match *self.kind {
            PatternKind::Binding { subpattern: None, .. } | PatternKind::Wild =>
                true,
            _ => false
        }
    }
}

pub struct Matrix<'a, 'tcx: 'a>(Vec<Vec<&'a Pattern<'tcx>>>);

impl<'a, 'tcx> Matrix<'a, 'tcx> {
    pub fn empty() -> Self {
        Matrix(vec![])
    }

    pub fn push(&mut self, row: Vec<&'a Pattern<'tcx>>) {
        self.0.push(row)
    }
}

/// Pretty-printer for matrices of patterns, example:
/// ++++++++++++++++++++++++++
/// + _     + []             +
/// ++++++++++++++++++++++++++
/// + true  + [First]        +
/// ++++++++++++++++++++++++++
/// + true  + [Second(true)] +
/// ++++++++++++++++++++++++++
/// + false + [_]            +
/// ++++++++++++++++++++++++++
/// + _     + [_, _, ..tail] +
/// ++++++++++++++++++++++++++
impl<'a, 'tcx> fmt::Debug for Matrix<'a, 'tcx> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "\n")?;

        let &Matrix(ref m) = self;
        let pretty_printed_matrix: Vec<Vec<String>> = m.iter().map(|row| {
            row.iter().map(|pat| format!("{:?}", pat)).collect()
        }).collect();

        let column_count = m.iter().map(|row| row.len()).max().unwrap_or(0);
        assert!(m.iter().all(|row| row.len() == column_count));
        let column_widths: Vec<usize> = (0..column_count).map(|col| {
            pretty_printed_matrix.iter().map(|row| row[col].len()).max().unwrap_or(0)
        }).collect();

        let total_width = column_widths.iter().cloned().sum::<usize>() + column_count * 3 + 1;
        let br = repeat('+').take(total_width).collect::<String>();
        write!(f, "{}\n", br)?;
        for row in pretty_printed_matrix {
            write!(f, "+")?;
            for (column, pat_str) in row.into_iter().enumerate() {
                write!(f, " ")?;
                write!(f, "{:1$}", pat_str, column_widths[column])?;
                write!(f, " +")?;
            }
            write!(f, "\n")?;
            write!(f, "{}\n", br)?;
        }
        Ok(())
    }
}

impl<'a, 'tcx> FromIterator<Vec<&'a Pattern<'tcx>>> for Matrix<'a, 'tcx> {
    fn from_iter<T: IntoIterator<Item=Vec<&'a Pattern<'tcx>>>>(iter: T) -> Self
    {
        Matrix(iter.into_iter().collect())
    }
}

//NOTE: appears to be the only place other then InferCtxt to contain a ParamEnv
pub struct MatchCheckCtxt<'a, 'tcx: 'a> {
    pub tcx: TyCtxt<'a, 'tcx, 'tcx>,
    pub param_env: ty::ParameterEnvironment<'tcx>,
    /// A wild pattern with an error type - it exists to avoid having to normalize
    /// associated types to get field types.
    pub wild_pattern: &'a Pattern<'tcx>,
    pub pattern_arena: &'a TypedArena<Pattern<'tcx>>,
}

impl<'a, 'tcx> MatchCheckCtxt<'a, 'tcx> {
    pub fn create_and_enter<F, R>(
        tcx: TyCtxt<'a, 'tcx, 'tcx>,
        param_env: ty::ParameterEnvironment<'tcx>,
        f: F) -> R
        where F: for<'b> FnOnce(MatchCheckCtxt<'b, 'tcx>) -> R
    {
        let wild_pattern = Pattern {
            ty: tcx.types.err,
            span: DUMMY_SP,
            kind: box PatternKind::Wild
        };

        let pattern_arena = TypedArena::new();

        f(MatchCheckCtxt {
            tcx: tcx,
            param_env: param_env,
            wild_pattern: &wild_pattern,
            pattern_arena: &pattern_arena,
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Constructor {
    /// The constructor of all patterns that don't vary by constructor,
    /// e.g. struct patterns and fixed-length arrays.
    Single,
    /// Enum variants.
    Variant(DefId),
    /// Literal values.
    ConstantValue(ConstVal),
    /// Ranges of literal values (2..5).
    ConstantRange(ConstVal, ConstVal),
    /// Array patterns of length n.
    Slice(usize),
}

impl Constructor {
    fn variant_for_adt<'tcx, 'container, 'a>(&self,
                                             adt: &'a ty::AdtDefData<'tcx, 'container>)
                                             -> &'a ty::VariantDefData<'tcx, 'container> {
        match self {
            &Variant(vid) => adt.variant_with_id(vid),
            &Single => {
                assert_eq!(adt.variants.len(), 1);
                &adt.variants[0]
            }
            _ => bug!("bad constructor {:?} for adt {:?}", self, adt)
        }
    }
}

#[derive(Clone, PartialEq)]
pub enum Usefulness {
    Useful,
    UsefulWithWitness(Vec<Witness>),
    NotUseful
}

#[derive(Copy, Clone)]
pub enum WitnessPreference {
    ConstructWitness,
    LeaveOutWitness
}

#[derive(Copy, Clone, Debug)]
struct PatternContext<'tcx> {
    ty: Ty<'tcx>,
    max_slice_length: usize,
}


fn const_val_to_expr(value: &ConstVal) -> P<hir::Expr> {
    let node = match value {
        &ConstVal::Bool(b) => ast::LitKind::Bool(b),
        _ => bug!()
    };
    P(hir::Expr {
        id: DUMMY_NODE_ID,
        node: hir::ExprLit(P(Spanned { node: node, span: DUMMY_SP })),
        span: DUMMY_SP,
        attrs: ast::ThinVec::new(),
    })
}

/// A stack of patterns in reverse order of construction
#[derive(Clone, PartialEq, Eq)]
pub struct Witness(Vec<P<Pat>>);

impl Witness {
    pub fn single_pattern(&self) -> &Pat {
        assert_eq!(self.0.len(), 1);
        &self.0[0]
    }

    fn push_wild_constructor<'a, 'tcx>(
        mut self,
        cx: &MatchCheckCtxt<'a, 'tcx>,
        ctor: &Constructor,
        ty: Ty<'tcx>)
        -> Self
    {
        let arity = constructor_arity(cx, ctor, ty);
        self.0.extend(repeat(DUMMY_WILD_PAT).take(arity).map(|p| P(p.clone())));
        self.apply_constructor(cx, ctor, ty)
    }


    /// Constructs a partial witness for a pattern given a list of
    /// patterns expanded by the specialization step.
    ///
    /// When a pattern P is discovered to be useful, this function is used bottom-up
    /// to reconstruct a complete witness, e.g. a pattern P' that covers a subset
    /// of values, V, where each value in that set is not covered by any previously
    /// used patterns and is covered by the pattern P'. Examples:
    ///
    /// left_ty: tuple of 3 elements
    /// pats: [10, 20, _]           => (10, 20, _)
    ///
    /// left_ty: struct X { a: (bool, &'static str), b: usize}
    /// pats: [(false, "foo"), 42]  => X { a: (false, "foo"), b: 42 }
    fn apply_constructor<'a, 'tcx>(
        mut self,
        cx: &MatchCheckCtxt<'a,'tcx>,
        ctor: &Constructor,
        ty: Ty<'tcx>)
        -> Self
    {
        let arity = constructor_arity(cx, ctor, ty);
        let pat = {
            let len = self.0.len();
            let mut pats = self.0.drain(len-arity..).rev();

            match ty.sty {
                ty::TyTuple(..) => PatKind::Tuple(pats.collect(), None),

                ty::TyAdt(adt, _) => {
                    let v = ctor.variant_for_adt(adt);
                    match v.ctor_kind {
                        CtorKind::Fictive => {
                            let field_pats: hir::HirVec<_> = v.fields.iter()
                                .zip(pats)
                                .filter(|&(_, ref pat)| pat.node != PatKind::Wild)
                                .map(|(field, pat)| Spanned {
                                    span: DUMMY_SP,
                                    node: hir::FieldPat {
                                        name: field.name,
                                        pat: pat,
                                        is_shorthand: false,
                                    }
                                }).collect();
                            let has_more_fields = field_pats.len() < arity;
                            PatKind::Struct(
                                def_to_path(cx.tcx, v.did), field_pats, has_more_fields)
                        }
                        CtorKind::Fn => {
                            PatKind::TupleStruct(
                                def_to_path(cx.tcx, v.did), pats.collect(), None)
                        }
                        CtorKind::Const => {
                            PatKind::Path(None, def_to_path(cx.tcx, v.did))
                        }
                    }
                }

                ty::TyRef(_, ty::TypeAndMut { mutbl, .. }) => {
                    PatKind::Ref(pats.nth(0).unwrap(), mutbl)
                }

                ty::TySlice(_) | ty::TyArray(..) => {
                    PatKind::Slice(pats.collect(), None, hir::HirVec::new())
                }

                _ => {
                    match *ctor {
                        ConstantValue(ref v) => PatKind::Lit(const_val_to_expr(v)),
                        _ => PatKind::Wild,
                    }
                }
            }
        };

        self.0.push(P(hir::Pat {
            id: DUMMY_NODE_ID,
            node: pat,
            span: DUMMY_SP
        }));

        self
    }
}

/// Return the set of constructors from the same type as the first column of `matrix`,
/// that are matched only by wildcard patterns from that first column.
///
/// Therefore, if there is some pattern that is unmatched by `matrix`, it will
/// still be unmatched if the first constructor is replaced by any of the constructors
/// in the return value.
fn missing_constructors(cx: &MatchCheckCtxt, matrix: &Matrix,
                        pcx: PatternContext) -> Vec<Constructor> {
    let used_constructors: Vec<Constructor> =
        matrix.0.iter()
        .flat_map(|row| pat_constructors(cx, row[0], pcx).unwrap_or(vec![]))
        .collect();
    debug!("used_constructors = {:?}", used_constructors);
    all_constructors(cx, pcx).into_iter()
        .filter(|c| !used_constructors.contains(c))
        .collect()
}

/// This determines the set of all possible constructors of a pattern matching
/// values of type `left_ty`. For vectors, this would normally be an infinite set
/// but is instead bounded by the maximum fixed length of slice patterns in
/// the column of patterns being analyzed.
fn all_constructors(_cx: &MatchCheckCtxt, pcx: PatternContext) -> Vec<Constructor> {
    match pcx.ty.sty {
        ty::TyBool =>
            [true, false].iter().map(|b| ConstantValue(ConstVal::Bool(*b))).collect(),
        ty::TySlice(_) =>
            (0..pcx.max_slice_length+1).map(|length| Slice(length)).collect(),
        ty::TyAdt(def, _) if def.is_enum() && def.variants.len() > 1 =>
            def.variants.iter().map(|v| Variant(v.did)).collect(),
        _ => vec![Single]
    }
}

/// Algorithm from http://moscova.inria.fr/~maranget/papers/warn/index.html
///
/// Whether a vector `v` of patterns is 'useful' in relation to a set of such
/// vectors `m` is defined as there being a set of inputs that will match `v`
/// but not any of the sets in `m`.
///
/// This is used both for reachability checking (if a pattern isn't useful in
/// relation to preceding patterns, it is not reachable) and exhaustiveness
/// checking (if a wildcard pattern is useful in relation to a matrix, the
/// matrix isn't exhaustive).
///
/// Note: is_useful doesn't work on empty types, as the paper notes.
/// So it assumes that v is non-empty.
pub fn is_useful<'a, 'tcx>(cx: &MatchCheckCtxt<'a, 'tcx>,
                           matrix: &Matrix<'a, 'tcx>,
                           v: &[&'a Pattern<'tcx>],
                           witness: WitnessPreference)
                           -> Usefulness {
    let &Matrix(ref rows) = matrix;
    debug!("is_useful({:?}, {:?})", matrix, v);
    if rows.is_empty() {
        return match witness {
            ConstructWitness => UsefulWithWitness(vec![Witness(
                repeat(DUMMY_WILD_PAT).take(v.len()).map(|p| P(p.clone())).collect()
            )]),
            LeaveOutWitness => Useful
        };
    }
    if rows[0].is_empty() {
        return NotUseful;
    }
    assert!(rows.iter().all(|r| r.len() == v.len()));

    let pcx = PatternContext {
        ty: rows.iter().map(|r| r[0].ty).find(|ty| !ty.references_error())
            .unwrap_or(v[0].ty),
        max_slice_length: rows.iter().filter_map(|row| match *row[0].kind {
            PatternKind::Slice { ref prefix, slice: _, ref suffix } =>
                Some(prefix.len() + suffix.len()),
            _ => None
        }).max().map_or(0, |v| v + 1)
    };

    debug!("is_useful: pcx={:?}, expanding {:?}", pcx, v[0]);

    if let Some(constructors) = pat_constructors(cx, v[0], pcx) {
        debug!("is_useful - expanding constructors: {:?}", constructors);
        constructors.into_iter().map(|c|
            is_useful_specialized(cx, matrix, v, c.clone(), pcx.ty, witness)
        ).find(|result| result != &NotUseful).unwrap_or(NotUseful)
    } else {
        debug!("is_useful - expanding wildcard");
        let constructors = missing_constructors(cx, matrix, pcx);
        debug!("is_useful - missing_constructors = {:?}", constructors);
        if constructors.is_empty() {
            all_constructors(cx, pcx).into_iter().map(|c| {
                is_useful_specialized(cx, matrix, v, c.clone(), pcx.ty, witness)
            }).find(|result| result != &NotUseful).unwrap_or(NotUseful)
        } else {
            let matrix = rows.iter().filter_map(|r| {
                if r[0].is_wildcard() {
                    Some(r[1..].to_vec())
                } else {
                    None
                }
            }).collect();
            match is_useful(cx, &matrix, &v[1..], witness) {
                UsefulWithWitness(pats) => {
                    UsefulWithWitness(pats.into_iter().flat_map(|witness| {
                        constructors.iter().map(move |ctor| {
                            witness.clone().push_wild_constructor(cx, ctor, pcx.ty)
                        })
                    }).collect())
                }
                result => result
            }
        }
    }
}

fn is_useful_specialized<'a, 'tcx>(
    cx: &MatchCheckCtxt<'a, 'tcx>,
    &Matrix(ref m): &Matrix<'a, 'tcx>,
    v: &[&'a Pattern<'tcx>],
    ctor: Constructor,
    lty: Ty<'tcx>,
    witness: WitnessPreference) -> Usefulness
{
    let arity = constructor_arity(cx, &ctor, lty);
    let matrix = Matrix(m.iter().filter_map(|r| {
        specialize(cx, &r[..], &ctor, 0, arity)
    }).collect());
    match specialize(cx, v, &ctor, 0, arity) {
        Some(v) => match is_useful(cx, &matrix, &v[..], witness) {
            UsefulWithWitness(witnesses) => UsefulWithWitness(
                witnesses.into_iter()
                    .map(|witness| witness.apply_constructor(cx, &ctor, lty))
                    .collect()
            ),
            result => result
        },
        None => NotUseful
    }
}

/// Determines the constructors that the given pattern can be specialized to.
///
/// In most cases, there's only one constructor that a specific pattern
/// represents, such as a specific enum variant or a specific literal value.
/// Slice patterns, however, can match slices of different lengths. For instance,
/// `[a, b, ..tail]` can match a slice of length 2, 3, 4 and so on.
///
/// Returns None in case of a catch-all, which can't be specialized.
fn pat_constructors(_cx: &MatchCheckCtxt,
                    pat: &Pattern,
                    pcx: PatternContext)
                    -> Option<Vec<Constructor>>
{
    match *pat.kind {
        PatternKind::Binding { .. } | PatternKind::Wild =>
            None,
        PatternKind::Leaf { .. } | PatternKind::Deref { .. } | PatternKind::Array { .. } =>
            Some(vec![Single]),
        PatternKind::Variant { adt_def, variant_index, .. } =>
            Some(vec![Variant(adt_def.variants[variant_index].did)]),
        PatternKind::Constant { ref value } =>
            Some(vec![ConstantValue(value.clone())]),
        PatternKind::Range { ref lo, ref hi } =>
            Some(vec![ConstantRange(lo.clone(), hi.clone())]),
        PatternKind::Slice { ref prefix, ref slice, ref suffix } => {
            let pat_len = prefix.len() + suffix.len();
            if slice.is_some() {
                Some((pat_len..pcx.max_slice_length+1).map(Slice).collect())
            } else {
                Some(vec![Slice(pat_len)])
            }
        }
    }
}

/// This computes the arity of a constructor. The arity of a constructor
/// is how many subpattern patterns of that constructor should be expanded to.
///
/// For instance, a tuple pattern (_, 42, Some([])) has the arity of 3.
/// A struct pattern's arity is the number of fields it contains, etc.
fn constructor_arity(_cx: &MatchCheckCtxt, ctor: &Constructor, ty: Ty) -> usize {
    debug!("constructor_arity({:?}, {:?})", ctor, ty);
    match ty.sty {
        ty::TyTuple(ref fs) => fs.len(),
        ty::TyBox(_) => 1,
        ty::TySlice(_) => match *ctor {
            Slice(length) => length,
            ConstantValue(_) => {
                // TODO: this is utterly wrong, but required for byte arrays
                0
            }
            _ => bug!("bad slice pattern {:?} {:?}", ctor, ty)
        },
        ty::TyRef(..) => 1,
        ty::TyAdt(adt, _) => {
            ctor.variant_for_adt(adt).fields.len()
        }
        ty::TyArray(_, n) => n,
        _ => 0
    }
}

fn range_covered_by_constructor(tcx: TyCtxt, span: Span,
                                ctor: &Constructor,
                                from: &ConstVal, to: &ConstVal)
                                -> Result<bool, ErrorReported> {
    let (c_from, c_to) = match *ctor {
        ConstantValue(ref value)        => (value, value),
        ConstantRange(ref from, ref to) => (from, to),
        Single                          => return Ok(true),
        _                               => bug!()
    };
    let cmp_from = compare_const_vals(tcx, span, c_from, from)?;
    let cmp_to = compare_const_vals(tcx, span, c_to, to)?;
    Ok(cmp_from != Ordering::Less && cmp_to != Ordering::Greater)
}

fn patterns_for_variant<'a, 'tcx>(
    cx: &MatchCheckCtxt<'a, 'tcx>,
    subpatterns: &'a [FieldPattern<'tcx>],
    arity: usize)
    -> Vec<&'a Pattern<'tcx>>
{
    let mut result = vec![cx.wild_pattern; arity];

    for subpat in subpatterns {
        result[subpat.field.index()] = &subpat.pattern;
    }

    debug!("patterns_for_variant({:?}, {:?}) = {:?}", subpatterns, arity, result);
    result
}

/// This is the main specialization step. It expands the first pattern in the given row
/// into `arity` patterns based on the constructor. For most patterns, the step is trivial,
/// for instance tuple patterns are flattened and box patterns expand into their inner pattern.
///
/// OTOH, slice patterns with a subslice pattern (..tail) can be expanded into multiple
/// different patterns.
/// Structure patterns with a partial wild pattern (Foo { a: 42, .. }) have their missing
/// fields filled with wild patterns.
fn specialize<'a, 'tcx>(
    cx: &MatchCheckCtxt<'a, 'tcx>,
    r: &[&'a Pattern<'tcx>],
    constructor: &Constructor, col: usize, arity: usize)
    -> Option<Vec<&'a Pattern<'tcx>>>
{
    let pat = &r[col];

    let head: Option<Vec<&Pattern>> = match *pat.kind {
        PatternKind::Binding { .. } | PatternKind::Wild =>
            Some(vec![cx.wild_pattern; arity]),

        PatternKind::Variant { adt_def, variant_index, ref subpatterns } => {
            let ref variant = adt_def.variants[variant_index];
            if *constructor == Variant(variant.did) {
                Some(patterns_for_variant(cx, subpatterns, arity))
            } else {
                None
            }
        }

        PatternKind::Leaf { ref subpatterns } => Some(patterns_for_variant(cx, subpatterns, arity)),
        PatternKind::Deref { ref subpattern } => Some(vec![subpattern]),

        PatternKind::Constant { ref value } => {
            assert_eq!(constructor_arity(cx, constructor, pat.ty), 0);
            match range_covered_by_constructor(
                cx.tcx, pat.span, constructor, value, value
            ) {
                Ok(true) => Some(vec![]),
                Ok(false) => None,
                Err(ErrorReported) => None,
            }
        }

        PatternKind::Range { ref lo, ref hi } => {
            match range_covered_by_constructor(
                cx.tcx, pat.span, constructor, lo, hi
            ) {
                Ok(true) => Some(vec![]),
                Ok(false) => None,
                Err(ErrorReported) => None,
            }
        }

        PatternKind::Array { ref prefix, slice: _, ref suffix } => {
            let pat_len = prefix.len() + suffix.len();
            Some(
                prefix.iter().chain(
                repeat(cx.wild_pattern).take(arity - pat_len).chain(
                suffix.iter()
            )).collect())
        }

        PatternKind::Slice { ref prefix, ref slice, ref suffix } => {
            let pat_len = prefix.len() + suffix.len();
            if let Some(slice_count) = arity.checked_sub(pat_len) {
                if slice_count == 0 || slice.is_some() {
                    Some(
                        prefix.iter().chain(
                        repeat(cx.wild_pattern).take(slice_count).chain(
                        suffix.iter()
                    )).collect())
                } else {
                    None
                }
            } else {
                None
            }
        }
    };
    debug!("specialize({:?}, {:?}) = {:?}", r[col], arity, head);

    head.map(|mut head| {
        head.extend_from_slice(&r[..col]);
        head.extend_from_slice(&r[col + 1..]);
        head
    })
}

pub fn is_refutable<'a, 'tcx, A, F>(
    cx: &MatchCheckCtxt<'a, 'tcx>,
    pat: &'a Pattern<'tcx>,
    refutable: F)
    -> Option<A> where
    F: FnOnce(&Witness) -> A,
{
    let pats = Matrix(vec![vec![pat]]);
    match is_useful(cx, &pats, &[cx.wild_pattern], ConstructWitness) {
        UsefulWithWitness(pats) => Some(refutable(&pats[0])),
        NotUseful => None,
        Useful => bug!()
    }
}
