pub mod attr;
mod expr;
mod item;
mod module;
pub use module::{ModulePath, ModulePathSuccess};
mod pat;
mod path;
mod ty;
pub use path::PathStyle;
mod diagnostics;
mod generics;
mod stmt;
use diagnostics::Error;

use crate::lexer::UnmatchedBrace;
use crate::{Directory, DirectoryOwnership};

use log::debug;
use rustc_ast::ast::DUMMY_NODE_ID;
use rustc_ast::ast::{self, AttrStyle, AttrVec, Const, CrateSugar, Extern, Ident, Unsafe};
use rustc_ast::ast::{
    Async, MacArgs, MacDelimiter, Mutability, StrLit, Visibility, VisibilityKind,
};
use rustc_ast::ptr::P;
use rustc_ast::token::{self, DelimToken, Token, TokenKind};
use rustc_ast::tokenstream::{self, DelimSpan, TokenStream, TokenTree, TreeAndJoint};
use rustc_ast::util::comments::{doc_comment_style, strip_doc_comment_decoration};
use rustc_ast_pretty::pprust;
use rustc_errors::{struct_span_err, Applicability, DiagnosticBuilder, FatalError, PResult};
use rustc_session::parse::ParseSess;
use rustc_span::source_map::respan;
use rustc_span::symbol::{kw, sym, Symbol};
use rustc_span::{FileName, Span, DUMMY_SP};

use std::path::PathBuf;
use std::{cmp, mem, slice};

bitflags::bitflags! {
    struct Restrictions: u8 {
        const STMT_EXPR         = 1 << 0;
        const NO_STRUCT_LITERAL = 1 << 1;
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum SemiColonMode {
    Break,
    Ignore,
    Comma,
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum BlockMode {
    Break,
    Ignore,
}

/// Like `maybe_whole_expr`, but for things other than expressions.
#[macro_export]
macro_rules! maybe_whole {
    ($p:expr, $constructor:ident, |$x:ident| $e:expr) => {
        if let token::Interpolated(nt) = &$p.token.kind {
            if let token::$constructor(x) = &**nt {
                let $x = x.clone();
                $p.bump();
                return Ok($e);
            }
        }
    };
}

/// If the next tokens are ill-formed `$ty::` recover them as `<$ty>::`.
#[macro_export]
macro_rules! maybe_recover_from_interpolated_ty_qpath {
    ($self: expr, $allow_qpath_recovery: expr) => {
        if $allow_qpath_recovery && $self.look_ahead(1, |t| t == &token::ModSep) {
            if let token::Interpolated(nt) = &$self.token.kind {
                if let token::NtTy(ty) = &**nt {
                    let ty = ty.clone();
                    $self.bump();
                    return $self.maybe_recover_from_bad_qpath_stage_2($self.prev_token.span, ty);
                }
            }
        }
    };
}

#[derive(Clone)]
pub struct Parser<'a> {
    pub sess: &'a ParseSess,
    /// The current non-normalized token.
    pub token: Token,
    /// The current normalized token.
    /// "Normalized" means that some interpolated tokens
    /// (`$i: ident` and `$l: lifetime` meta-variables) are replaced
    /// with non-interpolated identifier and lifetime tokens they refer to.
    /// Use this if you need to check for `token::Ident` or `token::Lifetime` specifically,
    /// this also includes edition checks for edition-specific keyword identifiers.
    pub normalized_token: Token,
    /// The previous non-normalized token.
    pub prev_token: Token,
    /// The previous normalized token.
    /// Use this if you need to check for `token::Ident` or `token::Lifetime` specifically,
    /// this also includes edition checks for edition-specific keyword identifiers.
    pub normalized_prev_token: Token,
    restrictions: Restrictions,
    /// Used to determine the path to externally loaded source files.
    pub(super) directory: Directory,
    /// `true` to parse sub-modules in other files.
    // Public for rustfmt usage.
    pub recurse_into_file_modules: bool,
    /// Name of the root module this parser originated from. If `None`, then the
    /// name is not known. This does not change while the parser is descending
    /// into modules, and sub-parsers have new values for this name.
    pub root_module_name: Option<String>,
    expected_tokens: Vec<TokenType>,
    token_cursor: TokenCursor,
    desugar_doc_comments: bool,
    /// `true` we should configure out of line modules as we parse.
    // Public for rustfmt usage.
    pub cfg_mods: bool,
    /// This field is used to keep track of how many left angle brackets we have seen. This is
    /// required in order to detect extra leading left angle brackets (`<` characters) and error
    /// appropriately.
    ///
    /// See the comments in the `parse_path_segment` function for more details.
    unmatched_angle_bracket_count: u32,
    max_angle_bracket_count: u32,
    /// A list of all unclosed delimiters found by the lexer. If an entry is used for error recovery
    /// it gets removed from here. Every entry left at the end gets emitted as an independent
    /// error.
    pub(super) unclosed_delims: Vec<UnmatchedBrace>,
    last_unexpected_token_span: Option<Span>,
    pub last_type_ascription: Option<(Span, bool /* likely path typo */)>,
    /// If present, this `Parser` is not parsing Rust code but rather a macro call.
    subparser_name: Option<&'static str>,
}

impl<'a> Drop for Parser<'a> {
    fn drop(&mut self) {
        emit_unclosed_delims(&mut self.unclosed_delims, &self.sess);
    }
}

#[derive(Clone)]
struct TokenCursor {
    frame: TokenCursorFrame,
    stack: Vec<TokenCursorFrame>,
}

#[derive(Clone)]
struct TokenCursorFrame {
    delim: token::DelimToken,
    span: DelimSpan,
    open_delim: bool,
    tree_cursor: tokenstream::Cursor,
    close_delim: bool,
    last_token: LastToken,
}

/// This is used in `TokenCursorFrame` above to track tokens that are consumed
/// by the parser, and then that's transitively used to record the tokens that
/// each parse AST item is created with.
///
/// Right now this has two states, either collecting tokens or not collecting
/// tokens. If we're collecting tokens we just save everything off into a local
/// `Vec`. This should eventually though likely save tokens from the original
/// token stream and just use slicing of token streams to avoid creation of a
/// whole new vector.
///
/// The second state is where we're passively not recording tokens, but the last
/// token is still tracked for when we want to start recording tokens. This
/// "last token" means that when we start recording tokens we'll want to ensure
/// that this, the first token, is included in the output.
///
/// You can find some more example usage of this in the `collect_tokens` method
/// on the parser.
#[derive(Clone)]
enum LastToken {
    Collecting(Vec<TreeAndJoint>),
    Was(Option<TreeAndJoint>),
}

impl TokenCursorFrame {
    fn new(span: DelimSpan, delim: DelimToken, tts: &TokenStream) -> Self {
        TokenCursorFrame {
            delim,
            span,
            open_delim: delim == token::NoDelim,
            tree_cursor: tts.clone().into_trees(),
            close_delim: delim == token::NoDelim,
            last_token: LastToken::Was(None),
        }
    }
}

impl TokenCursor {
    fn next(&mut self) -> Token {
        loop {
            let tree = if !self.frame.open_delim {
                self.frame.open_delim = true;
                TokenTree::open_tt(self.frame.span, self.frame.delim)
            } else if let Some(tree) = self.frame.tree_cursor.next() {
                tree
            } else if !self.frame.close_delim {
                self.frame.close_delim = true;
                TokenTree::close_tt(self.frame.span, self.frame.delim)
            } else if let Some(frame) = self.stack.pop() {
                self.frame = frame;
                continue;
            } else {
                return Token::new(token::Eof, DUMMY_SP);
            };

            match self.frame.last_token {
                LastToken::Collecting(ref mut v) => v.push(tree.clone().into()),
                LastToken::Was(ref mut t) => *t = Some(tree.clone().into()),
            }

            match tree {
                TokenTree::Token(token) => return token,
                TokenTree::Delimited(sp, delim, tts) => {
                    let frame = TokenCursorFrame::new(sp, delim, &tts);
                    self.stack.push(mem::replace(&mut self.frame, frame));
                }
            }
        }
    }

    fn next_desugared(&mut self) -> Token {
        let (name, sp) = match self.next() {
            Token { kind: token::DocComment(name), span } => (name, span),
            tok => return tok,
        };

        let stripped = strip_doc_comment_decoration(&name.as_str());

        // Searches for the occurrences of `"#*` and returns the minimum number of `#`s
        // required to wrap the text.
        let mut num_of_hashes = 0;
        let mut count = 0;
        for ch in stripped.chars() {
            count = match ch {
                '"' => 1,
                '#' if count > 0 => count + 1,
                _ => 0,
            };
            num_of_hashes = cmp::max(num_of_hashes, count);
        }

        let delim_span = DelimSpan::from_single(sp);
        let body = TokenTree::Delimited(
            delim_span,
            token::Bracket,
            [
                TokenTree::token(token::Ident(sym::doc, false), sp),
                TokenTree::token(token::Eq, sp),
                TokenTree::token(
                    TokenKind::lit(token::StrRaw(num_of_hashes), Symbol::intern(&stripped), None),
                    sp,
                ),
            ]
            .iter()
            .cloned()
            .collect::<TokenStream>(),
        );

        self.stack.push(mem::replace(
            &mut self.frame,
            TokenCursorFrame::new(
                delim_span,
                token::NoDelim,
                &if doc_comment_style(&name.as_str()) == AttrStyle::Inner {
                    [TokenTree::token(token::Pound, sp), TokenTree::token(token::Not, sp), body]
                        .iter()
                        .cloned()
                        .collect::<TokenStream>()
                } else {
                    [TokenTree::token(token::Pound, sp), body]
                        .iter()
                        .cloned()
                        .collect::<TokenStream>()
                },
            ),
        ));

        self.next()
    }
}

#[derive(Clone, PartialEq)]
enum TokenType {
    Token(TokenKind),
    Keyword(Symbol),
    Operator,
    Lifetime,
    Ident,
    Path,
    Type,
    Const,
}

impl TokenType {
    fn to_string(&self) -> String {
        match *self {
            TokenType::Token(ref t) => format!("`{}`", pprust::token_kind_to_string(t)),
            TokenType::Keyword(kw) => format!("`{}`", kw),
            TokenType::Operator => "an operator".to_string(),
            TokenType::Lifetime => "lifetime".to_string(),
            TokenType::Ident => "identifier".to_string(),
            TokenType::Path => "path".to_string(),
            TokenType::Type => "type".to_string(),
            TokenType::Const => "const".to_string(),
        }
    }
}

#[derive(Copy, Clone, Debug)]
enum TokenExpectType {
    Expect,
    NoExpect,
}

/// A sequence separator.
struct SeqSep {
    /// The separator token.
    sep: Option<TokenKind>,
    /// `true` if a trailing separator is allowed.
    trailing_sep_allowed: bool,
}

impl SeqSep {
    fn trailing_allowed(t: TokenKind) -> SeqSep {
        SeqSep { sep: Some(t), trailing_sep_allowed: true }
    }

    fn none() -> SeqSep {
        SeqSep { sep: None, trailing_sep_allowed: false }
    }
}

pub enum FollowedByType {
    Yes,
    No,
}

fn token_descr_opt(token: &Token) -> Option<&'static str> {
    Some(match token.kind {
        _ if token.is_special_ident() => "reserved identifier",
        _ if token.is_used_keyword() => "keyword",
        _ if token.is_unused_keyword() => "reserved keyword",
        token::DocComment(..) => "doc comment",
        _ => return None,
    })
}

pub(super) fn token_descr(token: &Token) -> String {
    let token_str = pprust::token_to_string(token);
    match token_descr_opt(token) {
        Some(prefix) => format!("{} `{}`", prefix, token_str),
        _ => format!("`{}`", token_str),
    }
}

impl<'a> Parser<'a> {
    pub fn new(
        sess: &'a ParseSess,
        tokens: TokenStream,
        directory: Option<Directory>,
        recurse_into_file_modules: bool,
        desugar_doc_comments: bool,
        subparser_name: Option<&'static str>,
    ) -> Self {
        let mut parser = Parser {
            sess,
            token: Token::dummy(),
            normalized_token: Token::dummy(),
            prev_token: Token::dummy(),
            normalized_prev_token: Token::dummy(),
            restrictions: Restrictions::empty(),
            recurse_into_file_modules,
            directory: Directory {
                path: PathBuf::new(),
                ownership: DirectoryOwnership::Owned { relative: None },
            },
            root_module_name: None,
            expected_tokens: Vec::new(),
            token_cursor: TokenCursor {
                frame: TokenCursorFrame::new(DelimSpan::dummy(), token::NoDelim, &tokens),
                stack: Vec::new(),
            },
            desugar_doc_comments,
            cfg_mods: true,
            unmatched_angle_bracket_count: 0,
            max_angle_bracket_count: 0,
            unclosed_delims: Vec::new(),
            last_unexpected_token_span: None,
            last_type_ascription: None,
            subparser_name,
        };

        // Make parser point to the first token.
        parser.bump();

        if let Some(directory) = directory {
            parser.directory = directory;
        } else if !parser.token.span.is_dummy() {
            if let Some(FileName::Real(path)) =
                &sess.source_map().lookup_char_pos(parser.token.span.lo()).file.unmapped_path
            {
                if let Some(directory_path) = path.parent() {
                    parser.directory.path = directory_path.to_path_buf();
                }
            }
        }

        parser
    }

    fn next_tok(&mut self, fallback_span: Span) -> Token {
        let mut next = if self.desugar_doc_comments {
            self.token_cursor.next_desugared()
        } else {
            self.token_cursor.next()
        };
        if next.span.is_dummy() {
            // Tweak the location for better diagnostics, but keep syntactic context intact.
            next.span = fallback_span.with_ctxt(next.span.ctxt());
        }
        next
    }

    crate fn unexpected<T>(&mut self) -> PResult<'a, T> {
        match self.expect_one_of(&[], &[]) {
            Err(e) => Err(e),
            // We can get `Ok(true)` from `recover_closing_delimiter`
            // which is called in `expected_one_of_not_found`.
            Ok(_) => FatalError.raise(),
        }
    }

    /// Expects and consumes the token `t`. Signals an error if the next token is not `t`.
    pub fn expect(&mut self, t: &TokenKind) -> PResult<'a, bool /* recovered */> {
        if self.expected_tokens.is_empty() {
            if self.token == *t {
                self.bump();
                Ok(false)
            } else {
                self.unexpected_try_recover(t)
            }
        } else {
            self.expect_one_of(slice::from_ref(t), &[])
        }
    }

    /// Expect next token to be edible or inedible token.  If edible,
    /// then consume it; if inedible, then return without consuming
    /// anything.  Signal a fatal error if next token is unexpected.
    pub fn expect_one_of(
        &mut self,
        edible: &[TokenKind],
        inedible: &[TokenKind],
    ) -> PResult<'a, bool /* recovered */> {
        if edible.contains(&self.token.kind) {
            self.bump();
            Ok(false)
        } else if inedible.contains(&self.token.kind) {
            // leave it in the input
            Ok(false)
        } else if self.last_unexpected_token_span == Some(self.token.span) {
            FatalError.raise();
        } else {
            self.expected_one_of_not_found(edible, inedible)
        }
    }

    // Public for rustfmt usage.
    pub fn parse_ident(&mut self) -> PResult<'a, ast::Ident> {
        self.parse_ident_common(true)
    }

    fn parse_ident_common(&mut self, recover: bool) -> PResult<'a, ast::Ident> {
        match self.normalized_token.kind {
            token::Ident(name, _) => {
                if self.token.is_reserved_ident() {
                    let mut err = self.expected_ident_found();
                    if recover {
                        err.emit();
                    } else {
                        return Err(err);
                    }
                }
                self.bump();
                Ok(Ident::new(name, self.normalized_prev_token.span))
            }
            _ => Err(match self.prev_token.kind {
                TokenKind::DocComment(..) => {
                    self.span_fatal_err(self.prev_token.span, Error::UselessDocComment)
                }
                _ => self.expected_ident_found(),
            }),
        }
    }

    /// Checks if the next token is `tok`, and returns `true` if so.
    ///
    /// This method will automatically add `tok` to `expected_tokens` if `tok` is not
    /// encountered.
    fn check(&mut self, tok: &TokenKind) -> bool {
        let is_present = self.token == *tok;
        if !is_present {
            self.expected_tokens.push(TokenType::Token(tok.clone()));
        }
        is_present
    }

    /// Consumes a token 'tok' if it exists. Returns whether the given token was present.
    pub fn eat(&mut self, tok: &TokenKind) -> bool {
        let is_present = self.check(tok);
        if is_present {
            self.bump()
        }
        is_present
    }

    /// If the next token is the given keyword, returns `true` without eating it.
    /// An expectation is also added for diagnostics purposes.
    fn check_keyword(&mut self, kw: Symbol) -> bool {
        self.expected_tokens.push(TokenType::Keyword(kw));
        self.token.is_keyword(kw)
    }

    /// If the next token is the given keyword, eats it and returns `true`.
    /// Otherwise, returns `false`. An expectation is also added for diagnostics purposes.
    // Public for rustfmt usage.
    pub fn eat_keyword(&mut self, kw: Symbol) -> bool {
        if self.check_keyword(kw) {
            self.bump();
            true
        } else {
            false
        }
    }

    fn eat_keyword_noexpect(&mut self, kw: Symbol) -> bool {
        if self.token.is_keyword(kw) {
            self.bump();
            true
        } else {
            false
        }
    }

    /// If the given word is not a keyword, signals an error.
    /// If the next token is not the given word, signals an error.
    /// Otherwise, eats it.
    fn expect_keyword(&mut self, kw: Symbol) -> PResult<'a, ()> {
        if !self.eat_keyword(kw) { self.unexpected() } else { Ok(()) }
    }

    /// Is the given keyword `kw` followed by a non-reserved identifier?
    fn is_kw_followed_by_ident(&self, kw: Symbol) -> bool {
        self.token.is_keyword(kw) && self.look_ahead(1, |t| t.is_ident() && !t.is_reserved_ident())
    }

    fn check_or_expected(&mut self, ok: bool, typ: TokenType) -> bool {
        if ok {
            true
        } else {
            self.expected_tokens.push(typ);
            false
        }
    }

    fn check_ident(&mut self) -> bool {
        self.check_or_expected(self.token.is_ident(), TokenType::Ident)
    }

    fn check_path(&mut self) -> bool {
        self.check_or_expected(self.token.is_path_start(), TokenType::Path)
    }

    fn check_type(&mut self) -> bool {
        self.check_or_expected(self.token.can_begin_type(), TokenType::Type)
    }

    fn check_const_arg(&mut self) -> bool {
        self.check_or_expected(self.token.can_begin_const_arg(), TokenType::Const)
    }

    /// Checks to see if the next token is either `+` or `+=`.
    /// Otherwise returns `false`.
    fn check_plus(&mut self) -> bool {
        self.check_or_expected(
            self.token.is_like_plus(),
            TokenType::Token(token::BinOp(token::Plus)),
        )
    }

    /// Eats the expected token if it's present possibly breaking
    /// compound tokens like multi-character operators in process.
    /// Returns `true` if the token was eaten.
    fn break_and_eat(&mut self, expected: TokenKind) -> bool {
        if self.token.kind == expected {
            self.bump();
            return true;
        }
        match self.token.kind.break_two_token_op() {
            Some((first, second)) if first == expected => {
                let first_span = self.sess.source_map().start_point(self.token.span);
                let second_span = self.token.span.with_lo(first_span.hi());
                self.set_token(Token::new(first, first_span));
                self.bump_with(Token::new(second, second_span));
                true
            }
            _ => {
                self.expected_tokens.push(TokenType::Token(expected));
                false
            }
        }
    }

    /// Eats `+` possibly breaking tokens like `+=` in process.
    fn eat_plus(&mut self) -> bool {
        self.break_and_eat(token::BinOp(token::Plus))
    }

    /// Eats `&` possibly breaking tokens like `&&` in process.
    /// Signals an error if `&` is not eaten.
    fn expect_and(&mut self) -> PResult<'a, ()> {
        if self.break_and_eat(token::BinOp(token::And)) { Ok(()) } else { self.unexpected() }
    }

    /// Eats `|` possibly breaking tokens like `||` in process.
    /// Signals an error if `|` was not eaten.
    fn expect_or(&mut self) -> PResult<'a, ()> {
        if self.break_and_eat(token::BinOp(token::Or)) { Ok(()) } else { self.unexpected() }
    }

    /// Eats `<` possibly breaking tokens like `<<` in process.
    fn eat_lt(&mut self) -> bool {
        let ate = self.break_and_eat(token::Lt);
        if ate {
            // See doc comment for `unmatched_angle_bracket_count`.
            self.unmatched_angle_bracket_count += 1;
            self.max_angle_bracket_count += 1;
            debug!("eat_lt: (increment) count={:?}", self.unmatched_angle_bracket_count);
        }
        ate
    }

    /// Eats `<` possibly breaking tokens like `<<` in process.
    /// Signals an error if `<` was not eaten.
    fn expect_lt(&mut self) -> PResult<'a, ()> {
        if self.eat_lt() { Ok(()) } else { self.unexpected() }
    }

    /// Eats `>` possibly breaking tokens like `>>` in process.
    /// Signals an error if `>` was not eaten.
    fn expect_gt(&mut self) -> PResult<'a, ()> {
        if self.break_and_eat(token::Gt) {
            // See doc comment for `unmatched_angle_bracket_count`.
            if self.unmatched_angle_bracket_count > 0 {
                self.unmatched_angle_bracket_count -= 1;
                debug!("expect_gt: (decrement) count={:?}", self.unmatched_angle_bracket_count);
            }
            Ok(())
        } else {
            self.unexpected()
        }
    }

    fn expect_any_with_type(&mut self, kets: &[&TokenKind], expect: TokenExpectType) -> bool {
        kets.iter().any(|k| match expect {
            TokenExpectType::Expect => self.check(k),
            TokenExpectType::NoExpect => self.token == **k,
        })
    }

    fn parse_seq_to_before_tokens<T>(
        &mut self,
        kets: &[&TokenKind],
        sep: SeqSep,
        expect: TokenExpectType,
        mut f: impl FnMut(&mut Parser<'a>) -> PResult<'a, T>,
    ) -> PResult<'a, (Vec<T>, bool /* trailing */, bool /* recovered */)> {
        let mut first = true;
        let mut recovered = false;
        let mut trailing = false;
        let mut v = vec![];
        while !self.expect_any_with_type(kets, expect) {
            if let token::CloseDelim(..) | token::Eof = self.token.kind {
                break;
            }
            if let Some(ref t) = sep.sep {
                if first {
                    first = false;
                } else {
                    match self.expect(t) {
                        Ok(false) => {}
                        Ok(true) => {
                            recovered = true;
                            break;
                        }
                        Err(mut expect_err) => {
                            let sp = self.prev_token.span.shrink_to_hi();
                            let token_str = pprust::token_kind_to_string(t);

                            // Attempt to keep parsing if it was a similar separator.
                            if let Some(ref tokens) = t.similar_tokens() {
                                if tokens.contains(&self.token.kind) {
                                    self.bump();
                                }
                            }

                            // Attempt to keep parsing if it was an omitted separator.
                            match f(self) {
                                Ok(t) => {
                                    // Parsed successfully, therefore most probably the code only
                                    // misses a separator.
                                    expect_err
                                        .span_suggestion_short(
                                            sp,
                                            &format!("missing `{}`", token_str),
                                            token_str,
                                            Applicability::MaybeIncorrect,
                                        )
                                        .emit();

                                    v.push(t);
                                    continue;
                                }
                                Err(mut e) => {
                                    // Parsing failed, therefore it must be something more serious
                                    // than just a missing separator.
                                    expect_err.emit();

                                    e.cancel();
                                    break;
                                }
                            }
                        }
                    }
                }
            }
            if sep.trailing_sep_allowed && self.expect_any_with_type(kets, expect) {
                trailing = true;
                break;
            }

            let t = f(self)?;
            v.push(t);
        }

        Ok((v, trailing, recovered))
    }

    /// Parses a sequence, not including the closing delimiter. The function
    /// `f` must consume tokens until reaching the next separator or
    /// closing bracket.
    fn parse_seq_to_before_end<T>(
        &mut self,
        ket: &TokenKind,
        sep: SeqSep,
        f: impl FnMut(&mut Parser<'a>) -> PResult<'a, T>,
    ) -> PResult<'a, (Vec<T>, bool, bool)> {
        self.parse_seq_to_before_tokens(&[ket], sep, TokenExpectType::Expect, f)
    }

    /// Parses a sequence, including the closing delimiter. The function
    /// `f` must consume tokens until reaching the next separator or
    /// closing bracket.
    fn parse_seq_to_end<T>(
        &mut self,
        ket: &TokenKind,
        sep: SeqSep,
        f: impl FnMut(&mut Parser<'a>) -> PResult<'a, T>,
    ) -> PResult<'a, (Vec<T>, bool /* trailing */)> {
        let (val, trailing, recovered) = self.parse_seq_to_before_end(ket, sep, f)?;
        if !recovered {
            self.eat(ket);
        }
        Ok((val, trailing))
    }

    /// Parses a sequence, including the closing delimiter. The function
    /// `f` must consume tokens until reaching the next separator or
    /// closing bracket.
    fn parse_unspanned_seq<T>(
        &mut self,
        bra: &TokenKind,
        ket: &TokenKind,
        sep: SeqSep,
        f: impl FnMut(&mut Parser<'a>) -> PResult<'a, T>,
    ) -> PResult<'a, (Vec<T>, bool)> {
        self.expect(bra)?;
        self.parse_seq_to_end(ket, sep, f)
    }

    fn parse_delim_comma_seq<T>(
        &mut self,
        delim: DelimToken,
        f: impl FnMut(&mut Parser<'a>) -> PResult<'a, T>,
    ) -> PResult<'a, (Vec<T>, bool)> {
        self.parse_unspanned_seq(
            &token::OpenDelim(delim),
            &token::CloseDelim(delim),
            SeqSep::trailing_allowed(token::Comma),
            f,
        )
    }

    fn parse_paren_comma_seq<T>(
        &mut self,
        f: impl FnMut(&mut Parser<'a>) -> PResult<'a, T>,
    ) -> PResult<'a, (Vec<T>, bool)> {
        self.parse_delim_comma_seq(token::Paren, f)
    }

    // Interpolated identifier (`$i: ident`) and lifetime (`$l: lifetime`)
    // tokens are replaced with usual identifier and lifetime tokens,
    // so the former are never encountered during normal parsing.
    crate fn set_token(&mut self, token: Token) {
        self.token = token;
        self.normalized_token = match &self.token.kind {
            token::Interpolated(nt) => match **nt {
                token::NtIdent(ident, is_raw) => {
                    Token::new(token::Ident(ident.name, is_raw), ident.span)
                }
                token::NtLifetime(ident) => Token::new(token::Lifetime(ident.name), ident.span),
                _ => self.token.clone(),
            },
            _ => self.token.clone(),
        }
    }

    /// Advance the parser by one token using provided token as the next one.
    fn bump_with(&mut self, next_token: Token) {
        // Bumping after EOF is a bad sign, usually an infinite loop.
        if self.prev_token.kind == TokenKind::Eof {
            let msg = "attempted to bump the parser past EOF (may be stuck in a loop)";
            self.span_bug(self.token.span, msg);
        }

        // Update the current and previous tokens.
        self.prev_token = self.token.take();
        self.normalized_prev_token = self.normalized_token.take();
        self.set_token(next_token);

        // Diagnostics.
        self.expected_tokens.clear();
    }

    /// Advance the parser by one token.
    pub fn bump(&mut self) {
        let next_token = self.next_tok(self.token.span);
        self.bump_with(next_token);
    }

    /// Look-ahead `dist` tokens of `self.token` and get access to that token there.
    /// When `dist == 0` then the current token is looked at.
    pub fn look_ahead<R>(&self, dist: usize, looker: impl FnOnce(&Token) -> R) -> R {
        if dist == 0 {
            return looker(&self.token);
        }

        let frame = &self.token_cursor.frame;
        looker(&match frame.tree_cursor.look_ahead(dist - 1) {
            Some(tree) => match tree {
                TokenTree::Token(token) => token,
                TokenTree::Delimited(dspan, delim, _) => {
                    Token::new(token::OpenDelim(delim), dspan.open)
                }
            },
            None => Token::new(token::CloseDelim(frame.delim), frame.span.close),
        })
    }

    /// Returns whether any of the given keywords are `dist` tokens ahead of the current one.
    fn is_keyword_ahead(&self, dist: usize, kws: &[Symbol]) -> bool {
        self.look_ahead(dist, |t| kws.iter().any(|&kw| t.is_keyword(kw)))
    }

    /// Parses asyncness: `async` or nothing.
    fn parse_asyncness(&mut self) -> Async {
        if self.eat_keyword(kw::Async) {
            let span = self.normalized_prev_token.span;
            Async::Yes { span, closure_id: DUMMY_NODE_ID, return_impl_trait_id: DUMMY_NODE_ID }
        } else {
            Async::No
        }
    }

    /// Parses unsafety: `unsafe` or nothing.
    fn parse_unsafety(&mut self) -> Unsafe {
        if self.eat_keyword(kw::Unsafe) {
            Unsafe::Yes(self.normalized_prev_token.span)
        } else {
            Unsafe::No
        }
    }

    /// Parses constness: `const` or nothing.
    fn parse_constness(&mut self) -> Const {
        if self.eat_keyword(kw::Const) {
            Const::Yes(self.normalized_prev_token.span)
        } else {
            Const::No
        }
    }

    /// Parses mutability (`mut` or nothing).
    fn parse_mutability(&mut self) -> Mutability {
        if self.eat_keyword(kw::Mut) { Mutability::Mut } else { Mutability::Not }
    }

    /// Possibly parses mutability (`const` or `mut`).
    fn parse_const_or_mut(&mut self) -> Option<Mutability> {
        if self.eat_keyword(kw::Mut) {
            Some(Mutability::Mut)
        } else if self.eat_keyword(kw::Const) {
            Some(Mutability::Not)
        } else {
            None
        }
    }

    fn parse_field_name(&mut self) -> PResult<'a, Ident> {
        if let token::Literal(token::Lit { kind: token::Integer, symbol, suffix }) = self.token.kind
        {
            self.expect_no_suffix(self.token.span, "a tuple index", suffix);
            self.bump();
            Ok(Ident::new(symbol, self.prev_token.span))
        } else {
            self.parse_ident_common(false)
        }
    }

    fn parse_mac_args(&mut self) -> PResult<'a, P<MacArgs>> {
        self.parse_mac_args_common(true).map(P)
    }

    fn parse_attr_args(&mut self) -> PResult<'a, MacArgs> {
        self.parse_mac_args_common(false)
    }

    fn parse_mac_args_common(&mut self, delimited_only: bool) -> PResult<'a, MacArgs> {
        Ok(
            if self.check(&token::OpenDelim(DelimToken::Paren))
                || self.check(&token::OpenDelim(DelimToken::Bracket))
                || self.check(&token::OpenDelim(DelimToken::Brace))
            {
                match self.parse_token_tree() {
                    TokenTree::Delimited(dspan, delim, tokens) =>
                    // We've confirmed above that there is a delimiter so unwrapping is OK.
                    {
                        MacArgs::Delimited(dspan, MacDelimiter::from_token(delim).unwrap(), tokens)
                    }
                    _ => unreachable!(),
                }
            } else if !delimited_only {
                if self.eat(&token::Eq) {
                    let eq_span = self.prev_token.span;
                    let mut is_interpolated_expr = false;
                    if let token::Interpolated(nt) = &self.token.kind {
                        if let token::NtExpr(..) = **nt {
                            is_interpolated_expr = true;
                        }
                    }
                    let token_tree = if is_interpolated_expr {
                        // We need to accept arbitrary interpolated expressions to continue
                        // supporting things like `doc = $expr` that work on stable.
                        // Non-literal interpolated expressions are rejected after expansion.
                        self.parse_token_tree()
                    } else {
                        self.parse_unsuffixed_lit()?.token_tree()
                    };

                    MacArgs::Eq(eq_span, token_tree.into())
                } else {
                    MacArgs::Empty
                }
            } else {
                return self.unexpected();
            },
        )
    }

    fn parse_or_use_outer_attributes(
        &mut self,
        already_parsed_attrs: Option<AttrVec>,
    ) -> PResult<'a, AttrVec> {
        if let Some(attrs) = already_parsed_attrs {
            Ok(attrs)
        } else {
            self.parse_outer_attributes().map(|a| a.into())
        }
    }

    /// Parses a single token tree from the input.
    pub fn parse_token_tree(&mut self) -> TokenTree {
        match self.token.kind {
            token::OpenDelim(..) => {
                let frame = mem::replace(
                    &mut self.token_cursor.frame,
                    self.token_cursor.stack.pop().unwrap(),
                );
                self.set_token(Token::new(TokenKind::CloseDelim(frame.delim), frame.span.close));
                self.bump();
                TokenTree::Delimited(frame.span, frame.delim, frame.tree_cursor.stream)
            }
            token::CloseDelim(_) | token::Eof => unreachable!(),
            _ => {
                self.bump();
                TokenTree::Token(self.prev_token.clone())
            }
        }
    }

    /// Parses a stream of tokens into a list of `TokenTree`s, up to EOF.
    pub fn parse_all_token_trees(&mut self) -> PResult<'a, Vec<TokenTree>> {
        let mut tts = Vec::new();
        while self.token != token::Eof {
            tts.push(self.parse_token_tree());
        }
        Ok(tts)
    }

    pub fn parse_tokens(&mut self) -> TokenStream {
        let mut result = Vec::new();
        loop {
            match self.token.kind {
                token::Eof | token::CloseDelim(..) => break,
                _ => result.push(self.parse_token_tree().into()),
            }
        }
        TokenStream::new(result)
    }

    /// Evaluates the closure with restrictions in place.
    ///
    /// Afters the closure is evaluated, restrictions are reset.
    fn with_res<T>(&mut self, res: Restrictions, f: impl FnOnce(&mut Self) -> T) -> T {
        let old = self.restrictions;
        self.restrictions = res;
        let res = f(self);
        self.restrictions = old;
        res
    }

    fn is_crate_vis(&self) -> bool {
        self.token.is_keyword(kw::Crate) && self.look_ahead(1, |t| t != &token::ModSep)
    }

    /// Parses `pub`, `pub(crate)` and `pub(in path)` plus shortcuts `crate` for `pub(crate)`,
    /// `pub(self)` for `pub(in self)` and `pub(super)` for `pub(in super)`.
    /// If the following element can't be a tuple (i.e., it's a function definition), then
    /// it's not a tuple struct field), and the contents within the parentheses isn't valid,
    /// so emit a proper diagnostic.
    pub fn parse_visibility(&mut self, fbt: FollowedByType) -> PResult<'a, Visibility> {
        maybe_whole!(self, NtVis, |x| x);

        self.expected_tokens.push(TokenType::Keyword(kw::Crate));
        if self.is_crate_vis() {
            self.bump(); // `crate`
            self.sess.gated_spans.gate(sym::crate_visibility_modifier, self.prev_token.span);
            return Ok(respan(self.prev_token.span, VisibilityKind::Crate(CrateSugar::JustCrate)));
        }

        if !self.eat_keyword(kw::Pub) {
            // We need a span for our `Spanned<VisibilityKind>`, but there's inherently no
            // keyword to grab a span from for inherited visibility; an empty span at the
            // beginning of the current token would seem to be the "Schelling span".
            return Ok(respan(self.token.span.shrink_to_lo(), VisibilityKind::Inherited));
        }
        let lo = self.prev_token.span;

        if self.check(&token::OpenDelim(token::Paren)) {
            // We don't `self.bump()` the `(` yet because this might be a struct definition where
            // `()` or a tuple might be allowed. For example, `struct Struct(pub (), pub (usize));`.
            // Because of this, we only `bump` the `(` if we're assured it is appropriate to do so
            // by the following tokens.
            if self.is_keyword_ahead(1, &[kw::Crate]) && self.look_ahead(2, |t| t != &token::ModSep)
            // account for `pub(crate::foo)`
            {
                // Parse `pub(crate)`.
                self.bump(); // `(`
                self.bump(); // `crate`
                self.expect(&token::CloseDelim(token::Paren))?; // `)`
                let vis = VisibilityKind::Crate(CrateSugar::PubCrate);
                return Ok(respan(lo.to(self.prev_token.span), vis));
            } else if self.is_keyword_ahead(1, &[kw::In]) {
                // Parse `pub(in path)`.
                self.bump(); // `(`
                self.bump(); // `in`
                let path = self.parse_path(PathStyle::Mod)?; // `path`
                self.expect(&token::CloseDelim(token::Paren))?; // `)`
                let vis = VisibilityKind::Restricted { path: P(path), id: ast::DUMMY_NODE_ID };
                return Ok(respan(lo.to(self.prev_token.span), vis));
            } else if self.look_ahead(2, |t| t == &token::CloseDelim(token::Paren))
                && self.is_keyword_ahead(1, &[kw::Super, kw::SelfLower])
            {
                // Parse `pub(self)` or `pub(super)`.
                self.bump(); // `(`
                let path = self.parse_path(PathStyle::Mod)?; // `super`/`self`
                self.expect(&token::CloseDelim(token::Paren))?; // `)`
                let vis = VisibilityKind::Restricted { path: P(path), id: ast::DUMMY_NODE_ID };
                return Ok(respan(lo.to(self.prev_token.span), vis));
            } else if let FollowedByType::No = fbt {
                // Provide this diagnostic if a type cannot follow;
                // in particular, if this is not a tuple struct.
                self.recover_incorrect_vis_restriction()?;
                // Emit diagnostic, but continue with public visibility.
            }
        }

        Ok(respan(lo, VisibilityKind::Public))
    }

    /// Recovery for e.g. `pub(something) fn ...` or `struct X { pub(something) y: Z }`
    fn recover_incorrect_vis_restriction(&mut self) -> PResult<'a, ()> {
        self.bump(); // `(`
        let path = self.parse_path(PathStyle::Mod)?;
        self.expect(&token::CloseDelim(token::Paren))?; // `)`

        let msg = "incorrect visibility restriction";
        let suggestion = r##"some possible visibility restrictions are:
`pub(crate)`: visible only on the current crate
`pub(super)`: visible only in the current module's parent
`pub(in path::to::module)`: visible only on the specified path"##;

        let path_str = pprust::path_to_string(&path);

        struct_span_err!(self.sess.span_diagnostic, path.span, E0704, "{}", msg)
            .help(suggestion)
            .span_suggestion(
                path.span,
                &format!("make this visible only to module `{}` with `in`", path_str),
                format!("in {}", path_str),
                Applicability::MachineApplicable,
            )
            .emit();

        Ok(())
    }

    /// Parses `extern string_literal?`.
    fn parse_extern(&mut self) -> PResult<'a, Extern> {
        Ok(if self.eat_keyword(kw::Extern) {
            Extern::from_abi(self.parse_abi())
        } else {
            Extern::None
        })
    }

    /// Parses a string literal as an ABI spec.
    fn parse_abi(&mut self) -> Option<StrLit> {
        match self.parse_str_lit() {
            Ok(str_lit) => Some(str_lit),
            Err(Some(lit)) => match lit.kind {
                ast::LitKind::Err(_) => None,
                _ => {
                    self.struct_span_err(lit.span, "non-string ABI literal")
                        .span_suggestion(
                            lit.span,
                            "specify the ABI with a string literal",
                            "\"C\"".to_string(),
                            Applicability::MaybeIncorrect,
                        )
                        .emit();
                    None
                }
            },
            Err(None) => None,
        }
    }

    fn collect_tokens<R>(
        &mut self,
        f: impl FnOnce(&mut Self) -> PResult<'a, R>,
    ) -> PResult<'a, (R, TokenStream)> {
        // Record all tokens we parse when parsing this item.
        let mut tokens = Vec::new();
        let prev_collecting = match self.token_cursor.frame.last_token {
            LastToken::Collecting(ref mut list) => Some(mem::take(list)),
            LastToken::Was(ref mut last) => {
                tokens.extend(last.take());
                None
            }
        };
        self.token_cursor.frame.last_token = LastToken::Collecting(tokens);
        let prev = self.token_cursor.stack.len();
        let ret = f(self);
        let last_token = if self.token_cursor.stack.len() == prev {
            &mut self.token_cursor.frame.last_token
        } else if self.token_cursor.stack.get(prev).is_none() {
            // This can happen due to a bad interaction of two unrelated recovery mechanisms with
            // mismatched delimiters *and* recovery lookahead on the likely typo `pub ident(`
            // (#62881).
            return Ok((ret?, TokenStream::default()));
        } else {
            &mut self.token_cursor.stack[prev].last_token
        };

        // Pull out the tokens that we've collected from the call to `f` above.
        let mut collected_tokens = match *last_token {
            LastToken::Collecting(ref mut v) => mem::take(v),
            LastToken::Was(ref was) => {
                let msg = format!("our vector went away? - found Was({:?})", was);
                debug!("collect_tokens: {}", msg);
                self.sess.span_diagnostic.delay_span_bug(self.token.span, &msg);
                // This can happen due to a bad interaction of two unrelated recovery mechanisms
                // with mismatched delimiters *and* recovery lookahead on the likely typo
                // `pub ident(` (#62895, different but similar to the case above).
                return Ok((ret?, TokenStream::default()));
            }
        };

        // If we're not at EOF our current token wasn't actually consumed by
        // `f`, but it'll still be in our list that we pulled out. In that case
        // put it back.
        let extra_token = if self.token != token::Eof { collected_tokens.pop() } else { None };

        // If we were previously collecting tokens, then this was a recursive
        // call. In that case we need to record all the tokens we collected in
        // our parent list as well. To do that we push a clone of our stream
        // onto the previous list.
        match prev_collecting {
            Some(mut list) => {
                list.extend(collected_tokens.iter().cloned());
                list.extend(extra_token);
                *last_token = LastToken::Collecting(list);
            }
            None => {
                *last_token = LastToken::Was(extra_token);
            }
        }

        Ok((ret?, TokenStream::new(collected_tokens)))
    }

    /// `::{` or `::*`
    fn is_import_coupler(&mut self) -> bool {
        self.check(&token::ModSep)
            && self.look_ahead(1, |t| {
                *t == token::OpenDelim(token::Brace) || *t == token::BinOp(token::Star)
            })
    }
}

crate fn make_unclosed_delims_error(
    unmatched: UnmatchedBrace,
    sess: &ParseSess,
) -> Option<DiagnosticBuilder<'_>> {
    // `None` here means an `Eof` was found. We already emit those errors elsewhere, we add them to
    // `unmatched_braces` only for error recovery in the `Parser`.
    let found_delim = unmatched.found_delim?;
    let mut err = sess.span_diagnostic.struct_span_err(
        unmatched.found_span,
        &format!(
            "mismatched closing delimiter: `{}`",
            pprust::token_kind_to_string(&token::CloseDelim(found_delim)),
        ),
    );
    err.span_label(unmatched.found_span, "mismatched closing delimiter");
    if let Some(sp) = unmatched.candidate_span {
        err.span_label(sp, "closing delimiter possibly meant for this");
    }
    if let Some(sp) = unmatched.unclosed_span {
        err.span_label(sp, "unclosed delimiter");
    }
    Some(err)
}

pub fn emit_unclosed_delims(unclosed_delims: &mut Vec<UnmatchedBrace>, sess: &ParseSess) {
    *sess.reached_eof.borrow_mut() |=
        unclosed_delims.iter().any(|unmatched_delim| unmatched_delim.found_delim.is_none());
    for unmatched in unclosed_delims.drain(..) {
        make_unclosed_delims_error(unmatched, sess).map(|mut e| {
            e.emit();
        });
    }
}
