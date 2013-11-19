// Copyright 2012 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

/*!
Support for parsing unsupported, old syntaxes, for the
purpose of reporting errors. Parsing of these syntaxes
is tested by compile-test/obsolete-syntax.rs.

Obsolete syntax that becomes too hard to parse can be
removed.
*/

use ast::{Expr, ExprLit, lit_nil};
use codemap::{Span, respan};
use parse::parser::Parser;
use parse::token::Token;
use parse::token;

use std::str;
use std::to_bytes;

/// The specific types of unsupported syntax
#[deriving(Eq)]
pub enum ObsoleteSyntax {
    ObsoleteSwap,
    ObsoleteUnsafeBlock,
    ObsoleteBareFnType,
    ObsoleteNamedExternModule,
    ObsoleteMultipleLocalDecl,
    ObsoleteUnsafeExternFn,
    ObsoleteTraitFuncVisibility,
    ObsoleteConstPointer,
    ObsoleteEmptyImpl,
    ObsoleteLoopAsContinue,
    ObsoleteEnumWildcard,
    ObsoleteStructWildcard,
    ObsoleteVecDotDotWildcard,
    ObsoleteBoxedClosure,
}

impl to_bytes::IterBytes for ObsoleteSyntax {
    #[inline]
    fn iter_bytes(&self, lsb0: bool, f: to_bytes::Cb) -> bool {
        (*self as uint).iter_bytes(lsb0, f)
    }
}

pub trait ParserObsoleteMethods {
    /// Reports an obsolete syntax non-fatal error.
    fn obsolete(&self, sp: Span, kind: ObsoleteSyntax);
    // Reports an obsolete syntax non-fatal error, and returns
    // a placeholder expression
    fn obsolete_expr(&self, sp: Span, kind: ObsoleteSyntax) -> @Expr;
    fn report(&self,
              sp: Span,
              kind: ObsoleteSyntax,
              kind_str: &str,
              desc: &str);
    fn token_is_obsolete_ident(&self, ident: &str, token: &Token) -> bool;
    fn is_obsolete_ident(&self, ident: &str) -> bool;
    fn eat_obsolete_ident(&self, ident: &str) -> bool;
}

impl ParserObsoleteMethods for Parser {
    /// Reports an obsolete syntax non-fatal error.
    fn obsolete(&self, sp: Span, kind: ObsoleteSyntax) {
        let (kind_str, desc) = match kind {
            ObsoleteSwap => (
                "swap",
                "Use std::util::{swap, replace} instead"
            ),
            ObsoleteUnsafeBlock => (
                "non-standalone unsafe block",
                "use an inner `unsafe { ... }` block instead"
            ),
            ObsoleteBareFnType => (
                "bare function type",
                "use `|A| -> B` or `extern fn(A) -> B` instead"
            ),
            ObsoleteNamedExternModule => (
                "named external module",
                "instead of `extern mod foo { ... }`, write `mod foo { \
                 extern { ... } }`"
            ),
            ObsoleteMultipleLocalDecl => (
                "declaration of multiple locals at once",
                "instead of e.g. `let a = 1, b = 2`, write \
                 `let (a, b) = (1, 2)`."
            ),
            ObsoleteUnsafeExternFn => (
                "unsafe external function",
                "external functions are always unsafe; remove the `unsafe` \
                 keyword"
            ),
            ObsoleteTraitFuncVisibility => (
                "visibility not necessary",
                "trait functions inherit the visibility of the trait itself"
            ),
            ObsoleteConstPointer => (
                "const pointer",
                "instead of `&const Foo` or `@const Foo`, write `&Foo` or \
                 `@Foo`"
            ),
            ObsoleteEmptyImpl => (
                "empty implementation",
                "instead of `impl A;`, write `impl A {}`"
            ),
            ObsoleteLoopAsContinue => (
                "`loop` instead of `continue`",
                "`loop` is now only used for loops and `continue` is used for \
                 skipping iterations"
            ),
            ObsoleteEnumWildcard => (
                "enum wildcard",
                "use `..` instead of `*` for matching all enum fields"
            ),
            ObsoleteStructWildcard => (
                "struct wildcard",
                "use `..` instead of `_` for matching trailing struct fields"
            ),
            ObsoleteVecDotDotWildcard => (
                "vec slice wildcard",
                "use `..` instead of `.._` for matching slices"
            ),
            ObsoleteBoxedClosure => (
                "managed or owned closure",
                "managed closures have been removed and owned closures are \
                 now written `proc()`"
            ),
        };

        self.report(sp, kind, kind_str, desc);
    }

    // Reports an obsolete syntax non-fatal error, and returns
    // a placeholder expression
    fn obsolete_expr(&self, sp: Span, kind: ObsoleteSyntax) -> @Expr {
        self.obsolete(sp, kind);
        self.mk_expr(sp.lo, sp.hi, ExprLit(@respan(sp, lit_nil)))
    }

    fn report(&self,
              sp: Span,
              kind: ObsoleteSyntax,
              kind_str: &str,
              desc: &str) {
        self.span_err(sp, format!("obsolete syntax: {}", kind_str));

        if !self.obsolete_set.contains(&kind) {
            self.sess.span_diagnostic.handler().note(format!("{}", desc));
            self.obsolete_set.insert(kind);
        }
    }

    fn token_is_obsolete_ident(&self, ident: &str, token: &Token)
                                   -> bool {
        match *token {
            token::IDENT(sid, _) => {
                str::eq_slice(self.id_to_str(sid), ident)
            }
            _ => false
        }
    }

    fn is_obsolete_ident(&self, ident: &str) -> bool {
        self.token_is_obsolete_ident(ident, self.token)
    }

    fn eat_obsolete_ident(&self, ident: &str) -> bool {
        if self.is_obsolete_ident(ident) {
            self.bump();
            true
        } else {
            false
        }
    }
}
