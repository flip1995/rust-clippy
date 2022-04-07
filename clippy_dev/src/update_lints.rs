use core::fmt::Write;
use itertools::Itertools;
use rustc_lexer::{tokenize, unescape, LiteralKind, TokenKind};
use std::collections::HashMap;
use std::ffi::OsStr;
use std::fs;
use std::path::Path;
use walkdir::WalkDir;

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
#[allow(clippy::too_many_lines)]
pub fn run(update_mode: UpdateMode) {
    let (lints, deprecated_lints) = gather_all();

    let internal_lints = Lint::internal_lints(&lints);
    let usable_lints = Lint::usable_lints(&lints);
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
        &gen_deprecated(&deprecated_lints),
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
}

pub fn print_lints() {
    let (lint_list, _) = gather_all();
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

/// Generates the code for registering a group
fn gen_lint_group_list<'a>(group_name: &str, lints: impl Iterator<Item = &'a Lint>) -> String {
    let mut details: Vec<_> = lints.map(|l| (&l.module, l.name.to_uppercase())).collect();
    details.sort_unstable();

    let mut output = GENERATED_FILE_COMMENT.to_string();

    output.push_str(&format!(
        "store.register_group(true, \"clippy::{0}\", Some(\"clippy_{0}\"), vec![\n",
        group_name
    ));
    for (module, name) in details {
        output.push_str(&format!("    LintId::of({}::{}),\n", module, name));
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
        output.push_str(&format!(
            concat!(
                "    store.register_removed(\n",
                "        \"clippy::{}\",\n",
                "        \"{}\",\n",
                "    );\n"
            ),
            lint.name, lint.reason,
        ));
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
        output.push_str(&format!("    {}::{},\n", module_name, lint_name));
    }
    output.push_str("])\n");

    output
}

/// Gathers all lints defined in `clippy_lints/src`
fn gather_all() -> (Vec<Lint>, Vec<DeprecatedLint>) {
    let mut lints = Vec::with_capacity(1000);
    let mut deprecated_lints = Vec::with_capacity(50);
    let root_path = clippy_project_root().join("clippy_lints/src");

    for (rel_path, file) in WalkDir::new(&root_path)
        .into_iter()
        .map(Result::unwrap)
        .filter(|f| f.path().extension() == Some(OsStr::new("rs")))
        .map(|f| (f.path().strip_prefix(&root_path).unwrap().to_path_buf(), f))
    {
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

        if module == "deprecated_lints" {
            parse_deprecated_contents(&contents, &mut deprecated_lints);
        } else {
            parse_contents(&contents, module, &mut lints);
        }
    }
    (lints, deprecated_lints)
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
