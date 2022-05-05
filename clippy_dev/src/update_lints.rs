use aho_corasick::AhoCorasickBuilder;
use core::fmt::Write as _;
use itertools::Itertools;
use rustc_lexer::{tokenize, unescape, LiteralKind, TokenKind};
use std::collections::{HashMap, HashSet};
use std::ffi::OsStr;
use std::fs;
use std::io::{self, Read as _, Seek as _, Write as _};
use std::path::{Path, PathBuf};
use walkdir::{DirEntry, WalkDir};

use crate::clippy_project_root;

const GENERATED_FILE_COMMENT: &str = "// This file was generated by `cargo dev update_lints`.\n\
     // Use that command to update this file and do not edit by hand.\n\
     // Manual edits will be overwritten.\n\n";

const DOCS_LINK: &str = "https://rust-lang.github.io/rust-clippy/master/index.html";

#[derive(Clone, Copy, PartialEq)]
pub enum UpdateMode {
    Check,
    Change,
}

/// Runs the `update_lints` command.
///
/// This updates various generated values from the lint source code.
///
/// `update_mode` indicates if the files should be updated or if updates should be checked for.
///
/// # Panics
///
/// Panics if a file path could not read from or then written to
pub fn update(update_mode: UpdateMode) {
    let (lints, deprecated_lints, renamed_lints) = gather_all();
    generate_lint_files(update_mode, &lints, &deprecated_lints, &renamed_lints);
}

fn generate_lint_files(
    update_mode: UpdateMode,
    lints: &[Lint],
    deprecated_lints: &[DeprecatedLint],
    renamed_lints: &[RenamedLint],
) {
    let internal_lints = Lint::internal_lints(lints);
    let usable_lints = Lint::usable_lints(lints);
    let mut sorted_usable_lints = usable_lints.clone();
    sorted_usable_lints.sort_by_key(|lint| lint.name.clone());

    replace_region_in_file(
        update_mode,
        Path::new("README.md"),
        "[There are over ",
        " lints included in this crate!]",
        |res| {
            write!(res, "{}", round_to_fifty(usable_lints.len())).unwrap();
        },
    );

    replace_region_in_file(
        update_mode,
        Path::new("CHANGELOG.md"),
        "<!-- begin autogenerated links to lint list -->\n",
        "<!-- end autogenerated links to lint list -->",
        |res| {
            for lint in usable_lints
                .iter()
                .map(|l| &l.name)
                .chain(deprecated_lints.iter().map(|l| &l.name))
                .sorted()
            {
                writeln!(res, "[`{}`]: {}#{}", lint, DOCS_LINK, lint).unwrap();
            }
        },
    );

    // This has to be in lib.rs, otherwise rustfmt doesn't work
    replace_region_in_file(
        update_mode,
        Path::new("clippy_lints/src/lib.rs"),
        "// begin lints modules, do not remove this comment, it’s used in `update_lints`\n",
        "// end lints modules, do not remove this comment, it’s used in `update_lints`",
        |res| {
            for lint_mod in usable_lints.iter().map(|l| &l.module).unique().sorted() {
                writeln!(res, "mod {};", lint_mod).unwrap();
            }
        },
    );

    process_file(
        "clippy_lints/src/lib.register_lints.rs",
        update_mode,
        &gen_register_lint_list(internal_lints.iter(), usable_lints.iter()),
    );
    process_file(
        "clippy_lints/src/lib.deprecated.rs",
        update_mode,
        &gen_deprecated(deprecated_lints),
    );

    let all_group_lints = usable_lints.iter().filter(|l| {
        matches!(
            &*l.group,
            "correctness" | "suspicious" | "style" | "complexity" | "perf"
        )
    });
    let content = gen_lint_group_list("all", all_group_lints);
    process_file("clippy_lints/src/lib.register_all.rs", update_mode, &content);

    for (lint_group, lints) in Lint::by_lint_group(usable_lints.into_iter().chain(internal_lints)) {
        let content = gen_lint_group_list(&lint_group, lints.iter());
        process_file(
            &format!("clippy_lints/src/lib.register_{}.rs", lint_group),
            update_mode,
            &content,
        );
    }

    let content = gen_deprecated_lints_test(deprecated_lints);
    process_file("tests/ui/deprecated.rs", update_mode, &content);

    let content = gen_renamed_lints_test(renamed_lints);
    process_file("tests/ui/rename.rs", update_mode, &content);
}

pub fn print_lints() {
    let (lint_list, _, _) = gather_all();
    let usable_lints = Lint::usable_lints(&lint_list);
    let usable_lint_count = usable_lints.len();
    let grouped_by_lint_group = Lint::by_lint_group(usable_lints.into_iter());

    for (lint_group, mut lints) in grouped_by_lint_group {
        println!("\n## {}", lint_group);

        lints.sort_by_key(|l| l.name.clone());

        for lint in lints {
            println!("* [{}]({}#{}) ({})", lint.name, DOCS_LINK, lint.name, lint.desc);
        }
    }

    println!("there are {} lints", usable_lint_count);
}

/// Runs the `rename_lint` command.
///
/// This does the following:
/// * Adds an entry to `renamed_lints.rs`.
/// * Renames all lint attributes to the new name (e.g. `#[allow(clippy::lint_name)]`).
/// * Renames the lint struct to the new name.
/// * Renames the module containing the lint struct to the new name if it shares a name with the
///   lint.
///
/// # Panics
/// Panics for the following conditions:
/// * If a file path could not read from or then written to
/// * If either lint name has a prefix
/// * If `old_name` doesn't name an existing lint.
/// * If `old_name` names a deprecated or renamed lint.
#[allow(clippy::too_many_lines)]
pub fn rename(old_name: &str, new_name: &str, uplift: bool) {
    if let Some((prefix, _)) = old_name.split_once("::") {
        panic!("`{}` should not contain the `{}` prefix", old_name, prefix);
    }
    if let Some((prefix, _)) = new_name.split_once("::") {
        panic!("`{}` should not contain the `{}` prefix", new_name, prefix);
    }

    let (mut lints, deprecated_lints, mut renamed_lints) = gather_all();
    let mut old_lint_index = None;
    let mut found_new_name = false;
    for (i, lint) in lints.iter().enumerate() {
        if lint.name == old_name {
            old_lint_index = Some(i);
        } else if lint.name == new_name {
            found_new_name = true;
        }
    }
    let old_lint_index = old_lint_index.unwrap_or_else(|| panic!("could not find lint `{}`", old_name));

    let lint = RenamedLint {
        old_name: format!("clippy::{}", old_name),
        new_name: if uplift {
            new_name.into()
        } else {
            format!("clippy::{}", new_name)
        },
    };

    // Renamed lints and deprecated lints shouldn't have been found in the lint list, but check just in
    // case.
    assert!(
        !renamed_lints.iter().any(|l| lint.old_name == l.old_name),
        "`{}` has already been renamed",
        old_name
    );
    assert!(
        !deprecated_lints.iter().any(|l| lint.old_name == l.name),
        "`{}` has already been deprecated",
        old_name
    );

    // Update all lint level attributes. (`clippy::lint_name`)
    for file in WalkDir::new(clippy_project_root())
        .into_iter()
        .map(Result::unwrap)
        .filter(|f| {
            let name = f.path().file_name();
            let ext = f.path().extension();
            (ext == Some(OsStr::new("rs")) || ext == Some(OsStr::new("fixed")))
                && name != Some(OsStr::new("rename.rs"))
                && name != Some(OsStr::new("renamed_lints.rs"))
        })
    {
        rewrite_file(file.path(), |s| {
            replace_ident_like(s, &[(&lint.old_name, &lint.new_name)])
        });
    }

    renamed_lints.push(lint);
    renamed_lints.sort_by(|lhs, rhs| {
        lhs.new_name
            .starts_with("clippy::")
            .cmp(&rhs.new_name.starts_with("clippy::"))
            .reverse()
            .then_with(|| lhs.old_name.cmp(&rhs.old_name))
    });

    write_file(
        Path::new("clippy_lints/src/renamed_lints.rs"),
        &gen_renamed_lints_list(&renamed_lints),
    );

    if uplift {
        write_file(Path::new("tests/ui/rename.rs"), &gen_renamed_lints_test(&renamed_lints));
        println!(
            "`{}` has be uplifted. All the code inside `clippy_lints` related to it needs to be removed manually.",
            old_name
        );
    } else if found_new_name {
        write_file(Path::new("tests/ui/rename.rs"), &gen_renamed_lints_test(&renamed_lints));
        println!(
            "`{}` is already defined. The old linting code inside `clippy_lints` needs to be updated/removed manually.",
            new_name
        );
    } else {
        // Rename the lint struct and source files sharing a name with the lint.
        let lint = &mut lints[old_lint_index];
        let old_name_upper = old_name.to_uppercase();
        let new_name_upper = new_name.to_uppercase();
        lint.name = new_name.into();

        // Rename test files. only rename `.stderr` and `.fixed` files if the new test name doesn't exist.
        if try_rename_file(
            Path::new(&format!("tests/ui/{}.rs", old_name)),
            Path::new(&format!("tests/ui/{}.rs", new_name)),
        ) {
            try_rename_file(
                Path::new(&format!("tests/ui/{}.stderr", old_name)),
                Path::new(&format!("tests/ui/{}.stderr", new_name)),
            );
            try_rename_file(
                Path::new(&format!("tests/ui/{}.fixed", old_name)),
                Path::new(&format!("tests/ui/{}.fixed", new_name)),
            );
        }

        // Try to rename the file containing the lint if the file name matches the lint's name.
        let replacements;
        let replacements = if lint.module == old_name
            && try_rename_file(
                Path::new(&format!("clippy_lints/src/{}.rs", old_name)),
                Path::new(&format!("clippy_lints/src/{}.rs", new_name)),
            ) {
            // Edit the module name in the lint list. Note there could be multiple lints.
            for lint in lints.iter_mut().filter(|l| l.module == old_name) {
                lint.module = new_name.into();
            }
            replacements = [(&*old_name_upper, &*new_name_upper), (old_name, new_name)];
            replacements.as_slice()
        } else if !lint.module.contains("::")
            // Catch cases like `methods/lint_name.rs` where the lint is stored in `methods/mod.rs`
            && try_rename_file(
                Path::new(&format!("clippy_lints/src/{}/{}.rs", lint.module, old_name)),
                Path::new(&format!("clippy_lints/src/{}/{}.rs", lint.module, new_name)),
            )
        {
            // Edit the module name in the lint list. Note there could be multiple lints, or none.
            let renamed_mod = format!("{}::{}", lint.module, old_name);
            for lint in lints.iter_mut().filter(|l| l.module == renamed_mod) {
                lint.module = format!("{}::{}", lint.module, new_name);
            }
            replacements = [(&*old_name_upper, &*new_name_upper), (old_name, new_name)];
            replacements.as_slice()
        } else {
            replacements = [(&*old_name_upper, &*new_name_upper), ("", "")];
            &replacements[0..1]
        };

        // Don't change `clippy_utils/src/renamed_lints.rs` here as it would try to edit the lint being
        // renamed.
        for (_, file) in clippy_lints_src_files().filter(|(rel_path, _)| rel_path != OsStr::new("renamed_lints.rs")) {
            rewrite_file(file.path(), |s| replace_ident_like(s, replacements));
        }

        generate_lint_files(UpdateMode::Change, &lints, &deprecated_lints, &renamed_lints);
        println!("{} has been successfully renamed", old_name);
    }

    println!("note: `cargo uitest` still needs to be run to update the test results");
}

/// Replace substrings if they aren't bordered by identifier characters. Returns `None` if there
/// were no replacements.
fn replace_ident_like(contents: &str, replacements: &[(&str, &str)]) -> Option<String> {
    fn is_ident_char(c: u8) -> bool {
        matches!(c, b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'_')
    }

    let searcher = AhoCorasickBuilder::new()
        .dfa(true)
        .match_kind(aho_corasick::MatchKind::LeftmostLongest)
        .build_with_size::<u16, _, _>(replacements.iter().map(|&(x, _)| x.as_bytes()))
        .unwrap();

    let mut result = String::with_capacity(contents.len() + 1024);
    let mut pos = 0;
    let mut edited = false;
    for m in searcher.find_iter(contents) {
        let (old, new) = replacements[m.pattern()];
        result.push_str(&contents[pos..m.start()]);
        result.push_str(
            if !is_ident_char(contents.as_bytes().get(m.start().wrapping_sub(1)).copied().unwrap_or(0))
                && !is_ident_char(contents.as_bytes().get(m.end()).copied().unwrap_or(0))
            {
                edited = true;
                new
            } else {
                old
            },
        );
        pos = m.end();
    }
    result.push_str(&contents[pos..]);
    edited.then(|| result)
}

fn round_to_fifty(count: usize) -> usize {
    count / 50 * 50
}

fn process_file(path: impl AsRef<Path>, update_mode: UpdateMode, content: &str) {
    if update_mode == UpdateMode::Check {
        let old_content =
            fs::read_to_string(&path).unwrap_or_else(|e| panic!("Cannot read from {}: {}", path.as_ref().display(), e));
        if content != old_content {
            exit_with_failure();
        }
    } else {
        fs::write(&path, content.as_bytes())
            .unwrap_or_else(|e| panic!("Cannot write to {}: {}", path.as_ref().display(), e));
    }
}

fn exit_with_failure() {
    println!(
        "Not all lints defined properly. \
                 Please run `cargo dev update_lints` to make sure all lints are defined properly."
    );
    std::process::exit(1);
}

/// Lint data parsed from the Clippy source code.
#[derive(Clone, PartialEq, Debug)]
struct Lint {
    name: String,
    group: String,
    desc: String,
    module: String,
}

impl Lint {
    #[must_use]
    fn new(name: &str, group: &str, desc: &str, module: &str) -> Self {
        Self {
            name: name.to_lowercase(),
            group: group.into(),
            desc: remove_line_splices(desc),
            module: module.into(),
        }
    }

    /// Returns all non-deprecated lints and non-internal lints
    #[must_use]
    fn usable_lints(lints: &[Self]) -> Vec<Self> {
        lints
            .iter()
            .filter(|l| !l.group.starts_with("internal"))
            .cloned()
            .collect()
    }

    /// Returns all internal lints (not `internal_warn` lints)
    #[must_use]
    fn internal_lints(lints: &[Self]) -> Vec<Self> {
        lints.iter().filter(|l| l.group == "internal").cloned().collect()
    }

    /// Returns the lints in a `HashMap`, grouped by the different lint groups
    #[must_use]
    fn by_lint_group(lints: impl Iterator<Item = Self>) -> HashMap<String, Vec<Self>> {
        lints.map(|lint| (lint.group.to_string(), lint)).into_group_map()
    }
}

#[derive(Clone, PartialEq, Debug)]
struct DeprecatedLint {
    name: String,
    reason: String,
}
impl DeprecatedLint {
    fn new(name: &str, reason: &str) -> Self {
        Self {
            name: name.to_lowercase(),
            reason: remove_line_splices(reason),
        }
    }
}

struct RenamedLint {
    old_name: String,
    new_name: String,
}
impl RenamedLint {
    fn new(old_name: &str, new_name: &str) -> Self {
        Self {
            old_name: remove_line_splices(old_name),
            new_name: remove_line_splices(new_name),
        }
    }
}

/// Generates the code for registering a group
fn gen_lint_group_list<'a>(group_name: &str, lints: impl Iterator<Item = &'a Lint>) -> String {
    let mut details: Vec<_> = lints.map(|l| (&l.module, l.name.to_uppercase())).collect();
    details.sort_unstable();

    let mut output = GENERATED_FILE_COMMENT.to_string();

    let _ = writeln!(
        output,
        "store.register_group(true, \"clippy::{0}\", Some(\"clippy_{0}\"), vec![",
        group_name
    );
    for (module, name) in details {
        let _ = writeln!(output, "    LintId::of({}::{}),", module, name);
    }
    output.push_str("])\n");

    output
}

/// Generates the `register_removed` code
#[must_use]
fn gen_deprecated(lints: &[DeprecatedLint]) -> String {
    let mut output = GENERATED_FILE_COMMENT.to_string();
    output.push_str("{\n");
    for lint in lints {
        let _ = write!(
            output,
            concat!(
                "    store.register_removed(\n",
                "        \"clippy::{}\",\n",
                "        \"{}\",\n",
                "    );\n"
            ),
            lint.name, lint.reason,
        );
    }
    output.push_str("}\n");

    output
}

/// Generates the code for registering lints
#[must_use]
fn gen_register_lint_list<'a>(
    internal_lints: impl Iterator<Item = &'a Lint>,
    usable_lints: impl Iterator<Item = &'a Lint>,
) -> String {
    let mut details: Vec<_> = internal_lints
        .map(|l| (false, &l.module, l.name.to_uppercase()))
        .chain(usable_lints.map(|l| (true, &l.module, l.name.to_uppercase())))
        .collect();
    details.sort_unstable();

    let mut output = GENERATED_FILE_COMMENT.to_string();
    output.push_str("store.register_lints(&[\n");

    for (is_public, module_name, lint_name) in details {
        if !is_public {
            output.push_str("    #[cfg(feature = \"internal\")]\n");
        }
        let _ = writeln!(output, "    {}::{},", module_name, lint_name);
    }
    output.push_str("])\n");

    output
}

fn gen_deprecated_lints_test(lints: &[DeprecatedLint]) -> String {
    let mut res: String = GENERATED_FILE_COMMENT.into();
    for lint in lints {
        writeln!(res, "#![warn(clippy::{})]", lint.name).unwrap();
    }
    res.push_str("\nfn main() {}\n");
    res
}

fn gen_renamed_lints_test(lints: &[RenamedLint]) -> String {
    let mut seen_lints = HashSet::new();
    let mut res: String = GENERATED_FILE_COMMENT.into();
    res.push_str("// run-rustfix\n\n");
    for lint in lints {
        if seen_lints.insert(&lint.new_name) {
            writeln!(res, "#![allow({})]", lint.new_name).unwrap();
        }
    }
    seen_lints.clear();
    for lint in lints {
        if seen_lints.insert(&lint.old_name) {
            writeln!(res, "#![warn({})]", lint.old_name).unwrap();
        }
    }
    res.push_str("\nfn main() {}\n");
    res
}

fn gen_renamed_lints_list(lints: &[RenamedLint]) -> String {
    const HEADER: &str = "\
        // This file is managed by `cargo dev rename_lint`. Prefer using that when possible.\n\n\
        #[rustfmt::skip]\n\
        pub static RENAMED_LINTS: &[(&str, &str)] = &[\n";

    let mut res = String::from(HEADER);
    for lint in lints {
        writeln!(res, "    (\"{}\", \"{}\"),", lint.old_name, lint.new_name).unwrap();
    }
    res.push_str("];\n");
    res
}

/// Gathers all lints defined in `clippy_lints/src`
fn gather_all() -> (Vec<Lint>, Vec<DeprecatedLint>, Vec<RenamedLint>) {
    let mut lints = Vec::with_capacity(1000);
    let mut deprecated_lints = Vec::with_capacity(50);
    let mut renamed_lints = Vec::with_capacity(50);

    for (rel_path, file) in clippy_lints_src_files() {
        let path = file.path();
        let contents =
            fs::read_to_string(path).unwrap_or_else(|e| panic!("Cannot read from `{}`: {}", path.display(), e));
        let module = rel_path
            .components()
            .map(|c| c.as_os_str().to_str().unwrap())
            .collect::<Vec<_>>()
            .join("::");

        // If the lints are stored in mod.rs, we get the module name from
        // the containing directory:
        let module = if let Some(module) = module.strip_suffix("::mod.rs") {
            module
        } else {
            module.strip_suffix(".rs").unwrap_or(&module)
        };

        match module {
            "deprecated_lints" => parse_deprecated_contents(&contents, &mut deprecated_lints),
            "renamed_lints" => parse_renamed_contents(&contents, &mut renamed_lints),
            _ => parse_contents(&contents, module, &mut lints),
        }
    }
    (lints, deprecated_lints, renamed_lints)
}

fn clippy_lints_src_files() -> impl Iterator<Item = (PathBuf, DirEntry)> {
    let root_path = clippy_project_root().join("clippy_lints/src");
    let iter = WalkDir::new(&root_path).into_iter();
    iter.map(Result::unwrap)
        .filter(|f| f.path().extension() == Some(OsStr::new("rs")))
        .map(move |f| (f.path().strip_prefix(&root_path).unwrap().to_path_buf(), f))
}

macro_rules! match_tokens {
    ($iter:ident, $($token:ident $({$($fields:tt)*})? $(($capture:ident))?)*) => {
         {
            $($(let $capture =)? if let Some((TokenKind::$token $({$($fields)*})?, _x)) = $iter.next() {
                _x
            } else {
                continue;
            };)*
            #[allow(clippy::unused_unit)]
            { ($($($capture,)?)*) }
        }
    }
}

/// Parse a source file looking for `declare_clippy_lint` macro invocations.
fn parse_contents(contents: &str, module: &str, lints: &mut Vec<Lint>) {
    let mut offset = 0usize;
    let mut iter = tokenize(contents).map(|t| {
        let range = offset..offset + t.len;
        offset = range.end;
        (t.kind, &contents[range])
    });

    while iter.any(|(kind, s)| kind == TokenKind::Ident && s == "declare_clippy_lint") {
        let mut iter = iter
            .by_ref()
            .filter(|&(kind, _)| !matches!(kind, TokenKind::Whitespace | TokenKind::LineComment { .. }));
        // matches `!{`
        match_tokens!(iter, Bang OpenBrace);
        match iter.next() {
            // #[clippy::version = "version"] pub
            Some((TokenKind::Pound, _)) => {
                match_tokens!(iter, OpenBracket Ident Colon Colon Ident Eq Literal{..} CloseBracket Ident);
            },
            // pub
            Some((TokenKind::Ident, _)) => (),
            _ => continue,
        }
        let (name, group, desc) = match_tokens!(
            iter,
            // LINT_NAME
            Ident(name) Comma
            // group,
            Ident(group) Comma
            // "description" }
            Literal{..}(desc) CloseBrace
        );
        lints.push(Lint::new(name, group, desc, module));
    }
}

/// Parse a source file looking for `declare_deprecated_lint` macro invocations.
fn parse_deprecated_contents(contents: &str, lints: &mut Vec<DeprecatedLint>) {
    let mut offset = 0usize;
    let mut iter = tokenize(contents).map(|t| {
        let range = offset..offset + t.len;
        offset = range.end;
        (t.kind, &contents[range])
    });
    while iter.any(|(kind, s)| kind == TokenKind::Ident && s == "declare_deprecated_lint") {
        let mut iter = iter
            .by_ref()
            .filter(|&(kind, _)| !matches!(kind, TokenKind::Whitespace | TokenKind::LineComment { .. }));
        let (name, reason) = match_tokens!(
            iter,
            // !{
            Bang OpenBrace
            // #[clippy::version = "version"]
            Pound OpenBracket Ident Colon Colon Ident Eq Literal{..} CloseBracket
            // pub LINT_NAME,
            Ident Ident(name) Comma
            // "description"
            Literal{kind: LiteralKind::Str{..},..}(reason)
            // }
            CloseBrace
        );
        lints.push(DeprecatedLint::new(name, reason));
    }
}

fn parse_renamed_contents(contents: &str, lints: &mut Vec<RenamedLint>) {
    for line in contents.lines() {
        let mut offset = 0usize;
        let mut iter = tokenize(line).map(|t| {
            let range = offset..offset + t.len;
            offset = range.end;
            (t.kind, &line[range])
        });
        let (old_name, new_name) = match_tokens!(
            iter,
            // ("old_name",
            Whitespace OpenParen Literal{kind: LiteralKind::Str{..},..}(old_name) Comma
            // "new_name"),
            Whitespace Literal{kind: LiteralKind::Str{..},..}(new_name) CloseParen Comma
        );
        lints.push(RenamedLint::new(old_name, new_name));
    }
}

/// Removes the line splices and surrounding quotes from a string literal
fn remove_line_splices(s: &str) -> String {
    let s = s
        .strip_prefix('r')
        .unwrap_or(s)
        .trim_matches('#')
        .strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
        .unwrap_or_else(|| panic!("expected quoted string, found `{}`", s));
    let mut res = String::with_capacity(s.len());
    unescape::unescape_literal(s, unescape::Mode::Str, &mut |range, _| res.push_str(&s[range]));
    res
}

/// Replaces a region in a file delimited by two lines matching regexes.
///
/// `path` is the relative path to the file on which you want to perform the replacement.
///
/// See `replace_region_in_text` for documentation of the other options.
///
/// # Panics
///
/// Panics if the path could not read or then written
fn replace_region_in_file(
    update_mode: UpdateMode,
    path: &Path,
    start: &str,
    end: &str,
    write_replacement: impl FnMut(&mut String),
) {
    let contents = fs::read_to_string(path).unwrap_or_else(|e| panic!("Cannot read from `{}`: {}", path.display(), e));
    let new_contents = match replace_region_in_text(&contents, start, end, write_replacement) {
        Ok(x) => x,
        Err(delim) => panic!("Couldn't find `{}` in file `{}`", delim, path.display()),
    };

    match update_mode {
        UpdateMode::Check if contents != new_contents => exit_with_failure(),
        UpdateMode::Check => (),
        UpdateMode::Change => {
            if let Err(e) = fs::write(path, new_contents.as_bytes()) {
                panic!("Cannot write to `{}`: {}", path.display(), e);
            }
        },
    }
}

/// Replaces a region in a text delimited by two strings. Returns the new text if both delimiters
/// were found, or the missing delimiter if not.
fn replace_region_in_text<'a>(
    text: &str,
    start: &'a str,
    end: &'a str,
    mut write_replacement: impl FnMut(&mut String),
) -> Result<String, &'a str> {
    let (text_start, rest) = text.split_once(start).ok_or(start)?;
    let (_, text_end) = rest.split_once(end).ok_or(end)?;

    let mut res = String::with_capacity(text.len() + 4096);
    res.push_str(text_start);
    res.push_str(start);
    write_replacement(&mut res);
    res.push_str(end);
    res.push_str(text_end);

    Ok(res)
}

fn try_rename_file(old_name: &Path, new_name: &Path) -> bool {
    match fs::OpenOptions::new().create_new(true).write(true).open(new_name) {
        Ok(file) => drop(file),
        Err(e) if matches!(e.kind(), io::ErrorKind::AlreadyExists | io::ErrorKind::NotFound) => return false,
        Err(e) => panic_file(e, new_name, "create"),
    };
    match fs::rename(old_name, new_name) {
        Ok(()) => true,
        Err(e) => {
            drop(fs::remove_file(new_name));
            if e.kind() == io::ErrorKind::NotFound {
                false
            } else {
                panic_file(e, old_name, "rename");
            }
        },
    }
}

#[allow(clippy::needless_pass_by_value)]
fn panic_file(error: io::Error, name: &Path, action: &str) -> ! {
    panic!("failed to {} file `{}`: {}", action, name.display(), error)
}

fn rewrite_file(path: &Path, f: impl FnOnce(&str) -> Option<String>) {
    let mut file = fs::OpenOptions::new()
        .write(true)
        .read(true)
        .open(path)
        .unwrap_or_else(|e| panic_file(e, path, "open"));
    let mut buf = String::new();
    file.read_to_string(&mut buf)
        .unwrap_or_else(|e| panic_file(e, path, "read"));
    if let Some(new_contents) = f(&buf) {
        file.rewind().unwrap_or_else(|e| panic_file(e, path, "write"));
        file.write_all(new_contents.as_bytes())
            .unwrap_or_else(|e| panic_file(e, path, "write"));
        file.set_len(new_contents.len() as u64)
            .unwrap_or_else(|e| panic_file(e, path, "write"));
    }
}

fn write_file(path: &Path, contents: &str) {
    fs::write(path, contents).unwrap_or_else(|e| panic_file(e, path, "write"));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_contents() {
        static CONTENTS: &str = r#"
            declare_clippy_lint! {
                #[clippy::version = "Hello Clippy!"]
                pub PTR_ARG,
                style,
                "really long \
                text"
            }

            declare_clippy_lint!{
                #[clippy::version = "Test version"]
                pub DOC_MARKDOWN,
                pedantic,
                "single line"
            }
        "#;
        let mut result = Vec::new();
        parse_contents(CONTENTS, "module_name", &mut result);

        let expected = vec![
            Lint::new("ptr_arg", "style", "\"really long text\"", "module_name"),
            Lint::new("doc_markdown", "pedantic", "\"single line\"", "module_name"),
        ];
        assert_eq!(expected, result);
    }

    #[test]
    fn test_parse_deprecated_contents() {
        static DEPRECATED_CONTENTS: &str = r#"
            /// some doc comment
            declare_deprecated_lint! {
                #[clippy::version = "I'm a version"]
                pub SHOULD_ASSERT_EQ,
                "`assert!()` will be more flexible with RFC 2011"
            }
        "#;

        let mut result = Vec::new();
        parse_deprecated_contents(DEPRECATED_CONTENTS, &mut result);

        let expected = vec![DeprecatedLint::new(
            "should_assert_eq",
            "\"`assert!()` will be more flexible with RFC 2011\"",
        )];
        assert_eq!(expected, result);
    }

    #[test]
    fn test_usable_lints() {
        let lints = vec![
            Lint::new("should_assert_eq2", "Not Deprecated", "\"abc\"", "module_name"),
            Lint::new("should_assert_eq2", "internal", "\"abc\"", "module_name"),
            Lint::new("should_assert_eq2", "internal_style", "\"abc\"", "module_name"),
        ];
        let expected = vec![Lint::new(
            "should_assert_eq2",
            "Not Deprecated",
            "\"abc\"",
            "module_name",
        )];
        assert_eq!(expected, Lint::usable_lints(&lints));
    }

    #[test]
    fn test_by_lint_group() {
        let lints = vec![
            Lint::new("should_assert_eq", "group1", "\"abc\"", "module_name"),
            Lint::new("should_assert_eq2", "group2", "\"abc\"", "module_name"),
            Lint::new("incorrect_match", "group1", "\"abc\"", "module_name"),
        ];
        let mut expected: HashMap<String, Vec<Lint>> = HashMap::new();
        expected.insert(
            "group1".to_string(),
            vec![
                Lint::new("should_assert_eq", "group1", "\"abc\"", "module_name"),
                Lint::new("incorrect_match", "group1", "\"abc\"", "module_name"),
            ],
        );
        expected.insert(
            "group2".to_string(),
            vec![Lint::new("should_assert_eq2", "group2", "\"abc\"", "module_name")],
        );
        assert_eq!(expected, Lint::by_lint_group(lints.into_iter()));
    }

    #[test]
    fn test_gen_deprecated() {
        let lints = vec![
            DeprecatedLint::new("should_assert_eq", "\"has been superseded by should_assert_eq2\""),
            DeprecatedLint::new("another_deprecated", "\"will be removed\""),
        ];

        let expected = GENERATED_FILE_COMMENT.to_string()
            + &[
                "{",
                "    store.register_removed(",
                "        \"clippy::should_assert_eq\",",
                "        \"has been superseded by should_assert_eq2\",",
                "    );",
                "    store.register_removed(",
                "        \"clippy::another_deprecated\",",
                "        \"will be removed\",",
                "    );",
                "}",
            ]
            .join("\n")
            + "\n";

        assert_eq!(expected, gen_deprecated(&lints));
    }

    #[test]
    fn test_gen_lint_group_list() {
        let lints = vec![
            Lint::new("abc", "group1", "\"abc\"", "module_name"),
            Lint::new("should_assert_eq", "group1", "\"abc\"", "module_name"),
            Lint::new("internal", "internal_style", "\"abc\"", "module_name"),
        ];
        let expected = GENERATED_FILE_COMMENT.to_string()
            + &[
                "store.register_group(true, \"clippy::group1\", Some(\"clippy_group1\"), vec![",
                "    LintId::of(module_name::ABC),",
                "    LintId::of(module_name::INTERNAL),",
                "    LintId::of(module_name::SHOULD_ASSERT_EQ),",
                "])",
            ]
            .join("\n")
            + "\n";

        let result = gen_lint_group_list("group1", lints.iter());

        assert_eq!(expected, result);
    }
}
