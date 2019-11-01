//! Checks that all error codes have at least one test to prevent having error
//! codes that are silently not thrown by the compiler anymore.

use std::collections::HashMap;
use std::ffi::OsStr;
use std::path::Path;

// A few of those error codes can't be tested but all the others can and *should* be tested!
const WHITELIST: &[&str] = &[
    "E0183",
    "E0227",
    "E0279",
    "E0280",
    "E0311",
    "E0313",
    "E0314",
    "E0315",
    "E0377",
    "E0456",
    "E0461",
    "E0462",
    "E0464",
    "E0465",
    "E0472",
    "E0473",
    "E0474",
    "E0475",
    "E0476",
    "E0479",
    "E0480",
    "E0481",
    "E0482",
    "E0483",
    "E0484",
    "E0485",
    "E0486",
    "E0487",
    "E0488",
    "E0489",
    "E0514",
    "E0519",
    "E0523",
    "E0554",
    "E0570",
    "E0629",
    "E0630",
    "E0640",
    "E0717",
    "E0727",
    "E0729",
];

fn extract_error_codes(f: &str, error_codes: &mut HashMap<String, bool>) {
    let mut reached_no_explanation = false;
    let mut last_error_code = None;

    for line in f.lines() {
        let s = line.trim();
        if s.starts_with('E') && s.ends_with(": r##\"") {
            if let Some(err_code) = s.splitn(2, ':').next() {
                let err_code = err_code.to_owned();
                last_error_code = Some(err_code.clone());
                if !error_codes.contains_key(&err_code) {
                    error_codes.insert(err_code, false);
                }
            }
        } else if s.starts_with("```") && s.contains("compile_fail") && s.contains('E') {
            if let Some(err_code) = s.splitn(2, 'E').skip(1).next() {
                if let Some(err_code) = err_code.splitn(2, ',').next() {
                    let nb = error_codes.entry(format!("E{}", err_code)).or_insert(false);
                    *nb = true;
                }
            }
        } else if s == ";" {
            reached_no_explanation = true;
        } else if reached_no_explanation && s.starts_with('E') {
            if let Some(err_code) = s.splitn(2, ',').next() {
                let err_code = err_code.to_owned();
                if !error_codes.contains_key(&err_code) { // this check should *never* fail!
                    error_codes.insert(err_code, false);
                }
            }
        } else if s.starts_with("#### Note: this error code is no longer emitted by the compiler") {
            if let Some(last) = last_error_code {
                error_codes.get_mut(&last).map(|x| *x = true);
            }
            last_error_code = None;
        }
    }
}

fn extract_error_codes_from_tests(f: &str, error_codes: &mut HashMap<String, bool>) {
    for line in f.lines() {
        let s = line.trim();
        if s.starts_with("error[E") || s.starts_with("warning[E") {
            if let Some(err_code) = s.splitn(2, ']').next() {
                if let Some(err_code) = err_code.splitn(2, '[').skip(1).next() {
                    let nb = error_codes.entry(err_code.to_owned()).or_insert(false);
                    *nb = true;
                }
            }
        }
    }
}

pub fn check(path: &Path, bad: &mut bool) {
    println!("Checking which error codes lack tests...");
    let mut error_codes: HashMap<String, bool> = HashMap::new();
    super::walk(path,
                &mut |path| super::filter_dirs(path),
                &mut |entry, contents| {
        let file_name = entry.file_name();
        if file_name == "error_codes.rs" {
            extract_error_codes(contents, &mut error_codes);
        } else if entry.path().extension() == Some(OsStr::new("stderr")) {
            extract_error_codes_from_tests(contents, &mut error_codes);
        }
    });
    println!("Found {} error codes", error_codes.len());

    let mut errors = Vec::new();
    for (err_code, nb) in &error_codes {
        if !*nb && !WHITELIST.contains(&err_code.as_str()) {
            errors.push(format!("Error code {} needs to have at least one UI test!", err_code));
        }
    }
    errors.sort();
    for err in &errors {
        eprintln!("{}", err);
    }
    println!("Found {} error codes with no tests", errors.len());
    if !errors.is_empty() {
        *bad = true;
    }
    println!("Done!");
}
