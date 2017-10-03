// Copyright 2012-2015 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use rustc::ty::{self, TyCtxt};
use rustc_errors::DiagnosticBuilder;
use syntax_pos::{MultiSpan, Span};

use std::fmt;

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Origin { Ast, Mir }

impl fmt::Display for Origin {
    fn fmt(&self, w: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Origin::Mir => write!(w, " (Mir)"),
            Origin::Ast => ty::tls::with_opt(|opt_tcx| {
                // If user passed `-Z borrowck-mir`, then include an
                // AST origin as part of the error report
                if let Some(tcx) = opt_tcx {
                    if tcx.sess.opts.debugging_opts.borrowck_mir {
                        return write!(w, " (Ast)");
                    }
                }
                // otherwise, do not include the origin (i.e., print
                // nothing at all)
                Ok(())
            }),
        }
    }
}

pub trait BorrowckErrors {
    fn struct_span_err_with_code<'a, S: Into<MultiSpan>>(&'a self,
                                                         sp: S,
                                                         msg: &str,
                                                         code: &str)
                                                         -> DiagnosticBuilder<'a>;

    fn struct_span_err<'a, S: Into<MultiSpan>>(&'a self,
                                               sp: S,
                                               msg: &str)
                                               -> DiagnosticBuilder<'a>;

    fn cannot_move_when_borrowed(&self, span: Span, desc: &str, o: Origin)
                                 -> DiagnosticBuilder
    {
        struct_span_err!(self, span, E0505,
                         "cannot move out of `{}` because it is borrowed{OGN}",
                         desc, OGN=o)
    }

    fn cannot_use_when_mutably_borrowed(&self,
                                        span: Span,
                                        desc: &str,
                                        borrow_span: Span,
                                        borrow_desc: &str,
                                        o: Origin)
                                        -> DiagnosticBuilder
    {
        let mut err = struct_span_err!(self, span, E0503,
                         "cannot use `{}` because it was mutably borrowed{OGN}",
                         desc, OGN=o);

        err.span_label(borrow_span, format!("borrow of `{}` occurs here", borrow_desc));
        err.span_label(span, format!("use of borrowed `{}`", borrow_desc));

        err
    }

    fn cannot_act_on_uninitialized_variable(&self,
                                            span: Span,
                                            verb: &str,
                                            desc: &str,
                                            o: Origin)
                                            -> DiagnosticBuilder
    {
        struct_span_err!(self, span, E0381,
                         "{} of possibly uninitialized variable: `{}`{OGN}",
                         verb, desc, OGN=o)
    }

    fn cannot_mutably_borrow_multiply(&self,
                                      span: Span,
                                      desc: &str,
                                      opt_via: &str,
                                      o: Origin)
                                      -> DiagnosticBuilder
    {
        struct_span_err!(self, span, E0499,
                         "cannot borrow `{}`{} as mutable more than once at a time{OGN}",
                         desc, opt_via, OGN=o)
    }

    fn cannot_uniquely_borrow_by_two_closures(&self, span: Span, desc: &str, o: Origin)
                                              -> DiagnosticBuilder
    {
        struct_span_err!(self, span, E0524,
                         "two closures require unique access to `{}` at the same time{OGN}",
                         desc, OGN=o)
    }

    fn cannot_uniquely_borrow_by_one_closure(&self,
                                             span: Span,
                                             desc_new: &str,
                                             noun_old: &str,
                                             msg_old: &str,
                                             o: Origin)
                                             -> DiagnosticBuilder
    {
        struct_span_err!(self, span, E0500,
                         "closure requires unique access to `{}` but {} is already borrowed{}{OGN}",
                         desc_new, noun_old, msg_old, OGN=o)
    }

    fn cannot_reborrow_already_uniquely_borrowed(&self,
                                                 span: Span,
                                                 desc_new: &str,
                                                 msg_new: &str,
                                                 kind_new: &str,
                                                 o: Origin)
                                                 -> DiagnosticBuilder
    {
        struct_span_err!(self, span, E0501,
                         "cannot borrow `{}`{} as {} because previous closure \
                          requires unique access{OGN}",
                         desc_new, msg_new, kind_new, OGN=o)
    }

    fn cannot_reborrow_already_borrowed(&self,
                                        span: Span,
                                        desc_new: &str,
                                        msg_new: &str,
                                        kind_new: &str,
                                        noun_old: &str,
                                        kind_old: &str,
                                        msg_old: &str,
                                        o: Origin)
                                        -> DiagnosticBuilder
    {
        struct_span_err!(self, span, E0502,
                         "cannot borrow `{}`{} as {} because {} is also borrowed as {}{}{OGN}",
                         desc_new, msg_new, kind_new, noun_old, kind_old, msg_old, OGN=o)
    }

    fn cannot_assign_to_borrowed(&self, span: Span, borrow_span: Span, desc: &str, o: Origin)
                                 -> DiagnosticBuilder
    {
        let mut err = struct_span_err!(self, span, E0506,
                         "cannot assign to `{}` because it is borrowed{OGN}",
                         desc, OGN=o);

        err.span_label(borrow_span, format!("borrow of `{}` occurs here", desc));
        err.span_label(span, format!("assignment to borrowed `{}` occurs here", desc));

        err
    }

    fn cannot_move_into_closure(&self, span: Span, desc: &str, o: Origin)
                                -> DiagnosticBuilder
    {
        struct_span_err!(self, span, E0504,
                         "cannot move `{}` into closure because it is borrowed{OGN}",
                         desc, OGN=o)
    }

    fn cannot_reassign_immutable(&self, span: Span, desc: &str, o: Origin)
                                 -> DiagnosticBuilder
    {
        struct_span_err!(self, span, E0384,
                         "re-assignment of immutable variable `{}`{OGN}",
                         desc, OGN=o)
    }

    fn cannot_assign(&self, span: Span, desc: &str, o: Origin) -> DiagnosticBuilder
    {
        struct_span_err!(self, span, E0594,
                         "cannot assign to {}{OGN}",
                         desc, OGN=o)
    }

    fn cannot_assign_static(&self, span: Span, desc: &str, o: Origin)
                            -> DiagnosticBuilder
    {
        self.cannot_assign(span, &format!("immutable static item `{}`", desc), o)
    }

    fn cannot_move_out_of(&self, move_from_span: Span, move_from_desc: &str, o: Origin)
                          -> DiagnosticBuilder
    {
        let mut err = struct_span_err!(self, move_from_span, E0507,
                                       "cannot move out of {}{OGN}",
                                       move_from_desc, OGN=o);
        err.span_label(
            move_from_span,
            format!("cannot move out of {}", move_from_desc));
        err
    }

    fn cannot_move_out_of_interior_noncopy(&self,
                                           move_from_span: Span,
                                           ty: ty::Ty,
                                           is_index: bool,
                                           o: Origin)
                                           -> DiagnosticBuilder
    {
        let type_name = match (&ty.sty, is_index) {
            (&ty::TyArray(_, _), true) => "array",
            (&ty::TySlice(_),    _) => "slice",
            _ => span_bug!(move_from_span, "this path should not cause illegal move"),
        };
        let mut err = struct_span_err!(self, move_from_span, E0508,
                                       "cannot move out of type `{}`, \
                                        a non-copy {}{OGN}",
                                       ty, type_name, OGN=o);
        err.span_label(move_from_span, "cannot move out of here");
        err
    }

    fn cannot_move_out_of_interior_of_drop(&self,
                                           move_from_span: Span,
                                           container_ty: ty::Ty,
                                           o: Origin)
                                           -> DiagnosticBuilder
    {
        let mut err = struct_span_err!(self, move_from_span, E0509,
                                       "cannot move out of type `{}`, \
                                        which implements the `Drop` trait{OGN}",
                                       container_ty, OGN=o);
        err.span_label(move_from_span, "cannot move out of here");
        err
    }
}

impl<'b, 'tcx, 'gcx> BorrowckErrors for TyCtxt<'b, 'tcx, 'gcx> {
    fn struct_span_err_with_code<'a, S: Into<MultiSpan>>(&'a self,
                                                         sp: S,
                                                         msg: &str,
                                                         code: &str)
                                                         -> DiagnosticBuilder<'a>
    {
        self.sess.struct_span_err_with_code(sp, msg, code)
    }

    fn struct_span_err<'a, S: Into<MultiSpan>>(&'a self,
                                               sp: S,
                                               msg: &str)
                                               -> DiagnosticBuilder<'a>
    {
        self.sess.struct_span_err(sp, msg)
    }
}
