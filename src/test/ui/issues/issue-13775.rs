// build-pass (FIXME(62277): could be check-pass?)
// pretty-expanded FIXME #23616

trait Foo {
    #[allow(anonymous_parameters)]
    fn bar(&self, isize) {}
}

fn main() {}
