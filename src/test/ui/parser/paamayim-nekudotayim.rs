// compile-flags: -Z parse-only

// http://phpsadness.com/sad/1

fn main() {
    ::; //~ ERROR expected identifier, found `;`
}
