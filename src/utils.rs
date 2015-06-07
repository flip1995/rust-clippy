use rustc::lint::Context;
use syntax::ast::{DefId, Name, Path};
use syntax::codemap::{ExpnInfo, Span};
use rustc::middle::ty;

/// returns true if the macro that expanded the crate was outside of
/// the current crate or was a compiler plugin
pub fn in_macro(cx: &Context, opt_info: Option<&ExpnInfo>) -> bool {
	// no ExpnInfo = no macro
	opt_info.map_or(false, |info| {
		// no span for the callee = external macro
		info.callee.span.map_or(true, |span| {
			// no snippet = external macro or compiler-builtin expansion
			cx.sess().codemap().span_to_snippet(span).ok().map_or(true, |code| 
				// macro doesn't start with "macro_rules"
				// = compiler plugin
				!code.starts_with("macro_rules")
			)
		})
	})
}

/// invokes in_macro with the expansion info of the given span
pub fn in_external_macro(cx: &Context, span: Span) -> bool {
	cx.sess().codemap().with_expn_info(span.expn_id, 
			|info| in_macro(cx, info))
}

/// check if a DefId's path matches the given absolute type path
/// usage e.g. with
/// `match_def_path(cx, id, &["core", "option", "Option"])`
pub fn match_def_path(cx: &Context, def_id: DefId, path: &[&str]) -> bool {
	ty::with_path(cx.tcx, def_id, |iter| iter.map(|elem| elem.name())
		.zip(path.iter()).all(|(nm, p)| &nm.as_str() == p))
}

/// match a Path against a slice of segment string literals, e.g.
/// `match_path(path, &["std", "rt", "begin_unwind"])`
pub fn match_path(path: &Path, segments: &[&str]) -> bool {
	path.segments.iter().rev().zip(segments.iter().rev()).all(
		|(a,b)| a.identifier.as_str() == *b)
}
