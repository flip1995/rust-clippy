// check that we don't StorageDead booleans before they are used

fn main() {
    let mut should_break = false;
    loop {
        if should_break {
            break;
        }
        should_break = true;
    }
}

// END RUST SOURCE
// START rustc.main.SimplifyCfg-initial.after.mir
//     bb0: {
//         StorageLive(_1);
//         _1 = const false;
//         FakeRead(ForLet, _1);
//         goto -> bb2;
//     }
//     bb1: {
//         resume;
//     }
//     bb2: {
//         falseUnwind -> [real: bb3, cleanup: bb1];
//     }
//     bb3: {
//         StorageLive(_4);
//         _4 = _1;
//         switchInt(move _4) -> [false: bb5, otherwise: bb4];
//     }
//     bb4: {
//         _0 = ();
//         StorageDead(_4);
//         StorageDead(_1);
//         return;
//     }
//     bb5: {
//         _3 = ();
//         StorageDead(_4);
//         _1 = const true;
//         _2 = ();
//         goto -> bb2;
//     }
// END rustc.main.SimplifyCfg-initial.after.mir
