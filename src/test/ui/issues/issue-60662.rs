// build-pass (FIXME(62277): could be check-pass?)
// compile-flags: -Z unpretty=hir

#![feature(type_alias_impl_trait)]

trait Animal {
}

fn main() {
    pub type ServeFut = impl Animal;
}
