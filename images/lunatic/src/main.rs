use std::env;

fn main() {
    print!("Hello, world!\n");

    let args: Vec<String> = env::args().collect();
    println!("{:?}", args);

    for (key, value) in env::vars() {
        println!("{}: {}", key, value);
    }
}