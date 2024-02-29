#![warn(clippy::unnecessary_get_then_check)]

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

fn main() {
    let s: HashSet<String> = HashSet::new();
    let _ = s.get("a").is_some(); //~ ERROR: unnecessary use of `get("a").is_some()`
    let _ = s.get("a").is_none(); //~ ERROR: unnecessary use of `get("a").is_none()`

    let s: HashMap<String, ()> = HashMap::new();
    let _ = s.get("a").is_some(); //~ ERROR: unnecessary use of `get("a").is_some()`
    let _ = s.get("a").is_none(); //~ ERROR: unnecessary use of `get("a").is_none()`

    let s: BTreeSet<String> = BTreeSet::new();
    let _ = s.get("a").is_some(); //~ ERROR: unnecessary use of `get("a").is_some()`
    let _ = s.get("a").is_none(); //~ ERROR: unnecessary use of `get("a").is_none()`

    let s: BTreeMap<String, ()> = BTreeMap::new();
    let _ = s.get("a").is_some(); //~ ERROR: unnecessary use of `get("a").is_some()`
    let _ = s.get("a").is_none(); //~ ERROR: unnecessary use of `get("a").is_none()`

    // Import to check that the generic annotations are kept!
    let s: HashSet<String> = HashSet::new();
    let _ = s.get::<str>("a").is_some(); //~ ERROR: unnecessary use of `get::<str>("a").is_some()`
    let _ = s.get::<str>("a").is_none(); //~ ERROR: unnecessary use of `get::<str>("a").is_none()`
}
