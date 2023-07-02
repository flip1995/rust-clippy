//! This lint is used to collect metadata about clippy lints. This metadata is exported as a json
//! file and then used to generate the [clippy lint list](https://rust-lang.github.io/rust-clippy/master/index.html)
//!
//! This module and therefore the entire lint is guarded by a feature flag called `internal`
//!
//! The module transforms all lint names to ascii lowercase to ensure that we don't have mismatches
//! during any comparison or mapping. (Please take care of this, it's not fun to spend time on such
//! a simple mistake)

use crate::renamed_lints::RENAMED_LINTS;
use crate::utils::{
    collect_configs,
    internal_lints::lint_without_lint_pass::{extract_clippy_version_value, is_lint_ref_type},
    ClippyConfiguration,
};

use clippy_utils::diagnostics::span_lint;
use clippy_utils::ty::{match_type, walk_ptrs_ty_depth};
use clippy_utils::{last_path_segment, match_def_path, match_function_call, match_path, paths};
use if_chain::if_chain;
use itertools::Itertools;
use rustc_ast as ast;
use rustc_data_structures::fx::FxHashMap;
use rustc_hir::{
    self as hir, def::DefKind, intravisit, intravisit::Visitor, Closure, ExprKind, Item, ItemKind, Mutability, QPath,
};
use rustc_lint::{CheckLintNameResult, LateContext, LateLintPass, LintContext, LintId};
use rustc_middle::hir::nested_filter;
use rustc_session::{declare_tool_lint, impl_lint_pass};
use rustc_span::symbol::Ident;
use rustc_span::{sym, Loc, Span, Symbol};
use serde::{ser::SerializeStruct, Serialize, Serializer};
use std::collections::{BTreeSet, BinaryHeap};
use std::fmt;
use std::fmt::Write as _;
use std::fs::{self, OpenOptions};
use std::io::prelude::*;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

/// This is the json output file of the lint collector.
const JSON_OUTPUT_FILE: &str = "../util/gh-pages/lints.json";
/// This is the markdown output file of the lint collector.
const MARKDOWN_OUTPUT_FILE: &str = "../book/src/lint_configuration.md";
/// These lints are excluded from the export.
const BLACK_LISTED_LINTS: &[&str] = &["lint_author", "dump_hir", "internal_metadata_collector"];
/// These groups will be ignored by the lint group matcher. This is useful for collections like
/// `clippy::all`
const IGNORED_LINT_GROUPS: [&str; 1] = ["clippy::all"];
/// Lints within this group will be excluded from the collection. These groups
/// have to be defined without the `clippy::` prefix.
const EXCLUDED_LINT_GROUPS: [&str; 1] = ["internal"];
/// Collected deprecated lint will be assigned to this group in the JSON output
const DEPRECATED_LINT_GROUP_STR: &str = "deprecated";
/// This is the lint level for deprecated lints that will be displayed in the lint list
const DEPRECATED_LINT_LEVEL: &str = "none";
/// This array holds Clippy's lint groups with their corresponding default lint level. The
/// lint level for deprecated lints is set in `DEPRECATED_LINT_LEVEL`.
const DEFAULT_LINT_LEVELS: &[(&str, &str)] = &[
    ("correctness", "deny"),
    ("suspicious", "warn"),
    ("restriction", "allow"),
    ("style", "warn"),
    ("pedantic", "allow"),
    ("complexity", "warn"),
    ("perf", "warn"),
    ("cargo", "allow"),
    ("nursery", "allow"),
];
/// This prefix is in front of the lint groups in the lint store. The prefix will be trimmed
/// to only keep the actual lint group in the output.
const CLIPPY_LINT_GROUP_PREFIX: &str = "clippy::";
const LINT_EMISSION_FUNCTIONS: [&[&str]; 7] = [
    &["clippy_utils", "diagnostics", "span_lint"],
    &["clippy_utils", "diagnostics", "span_lint_and_help"],
    &["clippy_utils", "diagnostics", "span_lint_and_note"],
    &["clippy_utils", "diagnostics", "span_lint_hir"],
    &["clippy_utils", "diagnostics", "span_lint_and_sugg"],
    &["clippy_utils", "diagnostics", "span_lint_and_then"],
    &["clippy_utils", "diagnostics", "span_lint_hir_and_then"],
];
const SUGGESTION_DIAGNOSTIC_BUILDER_METHODS: [(&str, bool); 9] = [
    ("span_suggestion", false),
    ("span_suggestion_short", false),
    ("span_suggestion_verbose", false),
    ("span_suggestion_hidden", false),
    ("tool_only_span_suggestion", false),
    ("multipart_suggestion", true),
    ("multipart_suggestions", true),
    ("tool_only_multipart_suggestion", true),
    ("span_suggestions", true),
];
const SUGGESTION_FUNCTIONS: [&[&str]; 2] = [
    &["clippy_utils", "diagnostics", "multispan_sugg"],
    &["clippy_utils", "diagnostics", "multispan_sugg_with_applicability"],
];
const DEPRECATED_LINT_TYPE: [&str; 3] = ["clippy_lints", "deprecated_lints", "ClippyDeprecatedLint"];

/// The index of the applicability name of `paths::APPLICABILITY_VALUES`
const APPLICABILITY_NAME_INDEX: usize = 2;
/// This applicability will be set for unresolved applicability values.
const APPLICABILITY_UNRESOLVED_STR: &str = "Unresolved";
/// The version that will be displayed if none has been defined
const VERSION_DEFAULT_STR: &str = "Unknown";

const CHANGELOG_PATH: &str = "../CHANGELOG.md";

declare_clippy_lint! {
    /// ### What it does
    /// Collects metadata about clippy lints for the website.
    ///
    /// This lint will be used to report problems of syntax parsing. You should hopefully never
    /// see this but never say never I guess ^^
    ///
    /// ### Why is this bad?
    /// This is not a bad thing but definitely a hacky way to do it. See
    /// issue [#4310](https://github.com/rust-lang/rust-clippy/issues/4310) for a discussion
    /// about the implementation.
    ///
    /// ### Known problems
    /// Hopefully none. It would be pretty uncool to have a problem here :)
    ///
    /// ### Example output
    /// ```json,ignore
    /// {
    ///     "id": "internal_metadata_collector",
    ///     "id_span": {
    ///         "path": "clippy_lints/src/utils/internal_lints/metadata_collector.rs",
    ///         "line": 1
    ///     },
    ///     "group": "clippy::internal",
    ///     "docs": " ### What it does\nCollects metadata about clippy lints for the website. [...] "
    /// }
    /// ```
    #[clippy::version = "1.56.0"]
    pub INTERNAL_METADATA_COLLECTOR,
    internal_warn,
    "A busy bee collection metadata about lints"
}

impl_lint_pass!(MetadataCollector => [INTERNAL_METADATA_COLLECTOR]);

#[allow(clippy::module_name_repetitions)]
#[derive(Debug, Clone)]
pub struct MetadataCollector {
    /// All collected lints
    ///
    /// We use a Heap here to have the lints added in alphabetic order in the export
    lints: BinaryHeap<LintMetadata>,
    applicability_info: FxHashMap<String, ApplicabilityInfo>,
    config: Vec<ClippyConfiguration>,
    clippy_project_root: PathBuf,
}

impl MetadataCollector {
    pub fn new() -> Self {
        Self {
            lints: BinaryHeap::<LintMetadata>::default(),
            applicability_info: FxHashMap::<String, ApplicabilityInfo>::default(),
            config: collect_configs(),
            clippy_project_root: std::env::current_dir()
                .expect("failed to get current dir")
                .ancestors()
                .nth(1)
                .expect("failed to get project root")
                .to_path_buf(),
        }
    }

    fn get_lint_configs(&self, lint_name: &str) -> Option<String> {
        self.config
            .iter()
            .filter(|config| config.lints.iter().any(|lint| lint == lint_name))
            .map(ToString::to_string)
            .reduce(|acc, x| acc + &x)
            .map(|configurations| {
                format!(
                    r#"
### Configuration
This lint has the following configuration variables:

{configurations}
"#
                )
            })
    }

    fn configs_to_markdown(&self, map_fn: fn(&ClippyConfiguration) -> String) -> String {
        self.config
            .iter()
            .filter(|config| config.deprecation_reason.is_none())
            .filter(|config| !config.lints.is_empty())
            .map(map_fn)
            .join("\n")
    }

    fn get_markdown_docs(&self) -> String {
        format!(
            r#"# Lint Configuration Options

The following list shows each configuration option, along with a description, its default value, an example
and lints affected.

---

{}"#,
            self.configs_to_markdown(ClippyConfiguration::to_markdown_paragraph),
        )
    }
}

impl Drop for MetadataCollector {
    /// You might ask: How hacky is this?
    /// My answer:     YES
    fn drop(&mut self) {
        // The metadata collector gets dropped twice, this makes sure that we only write
        // when the list is full
        if self.lints.is_empty() {
            return;
        }

        let mut applicability_info = std::mem::take(&mut self.applicability_info);

        // Mapping the final data
        let mut lints = std::mem::take(&mut self.lints).into_sorted_vec();
        for x in &mut lints {
            x.applicability = Some(applicability_info.remove(&x.id).unwrap_or_default());
            replace_produces(&x.id, &mut x.docs, &self.clippy_project_root);
        }

        collect_renames(&mut lints);

        // Outputting json
        if Path::new(JSON_OUTPUT_FILE).exists() {
            fs::remove_file(JSON_OUTPUT_FILE).unwrap();
        }
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .open(JSON_OUTPUT_FILE)
            .unwrap();
        writeln!(file, "{}", serde_json::to_string_pretty(&lints).unwrap()).unwrap();

        // Outputting markdown
        if Path::new(MARKDOWN_OUTPUT_FILE).exists() {
            fs::remove_file(MARKDOWN_OUTPUT_FILE).unwrap();
        }
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .open(MARKDOWN_OUTPUT_FILE)
            .unwrap();
        writeln!(
            file,
            "<!--
This file is generated by `cargo collect-metadata`.
Please use that command to update the file and do not edit it by hand.
-->

{}",
            self.get_markdown_docs(),
        )
        .unwrap();

        // Write configuration links to CHANGELOG.md
        let mut changelog = std::fs::read_to_string(CHANGELOG_PATH).unwrap();
        let mut changelog_file = OpenOptions::new().read(true).write(true).open(CHANGELOG_PATH).unwrap();

        if let Some(position) = changelog.find("<!-- begin autogenerated links to configuration documentation -->") {
            // I know this is kinda wasteful, we just don't have regex on `clippy_lints` so... this is the best
            // we can do AFAIK.
            changelog = changelog[..position].to_string();
        }
        writeln!(
            changelog_file,
            "{changelog}<!-- begin autogenerated links to configuration documentation -->\n{}\n<!-- end autogenerated links to configuration documentation -->",
            self.configs_to_markdown(ClippyConfiguration::to_markdown_link)
        )
        .unwrap();
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq, PartialOrd, Ord)]
struct LintMetadata {
    id: String,
    id_span: SerializableSpan,
    group: String,
    level: String,
    docs: String,
    version: String,
    /// This field is only used in the output and will only be
    /// mapped shortly before the actual output.
    applicability: Option<ApplicabilityInfo>,
    /// All the past names of lints which have been renamed.
    #[serde(skip_serializing_if = "BTreeSet::is_empty")]
    former_ids: BTreeSet<String>,
}

impl LintMetadata {
    fn new(
        id: String,
        id_span: SerializableSpan,
        group: String,
        level: &'static str,
        version: String,
        docs: String,
    ) -> Self {
        Self {
            id,
            id_span,
            group,
            level: level.to_string(),
            version,
            docs,
            applicability: None,
            former_ids: BTreeSet::new(),
        }
    }
}

fn replace_produces(lint_name: &str, docs: &mut String, clippy_project_root: &Path) {
    let mut doc_lines = docs.lines().map(ToString::to_string).collect::<Vec<_>>();
    let mut lines = doc_lines.iter_mut();

    'outer: loop {
        // Find the start of the example

        // ```rust
        loop {
            match lines.next() {
                Some(line) if line.trim_start().starts_with("```rust") => {
                    if line.contains("ignore") || line.contains("no_run") {
                        // A {{produces}} marker may have been put on a ignored code block by mistake,
                        // just seek to the end of the code block and continue checking.
                        if lines.any(|line| line.trim_start().starts_with("```")) {
                            continue;
                        }

                        panic!("lint `{lint_name}` has an unterminated code block")
                    }

                    break;
                },
                Some(line) if line.trim_start() == "{{produces}}" => {
                    panic!("lint `{lint_name}` has marker {{{{produces}}}} with an ignored or missing code block")
                },
                Some(line) => {
                    let line = line.trim();
                    // These are the two most common markers of the corrections section
                    if line.eq_ignore_ascii_case("Use instead:") || line.eq_ignore_ascii_case("Could be written as:") {
                        break 'outer;
                    }
                },
                None => break 'outer,
            }
        }

        // Collect the example
        let mut example = Vec::new();
        loop {
            match lines.next() {
                Some(line) if line.trim_start() == "```" => break,
                Some(line) => example.push(line),
                None => panic!("lint `{lint_name}` has an unterminated code block"),
            }
        }

        // Find the {{produces}} and attempt to generate the output
        loop {
            match lines.next() {
                Some(line) if line.is_empty() => {},
                Some(line) if line.trim() == "{{produces}}" => {
                    let output = get_lint_output(lint_name, &example, clippy_project_root);
                    line.replace_range(
                        ..,
                        &format!(
                            "<details>\
                            <summary>Produces</summary>\n\
                            \n\
                            ```text\n\
                            {output}\n\
                            ```\n\
                        </details>"
                        ),
                    );

                    break;
                },
                // No {{produces}}, we can move on to the next example
                Some(_) => break,
                None => break 'outer,
            }
        }
    }

    *docs = cleanup_docs(&doc_lines);
}

fn get_lint_output(lint_name: &str, example: &[&mut String], clippy_project_root: &Path) -> String {
    let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("failed to create temp dir: {e}"));
    let file = dir.path().join("lint_example.rs");

    let mut source = String::new();
    let unhidden = example
        .iter()
        .map(|line| line.trim_start().strip_prefix("# ").unwrap_or(line));

    // Get any attributes
    let mut lines = unhidden.peekable();
    while let Some(line) = lines.peek() {
        if line.starts_with("#!") {
            source.push_str(line);
            source.push('\n');
            lines.next();
        } else {
            break;
        }
    }

    let needs_main = !example.iter().any(|line| line.contains("fn main"));
    if needs_main {
        source.push_str("fn main() {\n");
    }

    for line in lines {
        source.push_str(line);
        source.push('\n');
    }

    if needs_main {
        source.push_str("}\n");
    }

    if let Err(e) = fs::write(&file, &source) {
        panic!("failed to write to `{}`: {e}", file.as_path().to_string_lossy());
    }

    let prefixed_name = format!("{CLIPPY_LINT_GROUP_PREFIX}{lint_name}");

    let mut cmd = Command::new("cargo");

    cmd.current_dir(clippy_project_root)
        .env("CARGO_INCREMENTAL", "0")
        .env("CLIPPY_ARGS", "")
        .env("CLIPPY_DISABLE_DOCS_LINKS", "1")
        // We need to disable this to enable all lints
        .env("ENABLE_METADATA_COLLECTION", "0")
        .args(["run", "--bin", "clippy-driver"])
        .args(["--target-dir", "./clippy_lints/target"])
        .args(["--", "--error-format=json"])
        .args(["--edition", "2021"])
        .arg("-Cdebuginfo=0")
        .args(["-A", "clippy::all"])
        .args(["-W", &prefixed_name])
        .args(["-L", "./target/debug"])
        .args(["-Z", "no-codegen"]);

    let output = cmd
        .arg(file.as_path())
        .output()
        .unwrap_or_else(|e| panic!("failed to run `{cmd:?}`: {e}"));

    let tmp_file_path = file.to_string_lossy();
    let stderr = std::str::from_utf8(&output.stderr).unwrap();
    let msgs = stderr
        .lines()
        .filter(|line| line.starts_with('{'))
        .map(|line| serde_json::from_str(line).unwrap())
        .collect::<Vec<serde_json::Value>>();

    let mut rendered = String::new();
    let iter = msgs
        .iter()
        .filter(|msg| matches!(&msg["code"]["code"], serde_json::Value::String(s) if s == &prefixed_name));

    for message in iter {
        let rendered_part = message["rendered"].as_str().expect("rendered field should exist");
        rendered.push_str(rendered_part);
    }

    if rendered.is_empty() {
        let rendered: Vec<&str> = msgs.iter().filter_map(|msg| msg["rendered"].as_str()).collect();
        let non_json: Vec<&str> = stderr.lines().filter(|line| !line.starts_with('{')).collect();
        panic!(
            "did not find lint `{lint_name}` in output of example, got:\n{}\n{}",
            non_json.join("\n"),
            rendered.join("\n")
        );
    }

    // The reader doesn't need to see `/tmp/.tmpfiy2Qd/lint_example.rs` :)
    rendered.trim_end().replace(&*tmp_file_path, "lint_example.rs")
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq, PartialOrd, Ord)]
struct SerializableSpan {
    path: String,
    line: usize,
}

impl fmt::Display for SerializableSpan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.path.rsplit('/').next().unwrap_or_default(), self.line)
    }
}

impl SerializableSpan {
    fn from_item(cx: &LateContext<'_>, item: &Item<'_>) -> Self {
        Self::from_span(cx, item.ident.span)
    }

    fn from_span(cx: &LateContext<'_>, span: Span) -> Self {
        let loc: Loc = cx.sess().source_map().lookup_char_pos(span.lo());

        Self {
            path: format!("{}", loc.file.name.prefer_remapped()),
            line: loc.line,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord)]
struct ApplicabilityInfo {
    /// Indicates if any of the lint emissions uses multiple spans. This is related to
    /// [rustfix#141](https://github.com/rust-lang/rustfix/issues/141) as such suggestions can
    /// currently not be applied automatically.
    is_multi_part_suggestion: bool,
    applicability: Option<usize>,
}

impl Serialize for ApplicabilityInfo {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut s = serializer.serialize_struct("ApplicabilityInfo", 2)?;
        s.serialize_field("is_multi_part_suggestion", &self.is_multi_part_suggestion)?;
        if let Some(index) = self.applicability {
            s.serialize_field(
                "applicability",
                &paths::APPLICABILITY_VALUES[index][APPLICABILITY_NAME_INDEX],
            )?;
        } else {
            s.serialize_field("applicability", APPLICABILITY_UNRESOLVED_STR)?;
        }
        s.end()
    }
}

impl fmt::Display for ClippyConfiguration {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "* `{}`: `{}`(defaults to `{}`): {}",
            self.name, self.config_type, self.default, self.doc
        )
    }
}

// ==================================================================
// Lint pass
// ==================================================================
impl<'hir> LateLintPass<'hir> for MetadataCollector {
    /// Collecting lint declarations like:
    /// ```rust, ignore
    /// declare_clippy_lint! {
    ///     /// ### What it does
    ///     /// Something IDK.
    ///     pub SOME_LINT,
    ///     internal,
    ///     "Who am I?"
    /// }
    /// ```
    fn check_item(&mut self, cx: &LateContext<'hir>, item: &'hir Item<'_>) {
        if let ItemKind::Static(ty, Mutability::Not, _) = item.kind {
            // Normal lint
            if_chain! {
                // item validation
                if is_lint_ref_type(cx, ty);
                // disallow check
                let lint_name = sym_to_string(item.ident.name).to_ascii_lowercase();
                if !BLACK_LISTED_LINTS.contains(&lint_name.as_str());
                // metadata extraction
                if let Some((group, level)) = get_lint_group_and_level_or_lint(cx, &lint_name, item);
                if let Some(mut raw_docs) = extract_attr_docs_or_lint(cx, item);
                then {
                    if let Some(configuration_section) = self.get_lint_configs(&lint_name) {
                        raw_docs.push_str(&configuration_section);
                    }
                    let version = get_lint_version(cx, item);

                    self.lints.push(LintMetadata::new(
                        lint_name,
                        SerializableSpan::from_item(cx, item),
                        group,
                        level,
                        version,
                        raw_docs,
                    ));
                }
            }

            if_chain! {
                if is_deprecated_lint(cx, ty);
                // disallow check
                let lint_name = sym_to_string(item.ident.name).to_ascii_lowercase();
                if !BLACK_LISTED_LINTS.contains(&lint_name.as_str());
                // Metadata the little we can get from a deprecated lint
                if let Some(raw_docs) = extract_attr_docs_or_lint(cx, item);
                then {
                    let version = get_lint_version(cx, item);

                    self.lints.push(LintMetadata::new(
                        lint_name,
                        SerializableSpan::from_item(cx, item),
                        DEPRECATED_LINT_GROUP_STR.to_string(),
                        DEPRECATED_LINT_LEVEL,
                        version,
                        raw_docs,
                    ));
                }
            }
        }
    }

    /// Collecting constant applicability from the actual lint emissions
    ///
    /// Example:
    /// ```rust, ignore
    /// span_lint_and_sugg(
    ///     cx,
    ///     SOME_LINT,
    ///     item.span,
    ///     "Le lint message",
    ///     "Here comes help:",
    ///     "#![allow(clippy::all)]",
    ///     Applicability::MachineApplicable, // <-- Extracts this constant value
    /// );
    /// ```
    fn check_expr(&mut self, cx: &LateContext<'hir>, expr: &'hir hir::Expr<'_>) {
        if let Some(args) = match_lint_emission(cx, expr) {
            let emission_info = extract_emission_info(cx, args);
            if emission_info.is_empty() {
                // See:
                // - src/misc.rs:734:9
                // - src/methods/mod.rs:3545:13
                // - src/methods/mod.rs:3496:13
                // We are basically unable to resolve the lint name itself.
                return;
            }

            for (lint_name, applicability, is_multi_part) in emission_info {
                let app_info = self.applicability_info.entry(lint_name).or_default();
                app_info.applicability = applicability;
                app_info.is_multi_part_suggestion = is_multi_part;
            }
        }
    }
}

// ==================================================================
// Lint definition extraction
// ==================================================================
fn sym_to_string(sym: Symbol) -> String {
    sym.as_str().to_string()
}

fn extract_attr_docs_or_lint(cx: &LateContext<'_>, item: &Item<'_>) -> Option<String> {
    extract_attr_docs(cx, item).or_else(|| {
        lint_collection_error_item(cx, item, "could not collect the lint documentation");
        None
    })
}

/// This function collects all documentation that has been added to an item using
/// `#[doc = r""]` attributes. Several attributes are aggravated using line breaks
///
/// ```ignore
/// #[doc = r"Hello world!"]
/// #[doc = r"=^.^="]
/// struct SomeItem {}
/// ```
///
/// Would result in `Hello world!\n=^.^=\n`
fn extract_attr_docs(cx: &LateContext<'_>, item: &Item<'_>) -> Option<String> {
    let attrs = cx.tcx.hir().attrs(item.hir_id());
    let mut lines = attrs.iter().filter_map(ast::Attribute::doc_str);

    if let Some(line) = lines.next() {
        let raw_docs = lines.fold(String::from(line.as_str()) + "\n", |s, line| s + line.as_str() + "\n");
        return Some(raw_docs);
    }

    None
}

/// This function may modify the doc comment to ensure that the string can be displayed using a
/// markdown viewer in Clippy's lint list. The following modifications could be applied:
/// * Removal of leading space after a new line. (Important to display tables)
/// * Ensures that code blocks only contain language information
fn cleanup_docs(docs_collection: &Vec<String>) -> String {
    let mut in_code_block = false;
    let mut is_code_block_rust = false;

    let mut docs = String::new();
    for line in docs_collection {
        // Rustdoc hides code lines starting with `# ` and this removes them from Clippy's lint list :)
        if is_code_block_rust && line.trim_start().starts_with("# ") {
            continue;
        }

        // The line should be represented in the lint list, even if it's just an empty line
        docs.push('\n');
        if let Some(info) = line.trim_start().strip_prefix("```") {
            in_code_block = !in_code_block;
            is_code_block_rust = false;
            if in_code_block {
                let lang = info
                    .trim()
                    .split(',')
                    // remove rustdoc directives
                    .find(|&s| !matches!(s, "" | "ignore" | "no_run" | "should_panic"))
                    // if no language is present, fill in "rust"
                    .unwrap_or("rust");
                docs.push_str("```");
                docs.push_str(lang);

                is_code_block_rust = lang == "rust";
                continue;
            }
        }
        // This removes the leading space that the macro translation introduces
        if let Some(stripped_doc) = line.strip_prefix(' ') {
            docs.push_str(stripped_doc);
        } else if !line.is_empty() {
            docs.push_str(line);
        }
    }

    docs
}

fn get_lint_version(cx: &LateContext<'_>, item: &Item<'_>) -> String {
    extract_clippy_version_value(cx, item).map_or_else(
        || VERSION_DEFAULT_STR.to_string(),
        |version| version.as_str().to_string(),
    )
}

fn get_lint_group_and_level_or_lint(
    cx: &LateContext<'_>,
    lint_name: &str,
    item: &Item<'_>,
) -> Option<(String, &'static str)> {
    let result = cx.lint_store.check_lint_name(
        lint_name,
        Some(sym::clippy),
        &std::iter::once(Ident::with_dummy_span(sym::clippy)).collect(),
    );
    if let CheckLintNameResult::Tool(Ok(lint_lst)) = result {
        if let Some(group) = get_lint_group(cx, lint_lst[0]) {
            if EXCLUDED_LINT_GROUPS.contains(&group.as_str()) {
                return None;
            }

            if let Some(level) = get_lint_level_from_group(&group) {
                Some((group, level))
            } else {
                lint_collection_error_item(
                    cx,
                    item,
                    &format!("Unable to determine lint level for found group `{group}`"),
                );
                None
            }
        } else {
            lint_collection_error_item(cx, item, "Unable to determine lint group");
            None
        }
    } else {
        lint_collection_error_item(cx, item, "Unable to find lint in lint_store");
        None
    }
}

fn get_lint_group(cx: &LateContext<'_>, lint_id: LintId) -> Option<String> {
    for (group_name, lints, _) in cx.lint_store.get_lint_groups() {
        if IGNORED_LINT_GROUPS.contains(&group_name) {
            continue;
        }

        if lints.iter().any(|group_lint| *group_lint == lint_id) {
            let group = group_name.strip_prefix(CLIPPY_LINT_GROUP_PREFIX).unwrap_or(group_name);
            return Some((*group).to_string());
        }
    }

    None
}

fn get_lint_level_from_group(lint_group: &str) -> Option<&'static str> {
    DEFAULT_LINT_LEVELS
        .iter()
        .find_map(|(group_name, group_level)| (*group_name == lint_group).then_some(*group_level))
}

pub(super) fn is_deprecated_lint(cx: &LateContext<'_>, ty: &hir::Ty<'_>) -> bool {
    if let hir::TyKind::Path(ref path) = ty.kind {
        if let hir::def::Res::Def(DefKind::Struct, def_id) = cx.qpath_res(path, ty.hir_id) {
            return match_def_path(cx, def_id, &DEPRECATED_LINT_TYPE);
        }
    }

    false
}

fn collect_renames(lints: &mut Vec<LintMetadata>) {
    for lint in lints {
        let mut collected = String::new();
        let mut names = vec![lint.id.clone()];

        loop {
            if let Some(lint_name) = names.pop() {
                for (k, v) in RENAMED_LINTS {
                    if_chain! {
                        if let Some(name) = v.strip_prefix(CLIPPY_LINT_GROUP_PREFIX);
                        if name == lint_name;
                        if let Some(past_name) = k.strip_prefix(CLIPPY_LINT_GROUP_PREFIX);
                        then {
                            lint.former_ids.insert(past_name.to_owned());
                            writeln!(collected, "* `{past_name}`").unwrap();
                            names.push(past_name.to_string());
                        }
                    }
                }

                continue;
            }

            break;
        }

        if !collected.is_empty() {
            write!(
                &mut lint.docs,
                r#"
### Past names

{collected}
"#
            )
            .unwrap();
        }
    }
}

// ==================================================================
// Lint emission
// ==================================================================
fn lint_collection_error_item(cx: &LateContext<'_>, item: &Item<'_>, message: &str) {
    span_lint(
        cx,
        INTERNAL_METADATA_COLLECTOR,
        item.ident.span,
        &format!("metadata collection error for `{}`: {message}", item.ident.name),
    );
}

// ==================================================================
// Applicability
// ==================================================================
/// This function checks if a given expression is equal to a simple lint emission function call.
/// It will return the function arguments if the emission matched any function.
fn match_lint_emission<'hir>(cx: &LateContext<'hir>, expr: &'hir hir::Expr<'_>) -> Option<&'hir [hir::Expr<'hir>]> {
    LINT_EMISSION_FUNCTIONS
        .iter()
        .find_map(|emission_fn| match_function_call(cx, expr, emission_fn))
}

fn take_higher_applicability(a: Option<usize>, b: Option<usize>) -> Option<usize> {
    a.map_or(b, |a| a.max(b.unwrap_or_default()).into())
}

fn extract_emission_info<'hir>(
    cx: &LateContext<'hir>,
    args: &'hir [hir::Expr<'hir>],
) -> Vec<(String, Option<usize>, bool)> {
    let mut lints = Vec::new();
    let mut applicability = None;
    let mut multi_part = false;

    for arg in args {
        let (arg_ty, _) = walk_ptrs_ty_depth(cx.typeck_results().expr_ty(arg));

        if match_type(cx, arg_ty, &paths::LINT) {
            // If we found the lint arg, extract the lint name
            let mut resolved_lints = resolve_lints(cx, arg);
            lints.append(&mut resolved_lints);
        } else if match_type(cx, arg_ty, &paths::APPLICABILITY) {
            applicability = resolve_applicability(cx, arg);
        } else if arg_ty.is_closure() {
            multi_part |= check_is_multi_part(cx, arg);
            applicability = applicability.or_else(|| resolve_applicability(cx, arg));
        }
    }

    lints
        .into_iter()
        .map(|lint_name| (lint_name, applicability, multi_part))
        .collect()
}

/// Resolves the possible lints that this expression could reference
fn resolve_lints<'hir>(cx: &LateContext<'hir>, expr: &'hir hir::Expr<'hir>) -> Vec<String> {
    let mut resolver = LintResolver::new(cx);
    resolver.visit_expr(expr);
    resolver.lints
}

/// This function tries to resolve the linked applicability to the given expression.
fn resolve_applicability<'hir>(cx: &LateContext<'hir>, expr: &'hir hir::Expr<'hir>) -> Option<usize> {
    let mut resolver = ApplicabilityResolver::new(cx);
    resolver.visit_expr(expr);
    resolver.complete()
}

fn check_is_multi_part<'hir>(cx: &LateContext<'hir>, closure_expr: &'hir hir::Expr<'hir>) -> bool {
    if let ExprKind::Closure(&Closure { body, .. }) = closure_expr.kind {
        let mut scanner = IsMultiSpanScanner::new(cx);
        intravisit::walk_body(&mut scanner, cx.tcx.hir().body(body));
        return scanner.is_multi_part();
    } else if let Some(local) = get_parent_local(cx, closure_expr) {
        if let Some(local_init) = local.init {
            return check_is_multi_part(cx, local_init);
        }
    }

    false
}

struct LintResolver<'a, 'hir> {
    cx: &'a LateContext<'hir>,
    lints: Vec<String>,
}

impl<'a, 'hir> LintResolver<'a, 'hir> {
    fn new(cx: &'a LateContext<'hir>) -> Self {
        Self {
            cx,
            lints: Vec::<String>::default(),
        }
    }
}

impl<'a, 'hir> intravisit::Visitor<'hir> for LintResolver<'a, 'hir> {
    type NestedFilter = nested_filter::All;

    fn nested_visit_map(&mut self) -> Self::Map {
        self.cx.tcx.hir()
    }

    fn visit_expr(&mut self, expr: &'hir hir::Expr<'hir>) {
        if_chain! {
            if let ExprKind::Path(qpath) = &expr.kind;
            if let QPath::Resolved(_, path) = qpath;

            let (expr_ty, _) = walk_ptrs_ty_depth(self.cx.typeck_results().expr_ty(expr));
            if match_type(self.cx, expr_ty, &paths::LINT);
            then {
                if let hir::def::Res::Def(DefKind::Static(..), _) = path.res {
                    let lint_name = last_path_segment(qpath).ident.name;
                    self.lints.push(sym_to_string(lint_name).to_ascii_lowercase());
                } else if let Some(local) = get_parent_local(self.cx, expr) {
                    if let Some(local_init) = local.init {
                        intravisit::walk_expr(self, local_init);
                    }
                }
            }
        }

        intravisit::walk_expr(self, expr);
    }
}

/// This visitor finds the highest applicability value in the visited expressions
struct ApplicabilityResolver<'a, 'hir> {
    cx: &'a LateContext<'hir>,
    /// This is the index of highest `Applicability` for `paths::APPLICABILITY_VALUES`
    applicability_index: Option<usize>,
}

impl<'a, 'hir> ApplicabilityResolver<'a, 'hir> {
    fn new(cx: &'a LateContext<'hir>) -> Self {
        Self {
            cx,
            applicability_index: None,
        }
    }

    fn add_new_index(&mut self, new_index: usize) {
        self.applicability_index = take_higher_applicability(self.applicability_index, Some(new_index));
    }

    fn complete(self) -> Option<usize> {
        self.applicability_index
    }
}

impl<'a, 'hir> intravisit::Visitor<'hir> for ApplicabilityResolver<'a, 'hir> {
    type NestedFilter = nested_filter::All;

    fn nested_visit_map(&mut self) -> Self::Map {
        self.cx.tcx.hir()
    }

    fn visit_path(&mut self, path: &hir::Path<'hir>, _id: hir::HirId) {
        for (index, enum_value) in paths::APPLICABILITY_VALUES.iter().enumerate() {
            if match_path(path, enum_value) {
                self.add_new_index(index);
                return;
            }
        }
    }

    fn visit_expr(&mut self, expr: &'hir hir::Expr<'hir>) {
        let (expr_ty, _) = walk_ptrs_ty_depth(self.cx.typeck_results().expr_ty(expr));

        if_chain! {
            if match_type(self.cx, expr_ty, &paths::APPLICABILITY);
            if let Some(local) = get_parent_local(self.cx, expr);
            if let Some(local_init) = local.init;
            then {
                intravisit::walk_expr(self, local_init);
            }
        };

        intravisit::walk_expr(self, expr);
    }
}

/// This returns the parent local node if the expression is a reference one
fn get_parent_local<'hir>(cx: &LateContext<'hir>, expr: &'hir hir::Expr<'hir>) -> Option<&'hir hir::Local<'hir>> {
    if let ExprKind::Path(QPath::Resolved(_, path)) = expr.kind {
        if let hir::def::Res::Local(local_hir) = path.res {
            return get_parent_local_hir_id(cx, local_hir);
        }
    }

    None
}

fn get_parent_local_hir_id<'hir>(cx: &LateContext<'hir>, hir_id: hir::HirId) -> Option<&'hir hir::Local<'hir>> {
    let map = cx.tcx.hir();

    match map.find_parent(hir_id) {
        Some(hir::Node::Local(local)) => Some(local),
        Some(hir::Node::Pat(pattern)) => get_parent_local_hir_id(cx, pattern.hir_id),
        _ => None,
    }
}

/// This visitor finds the highest applicability value in the visited expressions
struct IsMultiSpanScanner<'a, 'hir> {
    cx: &'a LateContext<'hir>,
    suggestion_count: usize,
}

impl<'a, 'hir> IsMultiSpanScanner<'a, 'hir> {
    fn new(cx: &'a LateContext<'hir>) -> Self {
        Self {
            cx,
            suggestion_count: 0,
        }
    }

    /// Add a new single expression suggestion to the counter
    fn add_single_span_suggestion(&mut self) {
        self.suggestion_count += 1;
    }

    /// Signals that a suggestion with possible multiple spans was found
    fn add_multi_part_suggestion(&mut self) {
        self.suggestion_count += 2;
    }

    /// Checks if the suggestions include multiple spans
    fn is_multi_part(&self) -> bool {
        self.suggestion_count > 1
    }
}

impl<'a, 'hir> intravisit::Visitor<'hir> for IsMultiSpanScanner<'a, 'hir> {
    type NestedFilter = nested_filter::All;

    fn nested_visit_map(&mut self) -> Self::Map {
        self.cx.tcx.hir()
    }

    fn visit_expr(&mut self, expr: &'hir hir::Expr<'hir>) {
        // Early return if the lint is already multi span
        if self.is_multi_part() {
            return;
        }

        match &expr.kind {
            ExprKind::Call(fn_expr, _args) => {
                let found_function = SUGGESTION_FUNCTIONS
                    .iter()
                    .any(|func_path| match_function_call(self.cx, fn_expr, func_path).is_some());
                if found_function {
                    // These functions are all multi part suggestions
                    self.add_single_span_suggestion();
                }
            },
            ExprKind::MethodCall(path, recv, _, _arg_span) => {
                let (self_ty, _) = walk_ptrs_ty_depth(self.cx.typeck_results().expr_ty(recv));
                if match_type(self.cx, self_ty, &paths::DIAGNOSTIC_BUILDER) {
                    let called_method = path.ident.name.as_str().to_string();
                    for (method_name, is_multi_part) in &SUGGESTION_DIAGNOSTIC_BUILDER_METHODS {
                        if *method_name == called_method {
                            if *is_multi_part {
                                self.add_multi_part_suggestion();
                            } else {
                                self.add_single_span_suggestion();
                            }
                            break;
                        }
                    }
                }
            },
            _ => {},
        }

        intravisit::walk_expr(self, expr);
    }
}
