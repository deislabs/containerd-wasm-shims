use lunatic::{spawn_link, Mailbox};

#[lunatic::main]
fn main(_: Mailbox<()>) {
    let child = spawn_link!(@task || {
        println!("Hello world from a process!");
    });
    // Wait for child to finish
    let _ = child.result();

    let child = spawn_link!(@task || {
        println!("Hello world from a 2nd process!");
    });
    // Wait for child to finish
    let _ = child.result();
}