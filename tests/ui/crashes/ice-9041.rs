pub struct Thing;

pub fn has_thing(things: &[Thing]) -> bool {
    let is_thing_ready = |_peer: &Thing| -> bool { todo!() };
    things.iter().find(|p| is_thing_ready(p)).is_some()
}

fn main() {}
