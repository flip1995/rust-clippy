// aux-build:intra-link-extern-crate.rs

// When loading `extern crate` statements, we would pull in their docs at the same time, even
// though they would never actually get displayed. This tripped intra-doc-link resolution failures,
// for items that aren't under our control, and not actually getting documented!

#![deny(intra_doc_link_resolution_failure)]

extern crate inner;
