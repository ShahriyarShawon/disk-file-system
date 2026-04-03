use std::env;

mod dfs;
use crate::dfs::{FSController, FSError};

fn main() {
    // let mut buffer = String::new();
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        eprintln!("usage: [CREATE|USE] {{file_name}}");
    }
    let _command = args[1].clone();
    let fname = args[2].clone();

    println!("We are using {fname} as our disk");

    let mut fs_controller = FSController::new(&fname);
    fs_controller.instantiate_disk().unwrap();
    match fs_controller.sync() {
        Err(FSError::Simple(s)) => eprintln!("{s}"),
        Err(FSError::BinRw(b)) => eprintln!("{b}"),
        Ok(_) => {}
    }

    // loop {
    //     buffer.clear();
    //     if let Err(e) = io::stdin().read_line(&mut buffer) {
    //         eprintln!("Error reading {e}");
    //         return;
    //     }
    // }
}
