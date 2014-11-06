// Copyright 2012-2013 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

pub use self::BinOpToken::*;
pub use self::Nonterminal::*;
pub use self::DelimToken::*;
pub use self::IdentStyle::*;
pub use self::Token::*;

use ast;
use ext::mtwt;
use ptr::P;
use util::interner::{RcStr, StrInterner};
use util::interner;

use serialize::{Decodable, Decoder, Encodable, Encoder};
use std::fmt;
use std::mem;
use std::path::BytesContainer;
use std::rc::Rc;

#[allow(non_camel_case_types)]
#[deriving(Clone, Encodable, Decodable, PartialEq, Eq, Hash, Show)]
pub enum BinOpToken {
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Caret,
    And,
    Or,
    Shl,
    Shr,
}

/// A delimeter token
#[deriving(Clone, Encodable, Decodable, PartialEq, Eq, Hash, Show)]
pub enum DelimToken {
    /// A round parenthesis: `(` or `)`
    Paren,
    /// A square bracket: `[` or `]`
    Bracket,
    /// A curly brace: `{` or `}`
    Brace,
}

#[deriving(Clone, Encodable, Decodable, PartialEq, Eq, Hash, Show)]
pub enum IdentStyle {
    /// `::` follows the identifier with no whitespace in-between.
    ModName,
    Plain,
}

#[allow(non_camel_case_types)]
#[deriving(Clone, Encodable, Decodable, PartialEq, Eq, Hash, Show)]
pub enum Token {
    /* Expression-operator symbols. */
    Eq,
    Lt,
    Le,
    EqEq,
    Ne,
    Ge,
    Gt,
    AndAnd,
    OrOr,
    Not,
    Tilde,
    BinOp(BinOpToken),
    BinOpEq(BinOpToken),

    /* Structural symbols */
    At,
    Dot,
    DotDot,
    DotDotDot,
    Comma,
    Semi,
    Colon,
    ModSep,
    RArrow,
    LArrow,
    FatArrow,
    Pound,
    Dollar,
    Question,
    /// An opening delimeter, eg. `{`
    OpenDelim(DelimToken),
    /// A closing delimeter, eg. `}`
    CloseDelim(DelimToken),

    /* Literals */
    LitByte(ast::Name),
    LitChar(ast::Name),
    LitInteger(ast::Name),
    LitFloat(ast::Name),
    LitStr(ast::Name),
    LitStrRaw(ast::Name, uint), /* raw str delimited by n hash symbols */
    LitBinary(ast::Name),
    LitBinaryRaw(ast::Name, uint), /* raw binary str delimited by n hash symbols */

    /* Name components */
    Ident(ast::Ident, IdentStyle),
    Underscore,
    Lifetime(ast::Ident),

    /* For interpolation */
    Interpolated(Nonterminal),
    // Can be expanded into several tokens.
    /// Doc comment
    DocComment(ast::Name),
    // In left-hand-sides of MBE macros:
    /// Parse a nonterminal (name to bind, name of NT, styles of their idents)
    MatchNt(ast::Ident, ast::Ident, IdentStyle, IdentStyle),
    // In right-hand-sides of MBE macros:
    /// A syntactic variable that will be filled in by macro expansion.
    SubstNt(ast::Ident, IdentStyle),

    // Junk. These carry no data because we don't really care about the data
    // they *would* carry, and don't really want to allocate a new ident for
    // them. Instead, users could extract that from the associated span.

    /// Whitespace
    Whitespace,
    /// Comment
    Comment,
    Shebang(ast::Name),

    Eof,
}

impl Token {
    /// Returns `true` if the token can appear at the start of an expression.
    pub fn can_begin_expr(&self) -> bool {
        match *self {
            OpenDelim(_)                => true,
            Ident(_, _)                 => true,
            Underscore                  => true,
            Tilde                       => true,
            LitByte(_)                  => true,
            LitChar(_)                  => true,
            LitInteger(_)               => true,
            LitFloat(_)                 => true,
            LitStr(_)                   => true,
            LitStrRaw(_, _)             => true,
            LitBinary(_)                => true,
            LitBinaryRaw(_, _)          => true,
            Pound                       => true,
            At                          => true,
            Not                         => true,
            BinOp(Minus)                => true,
            BinOp(Star)                 => true,
            BinOp(And)                  => true,
            BinOp(Or)                   => true, // in lambda syntax
            OrOr                        => true, // in lambda syntax
            ModSep                      => true,
            Interpolated(NtExpr(..))    => true,
            Interpolated(NtIdent(..))   => true,
            Interpolated(NtBlock(..))   => true,
            Interpolated(NtPath(..))    => true,
            _                           => false,
        }
    }

    /// Returns `true` if the token is any literal
    pub fn is_lit(&self) -> bool {
        match *self {
            LitByte(_)          => true,
            LitChar(_)          => true,
            LitInteger(_)       => true,
            LitFloat(_)         => true,
            LitStr(_)           => true,
            LitStrRaw(_, _)     => true,
            LitBinary(_)        => true,
            LitBinaryRaw(_, _)  => true,
            _                   => false,
        }
    }

    /// Returns `true` if the token is an identifier.
    pub fn is_ident(&self) -> bool {
        match *self {
            Ident(_, _) => true,
            _           => false,
        }
    }

    /// Returns `true` if the token is an interpolated path.
    pub fn is_path(&self) -> bool {
        match *self {
            Interpolated(NtPath(..))    => true,
            _                           => false,
        }
    }

    /// Returns `true` if the token is a path that is not followed by a `::`
    /// token.
    #[allow(non_upper_case_globals)]
    pub fn is_plain_ident(&self) -> bool {
        match *self {
            Ident(_, Plain) => true,
            _               => false,
        }
    }

    /// Returns `true` if the token is a lifetime.
    pub fn is_lifetime(&self) -> bool {
        match *self {
            Lifetime(..) => true,
            _            => false,
        }
    }

    /// Returns `true` if the token is either the `mut` or `const` keyword.
    pub fn is_mutability(&self) -> bool {
        self.is_keyword(keywords::Mut) ||
        self.is_keyword(keywords::Const)
    }

    /// Maps a token to its corresponding binary operator.
    pub fn to_binop(&self) -> Option<ast::BinOp> {
        match *self {
            BinOp(Star)     => Some(ast::BiMul),
            BinOp(Slash)    => Some(ast::BiDiv),
            BinOp(Percent)  => Some(ast::BiRem),
            BinOp(Plus)     => Some(ast::BiAdd),
            BinOp(Minus)    => Some(ast::BiSub),
            BinOp(Shl)      => Some(ast::BiShl),
            BinOp(Shr)      => Some(ast::BiShr),
            BinOp(And)      => Some(ast::BiBitAnd),
            BinOp(Caret)    => Some(ast::BiBitXor),
            BinOp(Or)       => Some(ast::BiBitOr),
            Lt              => Some(ast::BiLt),
            Le              => Some(ast::BiLe),
            Ge              => Some(ast::BiGe),
            Gt              => Some(ast::BiGt),
            EqEq            => Some(ast::BiEq),
            Ne              => Some(ast::BiNe),
            AndAnd          => Some(ast::BiAnd),
            OrOr            => Some(ast::BiOr),
            _               => None,
        }
    }

    /// Returns `true` if the token is a given keyword, `kw`.
    #[allow(non_upper_case_globals)]
    pub fn is_keyword(&self, kw: keywords::Keyword) -> bool {
        match *self {
            Ident(sid, Plain) => kw.to_name() == sid.name,
            _                      => false,
        }
    }

    /// Returns `true` if the token is either a special identifier, or a strict
    /// or reserved keyword.
    #[allow(non_upper_case_globals)]
    pub fn is_any_keyword(&self) -> bool {
        match *self {
            Ident(sid, Plain) => {
                let n = sid.name;

                   n == SELF_KEYWORD_NAME
                || n == STATIC_KEYWORD_NAME
                || n == SUPER_KEYWORD_NAME
                || STRICT_KEYWORD_START <= n
                && n <= RESERVED_KEYWORD_FINAL
            },
            _ => false
        }
    }

    /// Returns `true` if the token may not appear as an identifier.
    #[allow(non_upper_case_globals)]
    pub fn is_strict_keyword(&self) -> bool {
        match *self {
            Ident(sid, Plain) => {
                let n = sid.name;

                   n == SELF_KEYWORD_NAME
                || n == STATIC_KEYWORD_NAME
                || n == SUPER_KEYWORD_NAME
                || STRICT_KEYWORD_START <= n
                && n <= STRICT_KEYWORD_FINAL
            },
            Ident(sid, ModName) => {
                let n = sid.name;

                   n != SELF_KEYWORD_NAME
                && n != SUPER_KEYWORD_NAME
                && STRICT_KEYWORD_START <= n
                && n <= STRICT_KEYWORD_FINAL
            }
            _ => false,
        }
    }

    /// Returns `true` if the token is a keyword that has been reserved for
    /// possible future use.
    #[allow(non_upper_case_globals)]
    pub fn is_reserved_keyword(&self) -> bool {
        match *self {
            Ident(sid, Plain) => {
                let n = sid.name;

                   RESERVED_KEYWORD_START <= n
                && n <= RESERVED_KEYWORD_FINAL
            },
            _ => false,
        }
    }

    /// Hygienic identifier equality comparison.
    ///
    /// See `styntax::ext::mtwt`.
    pub fn mtwt_eq(&self, other : &Token) -> bool {
        match (self, other) {
            (&Ident(id1,_), &Ident(id2,_)) | (&Lifetime(id1), &Lifetime(id2)) =>
                mtwt::resolve(id1) == mtwt::resolve(id2),
            _ => *self == *other
        }
    }
}

#[deriving(Clone, Encodable, Decodable, PartialEq, Eq, Hash)]
/// For interpolation during macro expansion.
pub enum Nonterminal {
    NtItem(P<ast::Item>),
    NtBlock(P<ast::Block>),
    NtStmt(P<ast::Stmt>),
    NtPat(P<ast::Pat>),
    NtExpr(P<ast::Expr>),
    NtTy(P<ast::Ty>),
    NtIdent(Box<ast::Ident>, IdentStyle),
    /// Stuff inside brackets for attributes
    NtMeta(P<ast::MetaItem>),
    NtPath(Box<ast::Path>),
    NtTT(P<ast::TokenTree>), // needs P'ed to break a circularity
}

impl fmt::Show for Nonterminal {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            NtItem(..) => f.pad("NtItem(..)"),
            NtBlock(..) => f.pad("NtBlock(..)"),
            NtStmt(..) => f.pad("NtStmt(..)"),
            NtPat(..) => f.pad("NtPat(..)"),
            NtExpr(..) => f.pad("NtExpr(..)"),
            NtTy(..) => f.pad("NtTy(..)"),
            NtIdent(..) => f.pad("NtIdent(..)"),
            NtMeta(..) => f.pad("NtMeta(..)"),
            NtPath(..) => f.pad("NtPath(..)"),
            NtTT(..) => f.pad("NtTT(..)"),
        }
    }
}

// Get the first "argument"
macro_rules! first {
    ( $first:expr, $( $remainder:expr, )* ) => ( $first )
}

// Get the last "argument" (has to be done recursively to avoid phoney local ambiguity error)
macro_rules! last {
    ( $first:expr, $( $remainder:expr, )+ ) => ( last!( $( $remainder, )+ ) );
    ( $first:expr, ) => ( $first )
}

// In this macro, there is the requirement that the name (the number) must be monotonically
// increasing by one in the special identifiers, starting at 0; the same holds for the keywords,
// except starting from the next number instead of zero, and with the additional exception that
// special identifiers are *also* allowed (they are deduplicated in the important place, the
// interner), an exception which is demonstrated by "static" and "self".
macro_rules! declare_special_idents_and_keywords {(
    // So now, in these rules, why is each definition parenthesised?
    // Answer: otherwise we get a spurious local ambiguity bug on the "}"
    pub mod special_idents {
        $( ($si_name:expr, $si_static:ident, $si_str:expr); )*
    }

    pub mod keywords {
        'strict:
        $( ($sk_name:expr, $sk_variant:ident, $sk_str:expr); )*
        'reserved:
        $( ($rk_name:expr, $rk_variant:ident, $rk_str:expr); )*
    }
) => {
    static STRICT_KEYWORD_START: ast::Name = first!($( ast::Name($sk_name), )*);
    static STRICT_KEYWORD_FINAL: ast::Name = last!($( ast::Name($sk_name), )*);
    static RESERVED_KEYWORD_START: ast::Name = first!($( ast::Name($rk_name), )*);
    static RESERVED_KEYWORD_FINAL: ast::Name = last!($( ast::Name($rk_name), )*);

    pub mod special_idents {
        use ast;
        $(
            #[allow(non_upper_case_globals)]
            pub const $si_static: ast::Ident = ast::Ident {
                name: ast::Name($si_name),
                ctxt: 0,
            };
         )*
    }

    pub mod special_names {
        use ast;
        $(
            #[allow(non_upper_case_globals)]
            pub const $si_static: ast::Name =  ast::Name($si_name);
        )*
    }

    /**
     * All the valid words that have meaning in the Rust language.
     *
     * Rust keywords are either 'strict' or 'reserved'.  Strict keywords may not
     * appear as identifiers at all. Reserved keywords are not used anywhere in
     * the language and may not appear as identifiers.
     */
    pub mod keywords {
        pub use self::Keyword::*;
        use ast;

        pub enum Keyword {
            $( $sk_variant, )*
            $( $rk_variant, )*
        }

        impl Keyword {
            pub fn to_name(&self) -> ast::Name {
                match *self {
                    $( $sk_variant => ast::Name($sk_name), )*
                    $( $rk_variant => ast::Name($rk_name), )*
                }
            }
        }
    }

    fn mk_fresh_ident_interner() -> IdentInterner {
        // The indices here must correspond to the numbers in
        // special_idents, in Keyword to_name(), and in static
        // constants below.
        let mut init_vec = Vec::new();
        $(init_vec.push($si_str);)*
        $(init_vec.push($sk_str);)*
        $(init_vec.push($rk_str);)*
        interner::StrInterner::prefill(init_vec.as_slice())
    }
}}

// If the special idents get renumbered, remember to modify these two as appropriate
pub const SELF_KEYWORD_NAME: ast::Name = ast::Name(SELF_KEYWORD_NAME_NUM);
const STATIC_KEYWORD_NAME: ast::Name = ast::Name(STATIC_KEYWORD_NAME_NUM);
const SUPER_KEYWORD_NAME: ast::Name = ast::Name(SUPER_KEYWORD_NAME_NUM);

pub const SELF_KEYWORD_NAME_NUM: u32 = 1;
const STATIC_KEYWORD_NAME_NUM: u32 = 2;
const SUPER_KEYWORD_NAME_NUM: u32 = 3;

// NB: leaving holes in the ident table is bad! a different ident will get
// interned with the id from the hole, but it will be between the min and max
// of the reserved words, and thus tagged as "reserved".

declare_special_idents_and_keywords! {
    pub mod special_idents {
        // These ones are statics
        (0,                          invalid,                "");
        (super::SELF_KEYWORD_NAME_NUM,   self_,              "self");
        (super::STATIC_KEYWORD_NAME_NUM, statik,             "static");
        (super::SUPER_KEYWORD_NAME_NUM, super_,              "super");
        (4,                          static_lifetime,        "'static");

        // for matcher NTs
        (5,                          tt,                     "tt");
        (6,                          matchers,               "matchers");

        // outside of libsyntax
        (7,                          clownshoe_abi,          "__rust_abi");
        (8,                          opaque,                 "<opaque>");
        (9,                          unnamed_field,          "<unnamed_field>");
        (10,                         type_self,              "Self");
        (11,                         prelude_import,         "prelude_import");
    }

    pub mod keywords {
        // These ones are variants of the Keyword enum

        'strict:
        (12,                         As,         "as");
        (13,                         Break,      "break");
        (14,                         Crate,      "crate");
        (15,                         Else,       "else");
        (16,                         Enum,       "enum");
        (17,                         Extern,     "extern");
        (18,                         False,      "false");
        (19,                         Fn,         "fn");
        (20,                         For,        "for");
        (21,                         If,         "if");
        (22,                         Impl,       "impl");
        (23,                         In,         "in");
        (24,                         Let,        "let");
        (25,                         Loop,       "loop");
        (26,                         Match,      "match");
        (27,                         Mod,        "mod");
        (28,                         Move,       "move");
        (29,                         Mut,        "mut");
        (30,                         Pub,        "pub");
        (31,                         Ref,        "ref");
        (32,                         Return,     "return");
        // Static and Self are also special idents (prefill de-dupes)
        (super::STATIC_KEYWORD_NAME_NUM, Static, "static");
        (super::SELF_KEYWORD_NAME_NUM,   Self,   "self");
        (33,                         Struct,     "struct");
        (super::SUPER_KEYWORD_NAME_NUM, Super,   "super");
        (34,                         True,       "true");
        (35,                         Trait,      "trait");
        (36,                         Type,       "type");
        (37,                         Unsafe,     "unsafe");
        (38,                         Use,        "use");
        (39,                         Virtual,    "virtual");
        (40,                         While,      "while");
        (41,                         Continue,   "continue");
        (42,                         Proc,       "proc");
        (43,                         Box,        "box");
        (44,                         Const,      "const");
        (45,                         Where,      "where");

        'reserved:
        (46,                         Alignof,    "alignof");
        (47,                         Be,         "be");
        (48,                         Offsetof,   "offsetof");
        (49,                         Priv,       "priv");
        (50,                         Pure,       "pure");
        (51,                         Sizeof,     "sizeof");
        (52,                         Typeof,     "typeof");
        (53,                         Unsized,    "unsized");
        (54,                         Yield,      "yield");
        (55,                         Do,         "do");
        (56,                         Abstract,   "abstract");
        (57,                         Final,      "final");
        (58,                         Override,   "override");
    }
}

// looks like we can get rid of this completely...
pub type IdentInterner = StrInterner;

// if an interner exists in TLS, return it. Otherwise, prepare a
// fresh one.
// FIXME(eddyb) #8726 This should probably use a task-local reference.
pub fn get_ident_interner() -> Rc<IdentInterner> {
    local_data_key!(key: Rc<::parse::token::IdentInterner>)
    match key.get() {
        Some(interner) => interner.clone(),
        None => {
            let interner = Rc::new(mk_fresh_ident_interner());
            key.replace(Some(interner.clone()));
            interner
        }
    }
}

/// Represents a string stored in the task-local interner. Because the
/// interner lives for the life of the task, this can be safely treated as an
/// immortal string, as long as it never crosses between tasks.
///
/// FIXME(pcwalton): You must be careful about what you do in the destructors
/// of objects stored in TLS, because they may run after the interner is
/// destroyed. In particular, they must not access string contents. This can
/// be fixed in the future by just leaking all strings until task death
/// somehow.
#[deriving(Clone, PartialEq, Hash, PartialOrd, Eq, Ord)]
pub struct InternedString {
    string: RcStr,
}

impl InternedString {
    #[inline]
    pub fn new(string: &'static str) -> InternedString {
        InternedString {
            string: RcStr::new(string),
        }
    }

    #[inline]
    fn new_from_rc_str(string: RcStr) -> InternedString {
        InternedString {
            string: string,
        }
    }

    #[inline]
    pub fn get<'a>(&'a self) -> &'a str {
        self.string.as_slice()
    }
}

impl BytesContainer for InternedString {
    fn container_as_bytes<'a>(&'a self) -> &'a [u8] {
        // FIXME #12938: This is a workaround for the incorrect signature
        // of `BytesContainer`, which is itself a workaround for the lack of
        // DST.
        unsafe {
            let this = self.get();
            mem::transmute::<&[u8],&[u8]>(this.container_as_bytes())
        }
    }
}

impl fmt::Show for InternedString {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.string.as_slice())
    }
}

impl<'a> Equiv<&'a str> for InternedString {
    fn equiv(&self, other: & &'a str) -> bool {
        (*other) == self.string.as_slice()
    }
}

impl<D:Decoder<E>, E> Decodable<D, E> for InternedString {
    fn decode(d: &mut D) -> Result<InternedString, E> {
        Ok(get_name(get_ident_interner().intern(
                    try!(d.read_str()).as_slice())))
    }
}

impl<S:Encoder<E>, E> Encodable<S, E> for InternedString {
    fn encode(&self, s: &mut S) -> Result<(), E> {
        s.emit_str(self.string.as_slice())
    }
}

/// Returns the string contents of a name, using the task-local interner.
#[inline]
pub fn get_name(name: ast::Name) -> InternedString {
    let interner = get_ident_interner();
    InternedString::new_from_rc_str(interner.get(name))
}

/// Returns the string contents of an identifier, using the task-local
/// interner.
#[inline]
pub fn get_ident(ident: ast::Ident) -> InternedString {
    get_name(ident.name)
}

/// Interns and returns the string contents of an identifier, using the
/// task-local interner.
#[inline]
pub fn intern_and_get_ident(s: &str) -> InternedString {
    get_name(intern(s))
}

/// Maps a string to its interned representation.
#[inline]
pub fn intern(s: &str) -> ast::Name {
    get_ident_interner().intern(s)
}

/// gensym's a new uint, using the current interner.
#[inline]
pub fn gensym(s: &str) -> ast::Name {
    get_ident_interner().gensym(s)
}

/// Maps a string to an identifier with an empty syntax context.
#[inline]
pub fn str_to_ident(s: &str) -> ast::Ident {
    ast::Ident::new(intern(s))
}

/// Maps a string to a gensym'ed identifier.
#[inline]
pub fn gensym_ident(s: &str) -> ast::Ident {
    ast::Ident::new(gensym(s))
}

// create a fresh name that maps to the same string as the old one.
// note that this guarantees that str_ptr_eq(ident_to_string(src),interner_get(fresh_name(src)));
// that is, that the new name and the old one are connected to ptr_eq strings.
pub fn fresh_name(src: &ast::Ident) -> ast::Name {
    let interner = get_ident_interner();
    interner.gensym_copy(src.name)
    // following: debug version. Could work in final except that it's incompatible with
    // good error messages and uses of struct names in ambiguous could-be-binding
    // locations. Also definitely destroys the guarantee given above about ptr_eq.
    /*let num = rand::task_rng().gen_uint_range(0,0xffff);
    gensym(format!("{}_{}",ident_to_string(src),num))*/
}

// create a fresh mark.
pub fn fresh_mark() -> ast::Mrk {
    gensym("mark").uint() as u32
}

#[cfg(test)]
mod test {
    use super::*;
    use ast;
    use ext::mtwt;

    fn mark_ident(id : ast::Ident, m : ast::Mrk) -> ast::Ident {
        ast::Ident { name: id.name, ctxt:mtwt::apply_mark(m, id.ctxt) }
    }

    #[test] fn mtwt_token_eq_test() {
        assert!(Gt.mtwt_eq(&Gt));
        let a = str_to_ident("bac");
        let a1 = mark_ident(a,92);
        assert!(Ident(a, ModName).mtwt_eq(&Ident(a1, Plain)));
    }
}
