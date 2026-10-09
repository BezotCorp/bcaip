//launcher service entry point need to launch the services
//for now only the storage service is launched
use std::process;

fn main() {
    //launch the binary storage with cargo
    process::Command::new("cargo")
        .args(["run", "--bin", "storage"])
        .spawn()
        .expect("Failed to launch storage service");
}
