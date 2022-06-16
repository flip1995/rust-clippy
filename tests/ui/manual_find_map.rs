// run-rustfix
#![allow(dead_code)]
#![warn(clippy::manual_find_map)]
#![allow(clippy::redundant_closure)] // FIXME suggestion may have redundant closure

fn main() {
    // is_some(), unwrap()
    let _ = (0..).find(|n| to_opt(*n).is_some()).map(|a| to_opt(a).unwrap());

    // ref pattern, expect()
    let _ = (0..).find(|&n| to_opt(n).is_some()).map(|a| to_opt(a).expect("hi"));

    // is_ok(), unwrap_or()
    let _ = (0..).find(|&n| to_res(n).is_ok()).map(|a| to_res(a).unwrap_or(1));
}

fn no_lint() {
    // no shared code
    let _ = (0..).filter(|n| *n > 1).map(|n| n + 1);

    // very close but different since filter() provides a reference
    let _ = (0..).find(|n| to_opt(n).is_some()).map(|a| to_opt(a).unwrap());

    // similar but different
    let _ = (0..).find(|n| to_opt(n).is_some()).map(|n| to_res(n).unwrap());
    let _ = (0..)
        .find(|n| to_opt(n).map(|n| n + 1).is_some())
        .map(|a| to_opt(a).unwrap());
}

fn to_opt<T>(_: T) -> Option<T> {
    unimplemented!()
}

fn to_res<T>(_: T) -> Result<T, ()> {
    unimplemented!()
}

struct Issue8920<'a> {
    option_field: Option<String>,
    result_field: Result<String, ()>,
    ref_field: Option<&'a usize>,
}

fn issue_8920() {
    let mut vec = vec![Issue8920 {
        option_field: Some(String::from("str")),
        result_field: Ok(String::from("str")),
        ref_field: Some(&1),
    }];

    let _ = vec
        .iter()
        .find(|f| f.option_field.is_some())
        .map(|f| f.option_field.clone().unwrap());

    let _ = vec
        .iter()
        .find(|f| f.ref_field.is_some())
        .map(|f| f.ref_field.cloned().unwrap());

    let _ = vec
        .iter()
        .find(|f| f.ref_field.is_some())
        .map(|f| f.ref_field.copied().unwrap());

    let _ = vec
        .iter()
        .find(|f| f.result_field.is_ok())
        .map(|f| f.result_field.clone().unwrap());

    let _ = vec
        .iter()
        .find(|f| f.result_field.is_ok())
        .map(|f| f.result_field.as_ref().unwrap());

    let _ = vec
        .iter()
        .find(|f| f.result_field.is_ok())
        .map(|f| f.result_field.as_deref().unwrap());

    let _ = vec
        .iter_mut()
        .find(|f| f.result_field.is_ok())
        .map(|f| f.result_field.as_mut().unwrap());

    let _ = vec
        .iter_mut()
        .find(|f| f.result_field.is_ok())
        .map(|f| f.result_field.as_deref_mut().unwrap());

    let _ = vec
        .iter()
        .find(|f| f.result_field.is_ok())
        .map(|f| f.result_field.to_owned().unwrap());
}
