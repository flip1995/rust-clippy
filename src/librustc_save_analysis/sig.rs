// Copyright 2017 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// FIXME? None of these include visibility info.
// Large outstanding things - where clauses, defs/refs for generics
// paresable - each sig ends with `;` of ` {}`

use SaveContext;

use rls_data::{Signature, SigElement, Id};

use rustc::hir::def::Def;
use rustc::hir::def_id::DefId;
use syntax::ast::{self, NodeId};
use syntax::print::pprust;


// TODO dup from json_dumper
fn id_from_def_id(id: DefId) -> Id {
    Id {
        krate: id.krate.as_u32(),
        index: id.index.as_u32(),
    }
}

fn id_from_node_id(id: NodeId, scx: &SaveContext) -> Id {
    let def_id = scx.tcx.hir.local_def_id(id);
    id_from_def_id(def_id)
}

type Result = ::std::result::Result<Signature, &'static str>;

trait Sig {
    fn make(&self, offset: usize, id: Option<NodeId>, scx: &SaveContext) -> Result;
}

fn extend_sig(mut sig: Signature,
              text: String,
              defs: Vec<SigElement>,
              refs: Vec<SigElement>)
              -> Signature {
    sig.text = text;
    sig.defs.extend(defs.into_iter());
    sig.refs.extend(refs.into_iter());
    sig
}

fn replace_text(mut sig: Signature, text: String) -> Signature {
    sig.text = text;
    sig
}

fn merge_sigs(text: String, sigs: Vec<Signature>) -> Signature {
    let mut result = Signature {
        text,
        defs: vec![],
        refs: vec![],
    };

    let (defs, refs): (Vec<_>, Vec<_>) = sigs.into_iter().map(|s| (s.defs, s.refs)).unzip();

    result.defs.extend(defs.into_iter().flat_map(|ds| ds.into_iter()));
    result.refs.extend(refs.into_iter().flat_map(|rs| rs.into_iter()));

    result
}

fn text_sig(text: String) -> Signature {
    Signature {
        text: text,
        defs: vec![],
        refs: vec![],
    }
}

impl Sig for ast::Ty {
    fn make(&self, offset: usize, _parent_id: Option<NodeId>, scx: &SaveContext) -> Result {
        let id = Some(self.id);
        match self.node {
            ast::TyKind::Slice(ref ty) => {
                let nested = ty.make(offset + 1, id, scx)?;
                let text = format!("[{}]", nested.text);
                Ok(replace_text(nested, text))
            }
            ast::TyKind::Ptr(ref mt) => {
                let prefix = match mt.mutbl {
                    ast::Mutability::Mutable => "*mut ",
                    ast::Mutability::Immutable => "*const ",
                };
                let nested = mt.ty.make(offset + prefix.len(), id, scx)?;
                let text = format!("{}{}", prefix, nested.text);
                Ok(replace_text(nested, text))
            }
            ast::TyKind::Rptr(ref lifetime, ref mt) => {
                let mut prefix = "&".to_owned();
                if let &Some(ref l) = lifetime {
                    prefix.push_str(&l.ident.to_string());
                    prefix.push(' ');
                }
                if let ast::Mutability::Mutable = mt.mutbl {
                    prefix.push_str("mut ");
                };

                let nested = mt.ty.make(offset + prefix.len(), id, scx)?;
                let text = format!("{}{}", prefix, nested.text);
                Ok(replace_text(nested, text))
            }
            ast::TyKind::Never => {
                Ok(text_sig("!".to_owned()))
            },
            ast::TyKind::Tup(ref ts) => {
                let mut text = "(".to_owned();
                let mut defs = vec![];
                let mut refs = vec![];
                for t in ts {
                    let nested = t.make(offset + text.len(), id, scx)?;
                    text.push_str(&nested.text);
                    text.push(',');
                    defs.extend(nested.defs.into_iter());
                    refs.extend(nested.refs.into_iter());
                }
                text.push(')');
                Ok(Signature { text, defs, refs })
            }
            ast::TyKind::Paren(ref ty) => {
                let nested = ty.make(offset + 1, id, scx)?;
                let text = format!("({})", nested.text);
                Ok(replace_text(nested, text))
            }
            ast::TyKind::BareFn(ref f) => {
                let mut text = String::new();
                if !f.lifetimes.is_empty() {
                    // FIXME defs, bounds on lifetimes
                    text.push_str("for<");
                    text.push_str(&f.lifetimes.iter().map(|l|
                        l.lifetime.ident.to_string()).collect::<Vec<_>>().join(", "));
                    text.push('>');
                }

                if f.unsafety == ast::Unsafety::Unsafe {
                    text.push_str("unsafe ");
                }
                if f.abi != ::syntax::abi::Abi::Rust {
                    text.push_str("extern");
                    text.push_str(&f.abi.to_string());
                    text.push(' ');
                }
                text.push_str("fn(");

                let mut defs = vec![];
                let mut refs = vec![];
                for i in &f.decl.inputs {
                    let nested = i.ty.make(offset + text.len(), Some(i.id), scx)?;
                    text.push_str(&nested.text);
                    text.push(',');
                    defs.extend(nested.defs.into_iter());
                    refs.extend(nested.refs.into_iter());
                }
                text.push(')');
                if let ast::FunctionRetTy::Ty(ref t) = f.decl.output {
                    text.push_str(" -> ");
                    let nested = t.make(offset + text.len(), None, scx)?;
                    text.push_str(&nested.text);
                    text.push(',');
                    defs.extend(nested.defs.into_iter());
                    refs.extend(nested.refs.into_iter());
                }

                Ok(Signature { text, defs, refs })
            }
            ast::TyKind::Path(None, ref path) => {
                path.make(offset, id, scx)
            }
            ast::TyKind::Path(Some(ref qself), ref path) => {
                let nested_ty = qself.ty.make(offset + 1, id, scx)?;
                let prefix = if qself.position == 0 {
                    format!("<{}>::", nested_ty.text)
                } else if qself.position == 1 {
                    let first = pprust::path_segment_to_string(&path.segments[0]);
                    format!("<{} as {}>::", nested_ty.text, first)
                } else {
                    // FIXME handle path instead of elipses.
                    format!("<{} as ...>::", nested_ty.text)                    
                };

                let name = pprust::path_segment_to_string(path.segments.last().ok_or("Bad path")?);
                let def = scx.get_path_def(id.ok_or("Missing id for Path")?);
                let id = id_from_def_id(def.def_id());
                if path.segments.len() - qself.position == 1 {
                    let start = offset + prefix.len();
                    let end = start + name.len();

                    Ok(Signature {
                        text: prefix + &name,
                        defs: vec![],
                        refs: vec![SigElement { id, start, end }],
                    })
                } else {
                    let start = offset + prefix.len() + 5;
                    let end = start + name.len();
                    // FIXME should put the proper path in there, not elipses.
                    Ok(Signature {
                        text: prefix + "...::" + &name,
                        defs: vec![],
                        refs: vec![SigElement { id, start, end }],
                    })
                }
            }
            ast::TyKind::TraitObject(ref bounds) => {
                // FIXME recurse into bounds
                let nested = pprust::bounds_to_string(bounds);
                Ok(text_sig(nested))
            }
            ast::TyKind::ImplTrait(ref bounds) => {
                // FIXME recurse into bounds
                let nested = pprust::bounds_to_string(bounds);
                Ok(text_sig(format!("impl {}", nested)))
            }
            ast::TyKind::Array(ref ty, ref v) => {
                let nested_ty = ty.make(offset + 1, id, scx)?;
                let expr = pprust::expr_to_string(v).replace('\n', " ");
                let text = format!("[{}; {}]", nested_ty.text, expr);
                Ok(replace_text(nested_ty, text))
            }
            ast::TyKind::Typeof(_) |
            ast::TyKind::Infer |
            ast::TyKind::Err |
            ast::TyKind::ImplicitSelf |
            ast::TyKind::Mac(_) => Err("Ty"),
        }
    }    
}

impl Sig for ast::Item {
    fn make(&self, offset: usize, _parent_id: Option<NodeId>, scx: &SaveContext) -> Result {
        let id = Some(self.id);

        let name_and_generics = |mut text: String, generics: &ast::Generics| -> Result {
            let name = self.ident.to_string();
            let def = SigElement {
                id: id_from_node_id(self.id, scx),
                start: offset + 5,
                end: offset + 5 + name.len(),
            };
            text.push_str(&name);
            let generics: Signature = generics.make(offset + text.len(), id, scx)?;
            // FIXME where clause
            let text = format!("{}{}", text, generics.text);
            Ok(extend_sig(generics, text, vec![def], vec![]))
        };

        match self.node {
            ast::ItemKind::Static(ref ty, m, ref expr) => {
                let mut text = "static ".to_owned();
                if m == ast::Mutability::Mutable {
                    text.push_str("mut ");
                }
                let name = self.ident.to_string();
                let defs = vec![SigElement {
                    id: id_from_node_id(self.id, scx),
                    start: offset + text.len(),
                    end: offset + text.len() + name.len(),
                }];
                text.push_str(&name);
                text.push_str(": ");

                let ty = ty.make(offset + text.len(), id, scx)?;
                text.push_str(&ty.text);
                text.push_str(" = ");

                let expr = pprust::expr_to_string(expr).replace('\n', " ");
                text.push_str(&expr);
                text.push(';');

                Ok(extend_sig(ty, text, defs, vec![]))
            }
            ast::ItemKind::Const(ref ty, ref expr) => {
                let mut text = "const ".to_owned();
                let name = self.ident.to_string();
                let defs = vec![SigElement {
                    id: id_from_node_id(self.id, scx),
                    start: offset + text.len(),
                    end: offset + text.len() + name.len(),
                }];
                text.push_str(&name);
                text.push_str(": ");

                let ty = ty.make(offset + text.len(), id, scx)?;
                text.push_str(&ty.text);
                text.push_str(" = ");

                let expr = pprust::expr_to_string(expr).replace('\n', " ");
                text.push_str(&expr);
                text.push(';');

                Ok(extend_sig(ty, text, defs, vec![]))
            }
            ast::ItemKind::Fn(ref decl, unsafety, constness, abi, ref generics, _) => {
                let mut text = String::new();
                if constness.node == ast::Constness::Const {
                    text.push_str("const ");
                }
                if unsafety == ast::Unsafety::Unsafe {
                    text.push_str("unsafe ");
                }
                if abi != ::syntax::abi::Abi::Rust {
                    text.push_str("extern");
                    text.push_str(&abi.to_string());
                    text.push(' ');
                }
                text.push_str("fn ");

                let mut sig = name_and_generics(text, generics)?;

                sig.text.push('(');
                for i in &decl.inputs {
                    // FIXME shoudl descend into patterns to add defs.
                    sig.text.push_str(&pprust::pat_to_string(&i.pat));
                    sig.text.push_str(": ");
                    let nested = i.ty.make(offset + sig.text.len(), Some(i.id), scx)?;
                    sig.text.push_str(&nested.text);
                    sig.text.push(',');
                    sig.defs.extend(nested.defs.into_iter());
                    sig.refs.extend(nested.refs.into_iter());
                }
                sig.text.push(')');

                if let ast::FunctionRetTy::Ty(ref t) = decl.output {
                    sig.text.push_str(" -> ");
                    let nested = t.make(offset + sig.text.len(), None, scx)?;
                    sig.text.push_str(&nested.text);
                    sig.text.push(',');
                    sig.defs.extend(nested.defs.into_iter());
                    sig.refs.extend(nested.refs.into_iter());
                }

                Ok(sig)
            }
            ast::ItemKind::Mod(ref _mod) => {
                let mut text = "mod ".to_owned();
                let name = self.ident.to_string();
                let defs = vec![SigElement {
                    id: id_from_node_id(self.id, scx),
                    start: offset + text.len(),
                    end: offset + text.len() + name.len(),
                }];
                text.push_str(&name);
                // Could be either `mod foo;` or `mod foo { ... }`, but we'll just puck one.
                text.push(';');

                Ok(Signature {
                    text,
                    defs,
                    refs: vec![],
                })
            }
            ast::ItemKind::Ty(ref ty, ref generics) => {
                let text = "type ".to_owned();
                let mut sig = name_and_generics(text, generics)?;

                sig.text.push_str(" = ");
                let ty = ty.make(offset + sig.text.len(), id, scx)?;
                sig.text.push_str(&ty.text);
                sig.text.push(';');

                Ok(merge_sigs(sig.text.clone(), vec![sig, ty]))
            }
            ast::ItemKind::Enum(_, ref generics) => {
                let text = "enum ".to_owned();
                let mut sig = name_and_generics(text, generics)?;
                sig.text.push_str(" {}");
                Ok(sig)
            }
            ast::ItemKind::Struct(_, ref generics) => {
                let text = "struct ".to_owned();
                let mut sig = name_and_generics(text, generics)?;
                sig.text.push_str(" {}");
                Ok(sig)
            }
            ast::ItemKind::Union(_, ref generics) => {
                let text = "union ".to_owned();
                let mut sig = name_and_generics(text, generics)?;
                sig.text.push_str(" {}");
                Ok(sig)
            }
            ast::ItemKind::Trait(unsafety, ref generics, ref bounds, _) => {
                let mut text = String::new();
                if unsafety == ast::Unsafety::Unsafe {
                    text.push_str("unsafe ");
                }
                text.push_str("trait ");
                let mut sig = name_and_generics(text, generics)?;

                if !bounds.is_empty() {
                    sig.text.push_str(": ");
                    sig.text.push_str(&pprust::bounds_to_string(bounds));
                }
                // FIXME where clause
                sig.text.push_str(" {}");

                Ok(sig)
            }
            ast::ItemKind::DefaultImpl(unsafety, ref trait_ref) => {
                let mut text = String::new();
                if unsafety == ast::Unsafety::Unsafe {
                    text.push_str("unsafe ");
                }
                text.push_str("impl ");
                let trait_sig = trait_ref.path.make(offset + text.len(), id, scx)?;
                text.push_str(&trait_sig.text);
                text.push_str(" for .. {}");
                Ok(replace_text(trait_sig, text))
            }
            ast::ItemKind::Impl(unsafety,
                                polarity,
                                defaultness,
                                ref generics,
                                ref opt_trait,
                                ref ty,
                                _) => {
                let mut text = String::new();
                if let ast::Defaultness::Default = defaultness {
                    text.push_str("default ");
                }
                if unsafety == ast::Unsafety::Unsafe {
                    text.push_str("unsafe ");
                }
                text.push_str("impl");

                let generics_sig = generics.make(offset + text.len(), id, scx)?;
                text.push_str(&generics_sig.text);

                text.push(' ');

                let trait_sig = if let Some(ref t) = *opt_trait {
                    if polarity == ast::ImplPolarity::Negative {
                        text.push('!');
                    }
                    let trait_sig = t.path.make(offset + text.len(), id, scx)?;
                    text.push_str(&trait_sig.text);
                    text.push_str(" for ");
                    trait_sig
                } else {
                    text_sig(String::new())
                };

                let ty_sig = ty.make(offset + text.len(), id, scx)?;
                text.push_str(&ty_sig.text);
                
                text.push_str(" {}");

                Ok(merge_sigs(text, vec![generics_sig, trait_sig, ty_sig]))

                // FIXME where clause
            }
            ast::ItemKind::ForeignMod(_) => Err("extern mod"),
            ast::ItemKind::GlobalAsm(_) => Err("glboal asm"),
            ast::ItemKind::ExternCrate(_) => Err("extern crate"),
            // FIXME should implement this (e.g., pub use).
            ast::ItemKind::Use(_) => Err("import"),
            ast::ItemKind::Mac(..) |
            ast::ItemKind::MacroDef(_) => Err("Macro"),
        }
    }
}

impl Sig for ast::Path {
    fn make(&self, offset: usize, id: Option<NodeId>, scx: &SaveContext) -> Result {
        let def = scx.get_path_def(id.ok_or("Missing id for Path")?);
        let id = id_from_def_id(def.def_id());

        let (name, start, end) = match def {
            Def::AssociatedConst(..) |
            Def::Variant(..) |
            Def::VariantCtor(..) => {
                let len = self.segments.len();
                if len < 2 {
                    return Err("Bad path");
                }
                // FIXME: really we should descend into the generics here and add SigElements for
                // them.
                // FIXME: would be nice to have a def for the first path segment.
                let seg1 = pprust::path_segment_to_string(&self.segments[len - 2]);
                let seg2 = pprust::path_segment_to_string(&self.segments[len - 1]);
                let start = offset + seg1.len() + 2;
                (format!("{}::{}", seg1, seg2), start, start + seg2.len())
            }
            _ => {
                let name = pprust::path_segment_to_string(self.segments.last().ok_or("Bad path")?);
                let end = offset + name.len();
                (name, offset, end)
            }
        };

        Ok(Signature {
            text: name,
            defs: vec![],
            refs: vec![SigElement { id, start, end }],
        })
    }
}

// This does not cover the where clause, which must be processed separately.
impl Sig for ast::Generics {
    fn make(&self, offset: usize, _parent_id: Option<NodeId>, scx: &SaveContext) -> Result {
        let total = self.lifetimes.len() + self.ty_params.len();
        if total == 0 {
            return Ok(text_sig(String::new()));
        }

        let mut text = "<".to_owned();

        let mut defs = vec![];
        for l in &self.lifetimes {
            let mut l_text = l.lifetime.ident.to_string();
            defs.push(SigElement {
                id: id_from_node_id(l.lifetime.id, scx),
                start: offset + text.len(),
                end: offset + text.len() + l_text.len(),
            });

            if !l.bounds.is_empty() {
                l_text.push_str(": ");
                let bounds = l.bounds.iter().map(|l| l.ident.to_string()).collect::<Vec<_>>().join(" + ");
                l_text.push_str(&bounds);
                // FIXME add lifetime bounds refs.
            }
            text.push_str(&l_text);
            text.push(',');
        }
        for t in &self.ty_params {
            let mut t_text = t.ident.to_string();
            defs.push(SigElement {
                id: id_from_node_id(t.id, scx),
                start: offset + text.len(),
                end: offset + text.len() + t_text.len(),
            });

            if !t.bounds.is_empty() {
                t_text.push_str(": ");
                t_text.push_str(&pprust::bounds_to_string(&t.bounds));
                // FIXME descend properly into bounds.
            }
            text.push_str(&t_text);
            text.push(',');
        }

        text.push('>');
        Ok(Signature {text, defs, refs: vec![] })
    }
}

// TODO impl items, trait items
