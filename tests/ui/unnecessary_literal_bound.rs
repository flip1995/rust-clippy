#![warn(clippy::unnecessary_literal_bound)]

struct Struct<'a> {
    not_literal: &'a str,
}

impl Struct<'_> {
    // Should warn
    fn returns_lit(&self) -> &str {
        //~^ unnecessary_literal_bound
        "Hello"
    }

    // Should NOT warn
    fn returns_non_lit(&self) -> &str {
        self.not_literal
    }

    // Should warn, does not currently
    fn conditionally_returns_lit(&self, cond: bool) -> &str {
        if cond { "Literal" } else { "also a literal" }
    }

    // Should NOT warn
    fn conditionally_returns_non_lit(&self, cond: bool) -> &str {
        if cond { "Literal" } else { self.not_literal }
    }

    // Should warn
    fn contionally_returns_literals_explicit(&self, cond: bool) -> &str {
        //~^ unnecessary_literal_bound
        if cond {
            return "Literal";
        }

        "also a literal"
    }

    // Should NOT warn
    fn conditionally_returns_non_lit_explicit(&self, cond: bool) -> &str {
        if cond {
            return self.not_literal;
        }

        "Literal"
    }
}

trait ReturnsStr {
    fn trait_method(&self) -> &str;
}

impl ReturnsStr for u8 {
    // Should warn, even though not useful without trait refinement
    fn trait_method(&self) -> &str {
        //~^ unnecessary_literal_bound
        "Literal"
    }
}

impl ReturnsStr for Struct<'_> {
    // Should NOT warn
    fn trait_method(&self) -> &str {
        self.not_literal
    }
}

fn main() {}
