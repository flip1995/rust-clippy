// We must mark a variable whose initialization fails due to an
// abort statement as StorageDead.

fn main() {
    loop {
        let beacon = {
            match true {
                false => 4,
                true => break,
            }
        };
        drop(&beacon);
    }
}

// END RUST SOURCE
// START rustc.main.mir_map.0.mir
// fn main() -> (){
//     let mut _0: ();
//     scope 1 {
//     }
//     scope 2 {
//         let _2: i32;
//     }
//     let mut _1: ();
//     let mut _3: bool;
//     let mut _4: !;
//     let mut _5: ();
//     let mut _6: &i32;
//     bb0: {
//         goto -> bb1;
//     }
//     bb1: {
//         falseUnwind -> [real: bb3, cleanup: bb4];
//     }
//     bb2: {
//         goto -> bb20;
//     }
//     bb3: {
//         StorageLive(_2);
//         StorageLive(_3);
//         _3 = const true;
//         FakeRead(ForMatchedPlace, _3);
//         switchInt(_3) -> [false: bb9, otherwise: bb8];
//     }
//     bb4: {
//         resume;
//     }
//     bb5: {
//         falseEdges -> [real: bb12, imaginary: bb6];
//     }
//     bb6: {
//         falseEdges -> [real: bb14, imaginary: bb7];
//     }
//     bb7: {
//         unreachable;
//     }
//     bb8: {
//         goto -> bb6;
//     }
//     bb9: {
//         goto -> bb5;
//     }
//     bb10: {
//         FakeRead(ForLet, _2);
//         StorageDead(_3);
//         StorageLive(_6);
//         _6 = &_2;
//         _5 = const std::mem::drop(move _6) -> [return: bb19, unwind: bb4];
//     }
//     bb11: {
//         _2 = const 4i32;
//         goto -> bb10;
//     }
//     bb12: {
//         goto -> bb11;
//     }
//     bb13: {
//         _0 = ();
//         goto -> bb15;
//     }
//     bb14: {
//         goto -> bb13;
//     }
//     bb15: {
//         StorageDead(_3);
//         goto -> bb16;
//     }
//     bb16: {
//         StorageDead(_2);
//         goto -> bb2;
//     }
//     bb17: {
//         _4 = ();
//         unreachable;
//     }
//     bb18: {
//         StorageDead(_4);
//         goto -> bb10;
//     }
//     bb19: {
//         StorageDead(_6);
//         _1 = ();
//         StorageDead(_2);
//         goto -> bb1;
//     }
//     bb20: {
//         return;
//     }
// }
// END rustc.main.mir_map.0.mir
