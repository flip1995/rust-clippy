fn main() {}

extern "C" {
    default!(); //~ ERROR cannot find macro `default` in this scope
    default do
    //~^ ERROR `default` not followed by an item
    //~| ERROR non-item in item list
}
