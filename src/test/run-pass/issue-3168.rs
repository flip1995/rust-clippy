// xfail-fast

fn main() {
    let (c,p) = pipes::stream();
    do task::try |move c| {
        let (c2,p2) = pipes::stream();
        do task::spawn |move p2| {
            p2.recv();
            error!("sibling fails");
            fail;
        }   
        let (c3,p3) = pipes::stream();
        c.send(move c3);
        c2.send(());
        error!("child blocks");
        p3.recv();
    };  
    error!("parent tries");
    assert !p.recv().try_send(());
    error!("all done!");
}
