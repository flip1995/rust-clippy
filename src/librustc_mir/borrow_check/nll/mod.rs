use crate::borrow_check::borrow_set::BorrowSet;
use crate::borrow_check::location::{LocationIndex, LocationTable};
use crate::borrow_check::nll::facts::AllFactsExt;
use crate::borrow_check::nll::type_check::{MirTypeckResults, MirTypeckRegionConstraints};
use crate::borrow_check::nll::region_infer::values::RegionValueElements;
use crate::dataflow::indexes::BorrowIndex;
use crate::dataflow::move_paths::MoveData;
use crate::dataflow::FlowAtLocation;
use crate::dataflow::MaybeInitializedPlaces;
use crate::transform::MirSource;
use crate::borrow_check::Upvar;
use rustc::hir::def_id::DefId;
use rustc::infer::InferCtxt;
use rustc::mir::{ClosureOutlivesSubject, ClosureRegionRequirements, Body};
use rustc::ty::{self, RegionKind, RegionVid};
use rustc_errors::Diagnostic;
use std::fmt::Debug;
use std::env;
use std::io;
use std::path::PathBuf;
use std::rc::Rc;
use std::str::FromStr;
use syntax::symbol::sym;

use self::mir_util::PassWhere;
use polonius_engine::{Algorithm, Output};
use crate::util as mir_util;
use crate::util::pretty;

mod constraint_generation;
pub mod explain_borrow;
mod facts;
mod invalidation;
crate mod region_infer;
mod renumber;
crate mod type_check;
mod universal_regions;

mod constraints;
mod pick_constraints;

use self::facts::AllFacts;
use self::region_infer::RegionInferenceContext;
use self::universal_regions::UniversalRegions;

/// Rewrites the regions in the MIR to use NLL variables, also
/// scraping out the set of universal regions (e.g., region parameters)
/// declared on the function. That set will need to be given to
/// `compute_regions`.
pub(in crate::borrow_check) fn replace_regions_in_mir<'cx, 'tcx>(
    infcx: &InferCtxt<'cx, 'tcx>,
    def_id: DefId,
    param_env: ty::ParamEnv<'tcx>,
    body: &mut Body<'tcx>,
) -> UniversalRegions<'tcx> {
    debug!("replace_regions_in_mir(def_id={:?})", def_id);

    // Compute named region information. This also renumbers the inputs/outputs.
    let universal_regions = UniversalRegions::new(infcx, def_id, param_env);

    // Replace all remaining regions with fresh inference variables.
    renumber::renumber_mir(infcx, body);

    let source = MirSource::item(def_id);
    mir_util::dump_mir(infcx.tcx, None, "renumber", &0, source, body, |_, _| Ok(()));

    universal_regions
}

/// Computes the (non-lexical) regions from the input MIR.
///
/// This may result in errors being reported.
pub(in crate::borrow_check) fn compute_regions<'cx, 'tcx>(
    infcx: &InferCtxt<'cx, 'tcx>,
    def_id: DefId,
    universal_regions: UniversalRegions<'tcx>,
    body: &Body<'tcx>,
    upvars: &[Upvar],
    location_table: &LocationTable,
    param_env: ty::ParamEnv<'tcx>,
    flow_inits: &mut FlowAtLocation<'tcx, MaybeInitializedPlaces<'cx, 'tcx>>,
    move_data: &MoveData<'tcx>,
    borrow_set: &BorrowSet<'tcx>,
    errors_buffer: &mut Vec<Diagnostic>,
) -> (
    RegionInferenceContext<'tcx>,
    Option<Rc<Output<RegionVid, BorrowIndex, LocationIndex>>>,
    Option<ClosureRegionRequirements<'tcx>>,
) {
    let mut all_facts = if AllFacts::enabled(infcx.tcx) {
        Some(AllFacts::default())
    } else {
        None
    };

    let universal_regions = Rc::new(universal_regions);

    let elements = &Rc::new(RegionValueElements::new(body));

    // Run the MIR type-checker.
    let MirTypeckResults {
        constraints,
        universal_region_relations,
    } = type_check::type_check(
        infcx,
        param_env,
        body,
        def_id,
        &universal_regions,
        location_table,
        borrow_set,
        &mut all_facts,
        flow_inits,
        move_data,
        elements,
    );

    if let Some(all_facts) = &mut all_facts {
        all_facts
            .universal_region
            .extend(universal_regions.universal_regions());
    }

    // Create the region inference context, taking ownership of the
    // region inference data that was contained in `infcx`, and the
    // base constraints generated by the type-check.
    let var_origins = infcx.take_region_var_origins();
    let MirTypeckRegionConstraints {
        placeholder_indices,
        placeholder_index_to_region: _,
        mut liveness_constraints,
        outlives_constraints,
        closure_bounds_mapping,
        type_tests,
    } = constraints;
    let placeholder_indices = Rc::new(placeholder_indices);

    constraint_generation::generate_constraints(
        infcx,
        &mut liveness_constraints,
        &mut all_facts,
        location_table,
        &body,
        borrow_set,
    );

    let mut regioncx = RegionInferenceContext::new(
        var_origins,
        universal_regions,
        placeholder_indices,
        universal_region_relations,
        body,
        outlives_constraints,
        closure_bounds_mapping,
        type_tests,
        liveness_constraints,
        elements,
    );

    // Generate various additional constraints.
    invalidation::generate_invalidates(
        infcx.tcx,
        &mut all_facts,
        location_table,
        &body,
        borrow_set,
    );

    // Dump facts if requested.
    let polonius_output = all_facts.and_then(|all_facts| {
        if infcx.tcx.sess.opts.debugging_opts.nll_facts {
            let def_path = infcx.tcx.hir().def_path(def_id);
            let dir_path =
                PathBuf::from("nll-facts").join(def_path.to_filename_friendly_no_crate());
            all_facts.write_to_dir(dir_path, location_table).unwrap();
        }

        if infcx.tcx.sess.opts.debugging_opts.polonius {
            let algorithm = env::var("POLONIUS_ALGORITHM")
                .unwrap_or_else(|_| String::from("Hybrid"));
            let algorithm = Algorithm::from_str(&algorithm).unwrap();
            debug!("compute_regions: using polonius algorithm {:?}", algorithm);
            Some(Rc::new(Output::compute(
                &all_facts,
                algorithm,
                false,
            )))
        } else {
            None
        }
    });

    // Solve the region constraints.
    let closure_region_requirements =
        regioncx.solve(infcx, &body, upvars, def_id, errors_buffer);

    // Dump MIR results into a file, if that is enabled. This let us
    // write unit-tests, as well as helping with debugging.
    dump_mir_results(
        infcx,
        MirSource::item(def_id),
        &body,
        &regioncx,
        &closure_region_requirements,
    );

    // We also have a `#[rustc_nll]` annotation that causes us to dump
    // information
    dump_annotation(infcx, &body, def_id, &regioncx, &closure_region_requirements, errors_buffer);

    (regioncx, polonius_output, closure_region_requirements)
}

fn dump_mir_results<'a, 'tcx>(
    infcx: &InferCtxt<'a, 'tcx>,
    source: MirSource<'tcx>,
    body: &Body<'tcx>,
    regioncx: &RegionInferenceContext<'_>,
    closure_region_requirements: &Option<ClosureRegionRequirements<'_>>,
) {
    if !mir_util::dump_enabled(infcx.tcx, "nll", source) {
        return;
    }

    mir_util::dump_mir(
        infcx.tcx,
        None,
        "nll",
        &0,
        source,
        body,
        |pass_where, out| {
            match pass_where {
                // Before the CFG, dump out the values for each region variable.
                PassWhere::BeforeCFG => {
                    regioncx.dump_mir(out)?;
                    writeln!(out, "|")?;

                    if let Some(closure_region_requirements) = closure_region_requirements {
                        writeln!(out, "| Free Region Constraints")?;
                        for_each_region_constraint(closure_region_requirements, &mut |msg| {
                            writeln!(out, "| {}", msg)
                        })?;
                        writeln!(out, "|")?;
                    }
                }

                PassWhere::BeforeLocation(_) => {
                }

                PassWhere::AfterTerminator(_) => {
                }

                PassWhere::BeforeBlock(_) | PassWhere::AfterLocation(_) | PassWhere::AfterCFG => {}
            }
            Ok(())
        },
    );

    // Also dump the inference graph constraints as a graphviz file.
    let _: io::Result<()> = try {
        let mut file =
            pretty::create_dump_file(infcx.tcx, "regioncx.all.dot", None, "nll", &0, source)?;
        regioncx.dump_graphviz_raw_constraints(&mut file)?;
    };

    // Also dump the inference graph constraints as a graphviz file.
    let _: io::Result<()> = try {
        let mut file =
            pretty::create_dump_file(infcx.tcx, "regioncx.scc.dot", None, "nll", &0, source)?;
        regioncx.dump_graphviz_scc_constraints(&mut file)?;
    };
}

fn dump_annotation<'a, 'tcx>(
    infcx: &InferCtxt<'a, 'tcx>,
    body: &Body<'tcx>,
    mir_def_id: DefId,
    regioncx: &RegionInferenceContext<'tcx>,
    closure_region_requirements: &Option<ClosureRegionRequirements<'_>>,
    errors_buffer: &mut Vec<Diagnostic>,
) {
    let tcx = infcx.tcx;
    let base_def_id = tcx.closure_base_def_id(mir_def_id);
    if !tcx.has_attr(base_def_id, sym::rustc_regions) {
        return;
    }

    // When the enclosing function is tagged with `#[rustc_regions]`,
    // we dump out various bits of state as warnings. This is useful
    // for verifying that the compiler is behaving as expected.  These
    // warnings focus on the closure region requirements -- for
    // viewing the intraprocedural state, the -Zdump-mir output is
    // better.

    if let Some(closure_region_requirements) = closure_region_requirements {
        let mut err = tcx
            .sess
            .diagnostic()
            .span_note_diag(body.span, "External requirements");

        regioncx.annotate(tcx, &mut err);

        err.note(&format!(
            "number of external vids: {}",
            closure_region_requirements.num_external_vids
        ));

        // Dump the region constraints we are imposing *between* those
        // newly created variables.
        for_each_region_constraint(closure_region_requirements, &mut |msg| {
            err.note(msg);
            Ok(())
        }).unwrap();

        err.buffer(errors_buffer);
    } else {
        let mut err = tcx
            .sess
            .diagnostic()
            .span_note_diag(body.span, "No external requirements");
        regioncx.annotate(tcx, &mut err);

        err.buffer(errors_buffer);
    }
}

fn for_each_region_constraint(
    closure_region_requirements: &ClosureRegionRequirements<'_>,
    with_msg: &mut dyn FnMut(&str) -> io::Result<()>,
) -> io::Result<()> {
    for req in &closure_region_requirements.outlives_requirements {
        let subject: &dyn Debug = match &req.subject {
            ClosureOutlivesSubject::Region(subject) => subject,
            ClosureOutlivesSubject::Ty(ty) => ty,
        };
        with_msg(&format!(
            "where {:?}: {:?}",
            subject, req.outlived_free_region,
        ))?;
    }
    Ok(())
}

/// Right now, we piggy back on the `ReVar` to store our NLL inference
/// regions. These are indexed with `RegionVid`. This method will
/// assert that the region is a `ReVar` and extract its internal index.
/// This is reasonable because in our MIR we replace all universal regions
/// with inference variables.
pub trait ToRegionVid {
    fn to_region_vid(self) -> RegionVid;
}

impl<'tcx> ToRegionVid for &'tcx RegionKind {
    fn to_region_vid(self) -> RegionVid {
        if let ty::ReVar(vid) = self {
            *vid
        } else {
            bug!("region is not an ReVar: {:?}", self)
        }
    }
}

impl ToRegionVid for RegionVid {
    fn to_region_vid(self) -> RegionVid {
        self
    }
}

crate trait ConstraintDescription {
    fn description(&self) -> &'static str;
}
