use rustc::hir::def_id::DefId;
use rustc::mir;
use rustc::ty::{self, TyCtxt};

pub use self::qualifs::Qualif;

pub mod ops;
mod qualifs;
mod resolver;
pub mod validation;

/// Information about the item currently being validated, as well as a reference to the global
/// context.
pub struct Item<'mir, 'tcx> {
    body: &'mir mir::Body<'tcx>,
    tcx: TyCtxt<'tcx>,
    def_id: DefId,
    param_env: ty::ParamEnv<'tcx>,
    mode: validation::Mode,
}

impl Item<'mir, 'tcx> {
    pub fn new(
        tcx: TyCtxt<'tcx>,
        def_id: DefId,
        body: &'mir mir::Body<'tcx>,
    ) -> Self {
        let param_env = tcx.param_env(def_id);
        let mode = validation::Mode::for_item(tcx, def_id)
            .expect("const validation must only be run inside a const context");

        Item {
            body,
            tcx,
            def_id,
            param_env,
            mode,
        }
    }
}


fn is_lang_panic_fn(tcx: TyCtxt<'tcx>, def_id: DefId) -> bool {
    Some(def_id) == tcx.lang_items().panic_fn() ||
    Some(def_id) == tcx.lang_items().begin_panic_fn()
}
