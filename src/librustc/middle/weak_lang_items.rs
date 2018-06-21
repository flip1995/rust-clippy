// Copyright 2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Validity checking for weak lang items

use session::config;
use middle::lang_items;

use rustc_target::spec::PanicStrategy;
use syntax::ast;
use syntax::symbol::Symbol;
use syntax_pos::Span;
use hir::def_id::DefId;
use hir::intravisit::{Visitor, NestedVisitorMap};
use hir::intravisit;
use hir;
use ty::TyCtxt;

use std::collections::HashSet;

macro_rules! weak_lang_items {
    ($($name:ident, $item:ident, $sym:ident;)*) => (

struct Context<'a, 'tcx: 'a> {
    tcx: TyCtxt<'a, 'tcx, 'tcx>,
    items: &'a mut lang_items::LanguageItems,
}

/// Checks the crate for usage of weak lang items, returning a vector of all the
/// language items required by this crate, but not defined yet.
pub fn check_crate<'a, 'tcx>(tcx: TyCtxt<'a, 'tcx, 'tcx>,
                             items: &mut lang_items::LanguageItems) {
    // These are never called by user code, they're generated by the compiler.
    // They will never implicitly be added to the `missing` array unless we do
    // so here.
    if items.eh_personality().is_none() {
        items.missing.push(lang_items::EhPersonalityLangItem);
    }
    if tcx.sess.target.target.options.custom_unwind_resume &
       items.eh_unwind_resume().is_none() {
        items.missing.push(lang_items::EhUnwindResumeLangItem);
    }

    {
        let mut cx = Context { tcx, items };
        tcx.hir.krate().visit_all_item_likes(&mut cx.as_deep_visitor());
    }
    verify(tcx, items);
}

pub fn link_name(attrs: &[ast::Attribute]) -> Option<Symbol> {
    lang_items::extract(attrs).and_then(|(name, _)| {
        $(if name == stringify!($name) {
            Some(Symbol::intern(stringify!($sym)))
        } else)* {
            None
        }
    })
}

/// Returns whether the specified `lang_item` doesn't actually need to be
/// present for this compilation.
///
/// Not all lang items are always required for each compilation, particularly in
/// the case of panic=abort. In these situations some lang items are injected by
/// crates and don't actually need to be defined in libstd.
pub fn whitelisted(tcx: TyCtxt, lang_item: lang_items::LangItem) -> bool {
    // If we're not compiling with unwinding, we won't actually need these
    // symbols. Other panic runtimes ensure that the relevant symbols are
    // available to link things together, but they're never exercised.
    if tcx.sess.panic_strategy() != PanicStrategy::Unwind {
        return lang_item == lang_items::EhPersonalityLangItem ||
            lang_item == lang_items::EhUnwindResumeLangItem
    }

    false
}

fn verify<'a, 'tcx>(tcx: TyCtxt<'a, 'tcx, 'tcx>,
                    items: &lang_items::LanguageItems) {
    // We only need to check for the presence of weak lang items if we're
    // emitting something that's not an rlib.
    let needs_check = tcx.sess.crate_types.borrow().iter().any(|kind| {
        match *kind {
            config::CrateTypeDylib |
            config::CrateTypeProcMacro |
            config::CrateTypeCdylib |
            config::CrateTypeExecutable |
            config::CrateTypeStaticlib => true,
            config::CrateTypeRlib => false,
        }
    });
    if !needs_check {
        return
    }

    let mut missing = HashSet::new();
    for &cnum in tcx.crates().iter() {
        for &item in tcx.missing_lang_items(cnum).iter() {
            missing.insert(item);
        }
    }

    $(
        if missing.contains(&lang_items::$item) &&
           !whitelisted(tcx, lang_items::$item) &&
           items.$name().is_none() {
            tcx.sess.err(&format!("language item required, but not found: `{}`",
                                  stringify!($name)));

        }
    )*
}

impl<'a, 'tcx> Context<'a, 'tcx> {
    fn register(&mut self, name: &str, span: Span) {
        $(if name == stringify!($name) {
            if self.items.$name().is_none() {
                self.items.missing.push(lang_items::$item);
            }
        } else)* {
            span_err!(self.tcx.sess, span, E0264,
                      "unknown external lang item: `{}`",
                      name);
        }
    }
}

impl<'a, 'tcx, 'v> Visitor<'v> for Context<'a, 'tcx> {
    fn nested_visit_map<'this>(&'this mut self) -> NestedVisitorMap<'this, 'v> {
        NestedVisitorMap::None
    }

    fn visit_foreign_item(&mut self, i: &hir::ForeignItem) {
        if let Some((lang_item, _)) = lang_items::extract(&i.attrs) {
            self.register(&lang_item.as_str(), i.span);
        }
        intravisit::walk_foreign_item(self, i)
    }
}

impl<'a, 'tcx, 'gcx> TyCtxt<'a, 'tcx, 'gcx> {
    pub fn is_weak_lang_item(&self, item_def_id: DefId) -> bool {
        let lang_items = self.lang_items();
        let did = Some(item_def_id);

        $(lang_items.$name() == did)||+
    }
}

) }

weak_lang_items! {
    panic_impl,         PanicImplLangItem,          rust_begin_unwind;
    eh_personality,     EhPersonalityLangItem,      rust_eh_personality;
    eh_unwind_resume,   EhUnwindResumeLangItem,     rust_eh_unwind_resume;
    oom,                OomLangItem,                rust_oom;
}
