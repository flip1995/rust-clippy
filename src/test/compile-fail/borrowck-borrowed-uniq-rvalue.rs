//buggy.rs
use std;
import std::map::hashmap;
import std::map;

fn main() {
    let buggy_map :hashmap<uint, &uint> =
      hashmap::<uint, &uint>(|x| { uint::hash(*x) },
                             |x, y| { uint::eq(*x, *y) });
    buggy_map.insert(42, ~1); //~ ERROR illegal borrow
    
    // but it is ok if we use a temporary
    let tmp = ~2;
    buggy_map.insert(43, tmp);
}
