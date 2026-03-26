use crate::client::run;

mod client;
mod kadem;

fn main() {
    //TODO: this is only here to silence annoying warnings
    run(std::path::PathBuf::new(), None).expect("this should fail");
    println!("Hello, world!");
}
