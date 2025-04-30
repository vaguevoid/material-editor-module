use std::process::ExitCode;
use std::{env::var_os, path::PathBuf};

fn main() -> ExitCode {
    println!("Performing FFI codegen...");
    if let Err(error) = build_tools::write_ffi(
        "transform_demo",
        &PathBuf::from(var_os("OUT_DIR").unwrap()),
        &std::env::current_dir().unwrap().join("src/lib.rs"),
        true,
    ) {
        eprintln!("{error}");
        ExitCode::FAILURE
    } else {
        println!("Codegen finished.");
        ExitCode::SUCCESS
    }
}
