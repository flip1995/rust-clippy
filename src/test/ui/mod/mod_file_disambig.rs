mod mod_file_disambig_aux; //~ ERROR file for module `mod_file_disambig_aux` found at both

fn main() {
    assert_eq!(mod_file_aux::bar(), 10);
    //~^ ERROR failed to resolve: use of undeclared type or module `mod_file_aux`
}
