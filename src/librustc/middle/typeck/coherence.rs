// Copyright 2012 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// Coherence phase
//
// The job of the coherence phase of typechecking is to ensure that each trait
// has at most one implementation for each type. Then we build a mapping from
// each trait in the system to its implementations.

use core::prelude::*;

use driver;
use metadata::csearch::{each_path, get_impl_traits};
use metadata::csearch::{get_impls_for_mod};
use metadata::csearch;
use metadata::cstore::{CStore, iter_crate_data};
use metadata::decoder::{dl_def, dl_field, dl_impl};
use middle::resolve::{Impl, MethodInfo};
use middle::ty::{ProvidedMethodSource, ProvidedMethodInfo, bound_copy, get};
use middle::ty::{lookup_item_type, subst};
use middle::ty::{substs, t, ty_bool, ty_bot, ty_box, ty_enum, ty_err};
use middle::ty::{ty_estr, ty_evec, ty_float, ty_infer, ty_int, ty_nil};
use middle::ty::{ty_opaque_box, ty_param, ty_param_bounds_and_ty, ty_ptr};
use middle::ty::{ty_rptr, ty_self, ty_struct, ty_trait, ty_tup};
use middle::ty::{ty_type, ty_uint, ty_uniq, ty_bare_fn, ty_closure};
use middle::ty::{ty_opaque_closure_ptr, ty_unboxed_vec};
use middle::ty::{type_is_ty_var};
use middle::subst::Subst;
use middle::ty;
use middle::typeck::CrateCtxt;
use middle::typeck::infer::combine::Combine;
use middle::typeck::infer::InferCtxt;
use middle::typeck::infer::{new_infer_ctxt, resolve_ivar};
use middle::typeck::infer::{resolve_nested_tvar, resolve_type};
use syntax::ast::{crate, def_id, def_mod, def_trait};
use syntax::ast::{item, item_impl, item_mod, local_crate, method, trait_ref};
use syntax::ast;
use syntax::ast_map::node_item;
use syntax::ast_map;
use syntax::ast_util::{def_id_of_def, local_def};
use syntax::codemap::{span, dummy_sp};
use syntax::parse;
use syntax::visit::{default_simple_visitor, default_visitor};
use syntax::visit::{mk_simple_visitor, mk_vt, visit_crate, visit_item};
use syntax::visit::{Visitor, SimpleVisitor};
use syntax::visit::{visit_mod};
use util::ppaux::ty_to_str;

use core::result::Ok;
use core::hashmap::{HashMap, HashSet};
use core::uint;

pub struct UniversalQuantificationResult {
    monotype: t,
    type_variables: ~[ty::t],
    type_param_defs: @~[ty::TypeParameterDef]
}

pub fn get_base_type(inference_context: @mut InferCtxt,
                     span: span,
                     original_type: t)
                  -> Option<t> {
    let resolved_type;
    match resolve_type(inference_context,
                       original_type,
                       resolve_ivar) {
        Ok(resulting_type) if !type_is_ty_var(resulting_type) => {
            resolved_type = resulting_type;
        }
        _ => {
            inference_context.tcx.sess.span_fatal(span,
                                                  ~"the type of this value \
                                                    must be known in order \
                                                    to determine the base \
                                                    type");
        }
    }

    match get(resolved_type).sty {
        ty_enum(*) | ty_trait(*) | ty_struct(*) => {
            debug!("(getting base type) found base type");
            Some(resolved_type)
        }

        ty_nil | ty_bot | ty_bool | ty_int(*) | ty_uint(*) | ty_float(*) |
        ty_estr(*) | ty_evec(*) | ty_bare_fn(*) | ty_closure(*) | ty_tup(*) |
        ty_infer(*) | ty_param(*) | ty_self(*) | ty_type | ty_opaque_box |
        ty_opaque_closure_ptr(*) | ty_unboxed_vec(*) | ty_err | ty_box(_) |
        ty_uniq(_) | ty_ptr(_) | ty_rptr(_, _) => {
            debug!("(getting base type) no base type; found %?",
                   get(original_type).sty);
            None
        }
    }
}

pub fn type_is_defined_in_local_crate(original_type: t) -> bool {
    /*!
     *
     * For coherence, when we have `impl Trait for Type`, we need to
     * guarantee that `Type` is "local" to the
     * crate.  For our purposes, this means that it must contain
     * some nominal type defined in this crate.
     */

    let mut found_nominal = false;
    do ty::walk_ty(original_type) |t| {
        match get(t).sty {
            ty_enum(def_id, _) |
            ty_trait(def_id, _, _, _) |
            ty_struct(def_id, _) => {
                if def_id.crate == ast::local_crate {
                    found_nominal = true;
                }
            }

            _ => { }
        }
    }
    return found_nominal;
}

// Returns the def ID of the base type, if there is one.
pub fn get_base_type_def_id(inference_context: @mut InferCtxt,
                            span: span,
                            original_type: t)
                         -> Option<def_id> {
    match get_base_type(inference_context, span, original_type) {
        None => {
            return None;
        }
        Some(base_type) => {
            match get(base_type).sty {
                ty_enum(def_id, _) |
                ty_struct(def_id, _) |
                ty_trait(def_id, _, _, _) => {
                    return Some(def_id);
                }
                _ => {
                    fail!(~"get_base_type() returned a type that wasn't an \
                           enum, class, or trait");
                }
            }
        }
    }
}


pub fn method_to_MethodInfo(ast_method: @method) -> @MethodInfo {
    @MethodInfo {
        did: local_def(ast_method.id),
        n_tps: ast_method.generics.ty_params.len(),
        ident: ast_method.ident,
        self_type: ast_method.self_ty.node
    }
}

pub struct CoherenceInfo {
    // Contains implementations of methods that are inherent to a type.
    // Methods in these implementations don't need to be exported.
    inherent_methods: @mut HashMap<def_id, @mut ~[@Impl]>,

    // Contains implementations of methods associated with a trait. For these,
    // the associated trait must be imported at the call site.
    extension_methods: @mut HashMap<def_id, @mut ~[@Impl]>,
}

pub fn CoherenceInfo() -> CoherenceInfo {
    CoherenceInfo {
        inherent_methods: @mut HashMap::new(),
        extension_methods: @mut HashMap::new(),
    }
}

pub fn CoherenceChecker(crate_context: @mut CrateCtxt) -> CoherenceChecker {
    CoherenceChecker {
        crate_context: crate_context,
        inference_context: new_infer_ctxt(crate_context.tcx),

        base_type_def_ids: @mut HashMap::new(),
    }
}

pub struct CoherenceChecker {
    crate_context: @mut CrateCtxt,
    inference_context: @mut InferCtxt,

    // A mapping from implementations to the corresponding base type
    // definition ID.

    base_type_def_ids: @mut HashMap<def_id,def_id>,
}

pub impl CoherenceChecker {
    fn check_coherence(self, crate: @crate) {
        // Check implementations and traits. This populates the tables
        // containing the inherent methods and extension methods. It also
        // builds up the trait inheritance table.
        visit_crate(*crate, (), mk_simple_visitor(@SimpleVisitor {
            visit_item: |item| {
//                debug!("(checking coherence) item '%s'",
//                       self.crate_context.tcx.sess.str_of(item.ident));

                match item.node {
                    item_impl(_, opt_trait, _, _) => {
                        self.check_implementation(item,
                                                  iter::to_vec(&opt_trait));
                    }
                    _ => {
                        // Nothing to do.
                    }
                };
            },
            .. *default_simple_visitor()
        }));

        // Check that there are no overlapping trait instances
        self.check_implementation_coherence();

        // Check whether traits with base types are in privileged scopes.
        self.check_privileged_scopes(crate);

        // Bring in external crates. It's fine for this to happen after the
        // coherence checks, because we ensure by construction that no errors
        // can happen at link time.
        self.add_external_crates();

        // Populate the table of destructors. It might seem a bit strange to
        // do this here, but it's actually the most convenient place, since
        // the coherence tables contain the trait -> type mappings.
        self.populate_destructor_table();
    }

    fn check_implementation(&self,
                            item: @item, associated_traits: ~[@trait_ref]) {
        let self_type = self.crate_context.tcx.tcache.get(
            &local_def(item.id));

        // If there are no traits, then this implementation must have a
        // base type.

        if associated_traits.len() == 0 {
            debug!("(checking implementation) no associated traits for item \
                    '%s'",
                   *self.crate_context.tcx.sess.str_of(item.ident));

            match get_base_type_def_id(self.inference_context,
                                       item.span,
                                       self_type.ty) {
                None => {
                    let session = self.crate_context.tcx.sess;
                    session.span_err(item.span,
                                     ~"no base type found for inherent \
                                       implementation; implement a \
                                       trait or new type instead");
                }
                Some(_) => {
                    // Nothing to do.
                }
            }
        }

        // We only want to generate one Impl structure. When we generate one,
        // we store it here so that we don't recreate it.
        let mut implementation_opt = None;
        for associated_traits.each |&associated_trait| {
            let trait_ref =
                ty::node_id_to_trait_ref(
                    self.crate_context.tcx,
                    associated_trait.ref_id);
            debug!("(checking implementation) adding impl for trait '%s', item '%s'",
                   trait_ref.repr(self.crate_context.tcx),
                   *self.crate_context.tcx.sess.str_of(item.ident));

            self.instantiate_default_methods(item.id, trait_ref);

            let implementation;
            if implementation_opt.is_none() {
                implementation = self.create_impl_from_item(item);
                implementation_opt = Some(implementation);
            }

            self.add_trait_method(trait_ref.def_id, implementation_opt.get());
        }

        // Add the implementation to the mapping from implementation to base
        // type def ID, if there is a base type for this implementation and
        // the implementation does not have any associated traits.
        match get_base_type_def_id(self.inference_context,
                                   item.span,
                                   self_type.ty) {
            None => {
                // Nothing to do.
            }
            Some(base_type_def_id) => {
                // XXX: Gather up default methods?
                if associated_traits.len() == 0 {
                    let implementation;
                    match implementation_opt {
                        None => {
                            implementation =
                                self.create_impl_from_item(item);
                        }
                        Some(copy existing_implementation) => {
                            implementation = existing_implementation;
                        }
                    }
                    self.add_inherent_method(base_type_def_id,
                                             implementation);
                }

                self.base_type_def_ids.insert(local_def(item.id),
                                              base_type_def_id);
            }
        }
    }

    // Creates default method IDs and performs type substitutions for an impl
    // and trait pair. Then, for each provided method in the trait, inserts a
    // `ProvidedMethodInfo` instance into the `provided_method_sources` map.
    fn instantiate_default_methods(&self,
                                   impl_id: ast::node_id,
                                   trait_ref: &ty::TraitRef) {
        let tcx = self.crate_context.tcx;
        debug!("instantiate_default_methods(impl_id=%?, trait_ref=%s)",
               impl_id, trait_ref.repr(tcx));

        let impl_poly_type = ty::lookup_item_type(tcx, local_def(impl_id));

        for self.each_provided_trait_method(trait_ref.def_id) |trait_method| {
            // Synthesize an ID.
            let new_id = parse::next_node_id(tcx.sess.parse_sess);
            let new_did = local_def(new_id);

            debug!("new_did=%? trait_method=%s", new_did, trait_method.repr(tcx));

            // Create substitutions for the various trait parameters.
            let new_method_ty =
                @subst_receiver_types_in_method_ty(
                    tcx,
                    impl_id,
                    trait_ref,
                    new_did,
                    trait_method);

            debug!("new_method_ty=%s", new_method_ty.repr(tcx));

            // construct the polytype for the method based on the method_ty
            let new_generics = ty::Generics {
                type_param_defs:
                    @vec::append(
                        copy *impl_poly_type.generics.type_param_defs,
                        *new_method_ty.generics.type_param_defs),
                region_param:
                    impl_poly_type.generics.region_param
            };
            let new_polytype = ty::ty_param_bounds_and_ty {
                generics: new_generics,
                ty: ty::mk_bare_fn(tcx, copy new_method_ty.fty)
            };
            debug!("new_polytype=%s", new_polytype.repr(tcx));

            tcx.tcache.insert(new_did, new_polytype);
            tcx.methods.insert(new_did, new_method_ty);

            // Pair the new synthesized ID up with the
            // ID of the method.
            let source = ProvidedMethodSource {
                method_id: trait_method.def_id,
                impl_id: local_def(impl_id)
            };

            self.crate_context.tcx.provided_method_sources.insert(new_did,
                                                                  source);

            let provided_method_info =
                @ProvidedMethodInfo {
                    method_info: @MethodInfo {
                        did: new_did,
                        n_tps: trait_method.generics.type_param_defs.len(),
                        ident: trait_method.ident,
                        self_type: trait_method.self_ty
                    },
                    trait_method_def_id: trait_method.def_id
                };

            let pmm = self.crate_context.tcx.provided_methods;
            match pmm.find(&local_def(impl_id)) {
                Some(mis) => {
                    // If the trait already has an entry in the
                    // provided_methods_map, we just need to add this
                    // method to that entry.
                    debug!("(checking implementation) adding method `%s` \
                            to entry for existing trait",
                            *self.crate_context.tcx.sess.str_of(
                                provided_method_info.method_info.ident));
                    mis.push(provided_method_info);
                }
                None => {
                    // If the trait doesn't have an entry yet, create one.
                    debug!("(checking implementation) creating new entry \
                            for method `%s`",
                            *self.crate_context.tcx.sess.str_of(
                                provided_method_info.method_info.ident));
                    pmm.insert(local_def(impl_id),
                               @mut ~[provided_method_info]);
                }
            }
        }
    }

    fn add_inherent_method(&self,
                           base_def_id: def_id, implementation: @Impl) {
        let implementation_list;
        match self.crate_context.coherence_info.inherent_methods
                  .find(&base_def_id) {
            None => {
                implementation_list = @mut ~[];
                self.crate_context.coherence_info.inherent_methods
                    .insert(base_def_id, implementation_list);
            }
            Some(existing_implementation_list) => {
                implementation_list = *existing_implementation_list;
            }
        }

        implementation_list.push(implementation);
    }

    fn add_trait_method(&self, trait_id: def_id, implementation: @Impl) {
        let implementation_list;
        match self.crate_context.coherence_info.extension_methods
                  .find(&trait_id) {
            None => {
                implementation_list = @mut ~[];
                self.crate_context.coherence_info.extension_methods
                    .insert(trait_id, implementation_list);
            }
            Some(existing_implementation_list) => {
                implementation_list = *existing_implementation_list;
            }
        }

        implementation_list.push(implementation);
    }

    fn check_implementation_coherence(&self) {
        let coherence_info = &mut self.crate_context.coherence_info;
        let extension_methods = &coherence_info.extension_methods;

        for extension_methods.each_key |&trait_id| {
            self.check_implementation_coherence_of(trait_id);
        }
    }

    fn check_implementation_coherence_of(&self, trait_def_id: def_id) {

        // Unify pairs of polytypes.
        do self.iter_impls_of_trait(trait_def_id) |a| {
            let implementation_a = a;
            let polytype_a =
                self.get_self_type_for_implementation(implementation_a);

            // "We have an impl of trait <trait_def_id> for type <polytype_a>,
            // and that impl is <implementation_a>"
            self.add_impl_for_trait(trait_def_id, polytype_a.ty,
                                    implementation_a);
            do self.iter_impls_of_trait(trait_def_id) |b| {
                let implementation_b = b;

                // An impl is coherent with itself
                if a.did != b.did {
                    let polytype_b = self.get_self_type_for_implementation(
                            implementation_b);

                    if self.polytypes_unify(polytype_a, polytype_b) {
                        let session = self.crate_context.tcx.sess;
                        session.span_err(self.span_of_impl(implementation_b),
                                         ~"conflicting implementations for a \
                                           trait");
                        session.span_note(self.span_of_impl(implementation_a),
                                          ~"note conflicting implementation \
                                            here");
                    }
                }
            }
        }
    }

    // Adds an impl of trait trait_t for self type self_t; that impl
    // is the_impl
    fn add_impl_for_trait(&self,
                          trait_t: def_id, self_t: t, the_impl: @Impl) {
        debug!("Adding impl %? of %? for %s",
               the_impl.did, trait_t,
               ty_to_str(self.crate_context.tcx, self_t));
        match self.crate_context.tcx.trait_impls.find(&trait_t) {
            None => {
                let m = @mut HashMap::new();
                m.insert(self_t, the_impl);
                self.crate_context.tcx.trait_impls.insert(trait_t, m);
            }
            Some(m) => {
                m.insert(self_t, the_impl);
            }
        }
    }

    fn iter_impls_of_trait(&self, trait_def_id: def_id, f: &fn(@Impl)) {
        let coherence_info = &mut self.crate_context.coherence_info;
        let extension_methods = &coherence_info.extension_methods;

        match extension_methods.find(&trait_def_id) {
            Some(impls) => {
                let impls: &mut ~[@Impl] = *impls;
                for uint::range(0, impls.len()) |i| {
                    f(impls[i]);
                }
            }
            None => { /* no impls? */ }
        }
    }

    fn each_provided_trait_method(&self,
            trait_did: ast::def_id,
            f: &fn(x: @ty::method) -> bool) {
        // Make a list of all the names of the provided methods.
        // XXX: This is horrible.
        let mut provided_method_idents = HashSet::new();
        let tcx = self.crate_context.tcx;
        for ty::provided_trait_methods(tcx, trait_did).each |ident| {
            provided_method_idents.insert(*ident);
        }

        for ty::trait_methods(tcx, trait_did).each |&method| {
            if provided_method_idents.contains(&method.ident) {
                if !f(method) {
                    break;
                }
            }
        }
    }

    fn polytypes_unify(&self, polytype_a: ty_param_bounds_and_ty,
                       polytype_b: ty_param_bounds_and_ty)
                    -> bool {
        let universally_quantified_a =
            self.universally_quantify_polytype(polytype_a);
        let universally_quantified_b =
            self.universally_quantify_polytype(polytype_b);

        return self.can_unify_universally_quantified(
            &universally_quantified_a, &universally_quantified_b) ||
            self.can_unify_universally_quantified(
            &universally_quantified_b, &universally_quantified_a);
    }

    // Converts a polytype to a monotype by replacing all parameters with
    // type variables. Returns the monotype and the type variables created.
    fn universally_quantify_polytype(&self, polytype: ty_param_bounds_and_ty)
                                  -> UniversalQuantificationResult {
        // NDM--this span is bogus.
        let self_region =
            polytype.generics.region_param.map(
                |_r| self.inference_context.next_region_var_nb(dummy_sp()));

        let bounds_count = polytype.generics.type_param_defs.len();
        let type_parameters = self.inference_context.next_ty_vars(bounds_count);

        let substitutions = substs {
            self_r: self_region,
            self_ty: None,
            tps: type_parameters
        };
        let monotype = subst(self.crate_context.tcx,
                             &substitutions,
                             polytype.ty);

        // Get our type parameters back.
        let substs { self_r: _, self_ty: _, tps: type_parameters } =
            substitutions;

        UniversalQuantificationResult {
            monotype: monotype,
            type_variables: type_parameters,
            type_param_defs: polytype.generics.type_param_defs
        }
    }

    fn can_unify_universally_quantified<'a>
            (&self,
             a: &'a UniversalQuantificationResult,
             b: &'a UniversalQuantificationResult)
          -> bool {
        let mut might_unify = true;
        let _ = do self.inference_context.probe {
            let result = self.inference_context.sub(true, dummy_sp())
                                               .tys(a.monotype, b.monotype);
            if result.is_ok() {
                // Check to ensure that each parameter binding respected its
                // kind bounds.
                for [ a, b ].each |result| {
                    for vec::each2(result.type_variables, *result.type_param_defs)
                            |ty_var, type_param_def| {
                        match resolve_type(self.inference_context,
                                           *ty_var,
                                           resolve_nested_tvar) {
                            Ok(resolved_ty) => {
                                for type_param_def.bounds.each |bound| {
                                    match *bound {
                                        bound_copy => {
                                            if !ty::type_is_copyable(
                                                self.inference_context.tcx,
                                                resolved_ty)
                                            {
                                                might_unify = false;
                                                break;
                                            }
                                        }

                                        // XXX: We could be smarter here.
                                        // Check to see whether owned, send,
                                        // const, trait param bounds could
                                        // possibly unify.
                                        _ => {}
                                    }
                                }
                            }
                            Err(*) => {
                                // Conservatively assume it might unify.
                            }
                        }
                    }
                }
            } else {
                might_unify = false;
            }

            result
        };
        might_unify
    }

    fn get_self_type_for_implementation(&self, implementation: @Impl)
                                     -> ty_param_bounds_and_ty {
        return *self.crate_context.tcx.tcache.get(&implementation.did);
    }

    // Privileged scope checking
    fn check_privileged_scopes(self, crate: @crate) {
        visit_crate(*crate, (), mk_vt(@Visitor {
            visit_item: |item, _context, visitor| {
                match item.node {
                    item_mod(ref module_) => {
                        // Then visit the module items.
                        visit_mod(module_, item.span, item.id, (), visitor);
                    }
                    item_impl(_, opt_trait, _, _) => {
                        // `for_ty` is `Type` in `impl Trait for Type`
                        let for_ty =
                            ty::node_id_to_type(self.crate_context.tcx,
                                                item.id);
                        if !type_is_defined_in_local_crate(for_ty) {
                            // This implementation is not in scope of its base
                            // type. This still might be OK if the trait is
                            // defined in the same crate.

                            match opt_trait {
                                None => {
                                    // There is no trait to implement, so
                                    // this is an error.

                                    let session = self.crate_context.tcx.sess;
                                    session.span_err(item.span,
                                                     ~"cannot implement \
                                                      inherent methods for a \
                                                      type outside the crate \
                                                      the type was defined \
                                                      in; define and \
                                                      implement a trait or \
                                                      new type instead");
                                }

                                Some(trait_ref) => {
                                    // This is OK if and only if the trait was
                                    // defined in this crate.

                                    let trait_def_id =
                                        self.trait_ref_to_trait_def_id(
                                            trait_ref);

                                    if trait_def_id.crate != local_crate {
                                        let session = self.crate_context.tcx.sess;
                                        session.span_err(item.span,
                                                         ~"cannot provide an \
                                                           extension \
                                                           implementation for a \
                                                           trait not defined in \
                                                           this crate");
                                    }
                                }
                            }
                        }

                        visit_item(item, (), visitor);
                    }
                    _ => {
                        visit_item(item, (), visitor);
                    }
                }
            },
            .. *default_visitor()
        }));
    }

    fn trait_ref_to_trait_def_id(&self, trait_ref: @trait_ref) -> def_id {
        let def_map = self.crate_context.tcx.def_map;
        let trait_def = *def_map.get(&trait_ref.ref_id);
        let trait_id = def_id_of_def(trait_def);
        return trait_id;
    }

    // This check doesn't really have anything to do with coherence. It's
    // here for historical reasons
    fn please_check_that_trait_methods_are_implemented(&self,
        all_methods: &mut ~[@MethodInfo],
        trait_did: def_id,
        trait_ref_span: span) {

        let tcx = self.crate_context.tcx;

        let mut provided_names = HashSet::new();
        // Implemented methods
        for uint::range(0, all_methods.len()) |i| {
            provided_names.insert(all_methods[i].ident);
        }
        // Default methods
        for ty::provided_trait_methods(tcx, trait_did).each |ident| {
            provided_names.insert(*ident);
        }

        for (*ty::trait_methods(tcx, trait_did)).each |method| {
            if provided_names.contains(&method.ident) { loop; }

            tcx.sess.span_err(trait_ref_span,
                              fmt!("missing method `%s`",
                                   *tcx.sess.str_of(method.ident)));
        }
    }

    // Converts an implementation in the AST to an Impl structure.
    fn create_impl_from_item(&self, item: @item) -> @Impl {
        fn add_provided_methods(all_methods: &mut ~[@MethodInfo],
                            all_provided_methods: &mut ~[@ProvidedMethodInfo],
                            sess: driver::session::Session) {
            for all_provided_methods.each |provided_method| {
                debug!(
                    "(creating impl) adding provided method `%s` to impl",
                    *sess.str_of(provided_method.method_info.ident));
                vec::push(all_methods, provided_method.method_info);
            }
        }

        match item.node {
            item_impl(_, ref trait_refs, _, ref ast_methods) => {
                let mut methods = ~[];
                for ast_methods.each |ast_method| {
                    methods.push(method_to_MethodInfo(*ast_method));
                }

                // Check that we have implementations of every trait method
                for trait_refs.each |trait_ref| {
                    let trait_did =
                        self.trait_ref_to_trait_def_id(*trait_ref);
                    self.please_check_that_trait_methods_are_implemented(
                        &mut methods,
                        trait_did,
                        trait_ref.path.span);
                }

                // For each trait that the impl implements, see which
                // methods are provided.  For each of those methods,
                // if a method of that name is not inherent to the
                // impl, use the provided definition in the trait.
                for trait_refs.each |trait_ref| {
                    let trait_did =
                        self.trait_ref_to_trait_def_id(*trait_ref);

                    match self.crate_context.tcx
                              .provided_methods
                              .find(&local_def(item.id)) {
                        None => {
                            debug!("(creating impl) trait with node_id `%d` \
                                    has no provided methods", trait_did.node);
                            /* fall through */
                        }
                        Some(&all_provided) => {
                            debug!("(creating impl) trait with node_id `%d` \
                                    has provided methods", trait_did.node);
                            // Add all provided methods.
                            add_provided_methods(
                                &mut methods,
                                all_provided,
                                self.crate_context.tcx.sess);
                        }
                    }
                }

                return @Impl {
                    did: local_def(item.id),
                    ident: item.ident,
                    methods: methods
                };
            }
            _ => {
                self.crate_context.tcx.sess.span_bug(item.span,
                                                     ~"can't convert a \
                                                       non-impl to an impl");
            }
        }
    }

    fn span_of_impl(&self, implementation: @Impl) -> span {
        assert!(implementation.did.crate == local_crate);
        match self.crate_context.tcx.items.find(&implementation.did.node) {
            Some(&node_item(item, _)) => {
                return item.span;
            }
            _ => {
                self.crate_context.tcx.sess.bug(~"span_of_impl() called on \
                                                  something that wasn't an \
                                                  impl!");
            }
        }
    }

    // External crate handling

    fn add_impls_for_module(&self, impls_seen: &mut HashSet<def_id>,
                            crate_store: @mut CStore,
                            module_def_id: def_id) {
        let implementations = get_impls_for_mod(crate_store,
                                                module_def_id,
                                                None);
        for implementations.each |implementation| {
            debug!("coherence: adding impl from external crate: %s",
                   ty::item_path_str(self.crate_context.tcx,
                                     implementation.did));

            // Make sure we don't visit the same implementation
            // multiple times.
            if !impls_seen.insert(implementation.did) {
                // Skip this one.
                loop;
            }
            // Good. Continue.

            let self_type = lookup_item_type(self.crate_context.tcx,
                                             implementation.did);
            let associated_traits = get_impl_traits(self.crate_context.tcx,
                                                    implementation.did);

            // Do a sanity check to make sure that inherent methods have base
            // types.

            if associated_traits.len() == 0 {
                match get_base_type_def_id(self.inference_context,
                                           dummy_sp(),
                                           self_type.ty) {
                    None => {
                        let session = self.crate_context.tcx.sess;
                        session.bug(fmt!(
                            "no base type for external impl \
                             with no trait: %s (type %s)!",
                             *session.str_of(implementation.ident),
                             ty_to_str(self.crate_context.tcx,self_type.ty)));
                    }
                    Some(_) => {
                        // Nothing to do.
                    }
                }
            }

            // Record all the trait methods.
            for associated_traits.each |trait_ref| {
                self.add_trait_method(trait_ref.def_id, *implementation);
            }

            // Add the implementation to the mapping from
            // implementation to base type def ID, if there is a base
            // type for this implementation.

            match get_base_type_def_id(self.inference_context,
                                     dummy_sp(),
                                     self_type.ty) {
                None => {
                    // Nothing to do.
                }
                Some(base_type_def_id) => {
                    // inherent methods apply to `impl Type` but not
                    // `impl Trait for Type`:
                    if associated_traits.len() == 0 {
                        self.add_inherent_method(base_type_def_id,
                                                 *implementation);
                    }

                    self.base_type_def_ids.insert(implementation.did,
                                                  base_type_def_id);
                }
            }
        }
    }

    fn add_default_methods_for_external_trait(&self,
                                              trait_def_id: ast::def_id) {
        let tcx = self.crate_context.tcx;
        let pmm = tcx.provided_methods;

        if pmm.contains_key(&trait_def_id) { return; }

        debug!("(adding default methods for trait) processing trait");

        for csearch::get_provided_trait_methods(tcx, trait_def_id).each
                                                |trait_method_info| {
            debug!("(adding default methods for trait) found default method");

            // Create a new def ID for this provided method.
            let parse_sess = &self.crate_context.tcx.sess.parse_sess;
            let new_did = local_def(parse::next_node_id(*parse_sess));

            let provided_method_info =
                @ProvidedMethodInfo {
                    method_info: @MethodInfo {
                        did: new_did,
                        n_tps: trait_method_info.ty.generics.type_param_defs.len(),
                        ident: trait_method_info.ty.ident,
                        self_type: trait_method_info.ty.self_ty
                    },
                    trait_method_def_id: trait_method_info.def_id
                };

            pmm.insert(trait_def_id, @mut ~[provided_method_info]);
        }
    }

    // Adds implementations and traits from external crates to the coherence
    // info.
    fn add_external_crates(&self) {
        let mut impls_seen = HashSet::new();

        let crate_store = self.crate_context.tcx.sess.cstore;
        do iter_crate_data(crate_store) |crate_number, _crate_metadata| {
            self.add_impls_for_module(&mut impls_seen,
                                      crate_store,
                                      def_id { crate: crate_number,
                                               node: 0 });

            for each_path(crate_store, crate_number) |_p, def_like| {
                match def_like {
                    dl_def(def_mod(def_id)) => {
                        self.add_impls_for_module(&mut impls_seen,
                                                  crate_store,
                                                  def_id);
                    }
                    dl_def(def_trait(def_id)) => {
                        self.add_default_methods_for_external_trait(def_id);
                    }
                    dl_def(_) | dl_impl(_) | dl_field => {
                        // Skip this.
                        loop;
                    }
                }
            }
        }
    }

    //
    // Destructors
    //

    fn populate_destructor_table(&self) {
        let coherence_info = &mut self.crate_context.coherence_info;
        let tcx = self.crate_context.tcx;
        let drop_trait = tcx.lang_items.drop_trait();
        let impls_opt = coherence_info.extension_methods.find(&drop_trait);

        let impls;
        match impls_opt {
            None => return, // No types with (new-style) destructors present.
            Some(found_impls) => impls = found_impls
        }

        for impls.each |impl_info| {
            if impl_info.methods.len() < 1 {
                // We'll error out later. For now, just don't ICE.
                loop;
            }
            let method_def_id = impl_info.methods[0].did;

            let self_type = self.get_self_type_for_implementation(*impl_info);
            match ty::get(self_type.ty).sty {
                ty::ty_struct(type_def_id, _) => {
                    tcx.destructor_for_type.insert(type_def_id,
                                                   method_def_id);
                    tcx.destructors.insert(method_def_id);
                }
                _ => {
                    // Destructors only work on nominal types.
                    if impl_info.did.crate == ast::local_crate {
                        match tcx.items.find(&impl_info.did.node) {
                            Some(&ast_map::node_item(@ref item, _)) => {
                                tcx.sess.span_err((*item).span,
                                                  ~"the Drop trait may only \
                                                    be implemented on \
                                                    structures");
                            }
                            _ => {
                                tcx.sess.bug(~"didn't find impl in ast map");
                            }
                        }
                    } else {
                        tcx.sess.bug(~"found external impl of Drop trait on \
                                       something other than a struct");
                    }
                }
            }
        }
    }
}

fn subst_receiver_types_in_method_ty(
    tcx: ty::ctxt,
    impl_id: ast::node_id,
    trait_ref: &ty::TraitRef,
    new_def_id: ast::def_id,
    method: &ty::method) -> ty::method
{
    /*!
     * Substitutes the values for the receiver's type parameters
     * that are found in method, leaving the method's type parameters
     * intact.  This is in fact a mildly complex operation,
     * largely because of the hokey way that we concatenate the
     * receiver and method generics.
     */

    // determine how many type parameters were declared on the impl
    let num_impl_type_parameters = {
        let impl_polytype = ty::lookup_item_type(tcx, local_def(impl_id));
        impl_polytype.generics.type_param_defs.len()
    };

    // determine how many type parameters appear on the trait
    let num_trait_type_parameters = trait_ref.substs.tps.len();

    // the current method type has the type parameters from the trait + method
    let num_method_type_parameters =
        num_trait_type_parameters + method.generics.type_param_defs.len();

    // the new method type will have the type parameters from the impl + method
    let combined_tps = vec::from_fn(num_method_type_parameters, |i| {
        if i < num_trait_type_parameters {
            // replace type parameters that come from trait with new value
            trait_ref.substs.tps[i]
        } else {
            // replace type parameters that belong to method with another
            // type parameter, this time with the index adjusted
            let method_index = i - num_trait_type_parameters;
            let type_param_def = &method.generics.type_param_defs[method_index];
            let new_index = num_impl_type_parameters + method_index;
            ty::mk_param(tcx, new_index, type_param_def.def_id)
        }
    });

    let combined_substs = ty::substs {
        self_r: trait_ref.substs.self_r,
        self_ty: trait_ref.substs.self_ty,
        tps: combined_tps
    };

    ty::method {
        ident: method.ident,

        // method tps cannot appear in the self_ty, so use `substs` from trait ref
        transformed_self_ty: method.transformed_self_ty.subst(tcx, &trait_ref.substs),

        // method types *can* appear in the generic bounds or the fty
        generics: method.generics.subst(tcx, &combined_substs),
        fty: method.fty.subst(tcx, &combined_substs),
        self_ty: method.self_ty,
        vis: method.vis,
        def_id: new_def_id
    }
}

pub fn check_coherence(crate_context: @mut CrateCtxt, crate: @crate) {
    let coherence_checker = @CoherenceChecker(crate_context);
    coherence_checker.check_coherence(crate);
}

