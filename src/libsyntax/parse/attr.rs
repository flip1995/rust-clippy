// Copyright 2012 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use ast;
use codemap::{spanned, Spanned, mk_sp};
use parse::common::*; //resolve bug?
use parse::token;
use parse::parser::Parser;
use parse::token::INTERPOLATED;

use std::vec_ng::Vec;

// a parser that can parse attributes.
pub trait ParserAttr {
    fn parse_outer_attributes(&mut self) -> Vec<ast::Attribute> ;
    fn parse_attribute(&mut self, permit_inner: bool) -> ast::Attribute;
    fn parse_inner_attrs_and_next(&mut self)
                                  -> (Vec<ast::Attribute> , Vec<ast::Attribute> );
    fn parse_meta_item(&mut self) -> @ast::MetaItem;
    fn parse_meta_seq(&mut self) -> Vec<@ast::MetaItem> ;
    fn parse_optional_meta(&mut self) -> Vec<@ast::MetaItem> ;
}

impl ParserAttr for Parser {
    // Parse attributes that appear before an item
    fn parse_outer_attributes(&mut self) -> Vec<ast::Attribute> {
        let mut attrs: Vec<ast::Attribute> = Vec::new();
        loop {
            debug!("parse_outer_attributes: self.token={:?}",
                   self.token);
            match self.token {
              token::INTERPOLATED(token::NtAttr(..)) => {
                attrs.push(self.parse_attribute(false));
              }
              token::POUND => {
                if self.look_ahead(1, |t| *t != token::LBRACKET) {
                    break;
                }
                attrs.push(self.parse_attribute(false));
              }
              token::DOC_COMMENT(s) => {
                let attr = ::attr::mk_sugared_doc_attr(
                    self.id_to_interned_str(s),
                    self.span.lo,
                    self.span.hi
                );
                if attr.node.style != ast::AttrOuter {
                  self.fatal("expected outer comment");
                }
                attrs.push(attr);
                self.bump();
              }
              _ => break
            }
        }
        return attrs;
    }

    // matches attribute = # [ meta_item ]
    //
    // if permit_inner is true, then a trailing `;` indicates an inner
    // attribute
    fn parse_attribute(&mut self, permit_inner: bool) -> ast::Attribute {
        debug!("parse_attributes: permit_inner={:?} self.token={:?}",
               permit_inner, self.token);
        let (span, value) = match self.token {
            INTERPOLATED(token::NtAttr(attr)) => {
                assert!(attr.node.style == ast::AttrOuter);
                self.bump();
                (attr.span, attr.node.value)
            }
            token::POUND => {
                let lo = self.span.lo;
                self.bump();
                self.expect(&token::LBRACKET);
                let meta_item = self.parse_meta_item();
                self.expect(&token::RBRACKET);
                let hi = self.span.hi;
                (mk_sp(lo, hi), meta_item)
            }
            _ => {
                let token_str = self.this_token_to_str();
                self.fatal(format!("expected `\\#` but found `{}`",
                                   token_str));
            }
        };
        let style = if permit_inner && self.token == token::SEMI {
            self.bump();
            ast::AttrInner
        } else {
            ast::AttrOuter
        };
        return Spanned {
            span: span,
            node: ast::Attribute_ {
                style: style,
                value: value,
                is_sugared_doc: false
            }
        };
    }

    // Parse attributes that appear after the opening of an item, each
    // terminated by a semicolon. In addition to a vector of inner attributes,
    // this function also returns a vector that may contain the first outer
    // attribute of the next item (since we can't know whether the attribute
    // is an inner attribute of the containing item or an outer attribute of
    // the first contained item until we see the semi).

    // matches inner_attrs* outer_attr?
    // you can make the 'next' field an Option, but the result is going to be
    // more useful as a vector.
    fn parse_inner_attrs_and_next(&mut self)
                                  -> (Vec<ast::Attribute> , Vec<ast::Attribute> ) {
        let mut inner_attrs: Vec<ast::Attribute> = Vec::new();
        let mut next_outer_attrs: Vec<ast::Attribute> = Vec::new();
        loop {
            let attr = match self.token {
                token::INTERPOLATED(token::NtAttr(..)) => {
                    self.parse_attribute(true)
                }
                token::POUND => {
                    if self.look_ahead(1, |t| *t != token::LBRACKET) {
                        // This is an extension
                        break;
                    }
                    self.parse_attribute(true)
                }
                token::DOC_COMMENT(s) => {
                    self.bump();
                    ::attr::mk_sugared_doc_attr(self.id_to_interned_str(s),
                                                self.span.lo,
                                                self.span.hi)
                }
                _ => {
                    break;
                }
            };
            if attr.node.style == ast::AttrInner {
                inner_attrs.push(attr);
            } else {
                next_outer_attrs.push(attr);
                break;
            }
        }
        (inner_attrs, next_outer_attrs)
    }

    // matches meta_item = IDENT
    // | IDENT = lit
    // | IDENT meta_seq
    fn parse_meta_item(&mut self) -> @ast::MetaItem {
        let lo = self.span.lo;
        let ident = self.parse_ident();
        let name = self.id_to_interned_str(ident);
        match self.token {
            token::EQ => {
                self.bump();
                let lit = self.parse_lit();
                // FIXME #623 Non-string meta items are not serialized correctly;
                // just forbid them for now
                match lit.node {
                    ast::LitStr(..) => {}
                    _ => {
                        self.span_err(
                            lit.span,
                            "non-string literals are not allowed in meta-items");
                    }
                }
                let hi = self.span.hi;
                @spanned(lo, hi, ast::MetaNameValue(name, lit))
            }
            token::LPAREN => {
                let inner_items = self.parse_meta_seq();
                let hi = self.span.hi;
                @spanned(lo, hi, ast::MetaList(name, inner_items))
            }
            _ => {
                let hi = self.last_span.hi;
                @spanned(lo, hi, ast::MetaWord(name))
            }
        }
    }

    // matches meta_seq = ( COMMASEP(meta_item) )
    fn parse_meta_seq(&mut self) -> Vec<@ast::MetaItem> {
        self.parse_seq(&token::LPAREN,
                       &token::RPAREN,
                       seq_sep_trailing_disallowed(token::COMMA),
                       |p| p.parse_meta_item()).node
    }

    fn parse_optional_meta(&mut self) -> Vec<@ast::MetaItem> {
        match self.token {
            token::LPAREN => self.parse_meta_seq(),
            _ => Vec::new()
        }
    }
}
