extern crate bindgen;

use std::env;
use std::path::PathBuf;

fn main() {
    if std::env::var("CARGO_PRIMARY_PACKAGE").is_err() {
        println!("cargo:warning=Skipping build.rs because this is a dependency.");
        return;
    }
    
    println!("cargo:rustc-link-search={}", "src/c_lib");
    println!("cargo:rustc-link-lib=convenience");
    println!("cargo:rustc-link-search={}", "/usr/local/lib");
    println!("cargo:rustc-link-lib=rtlsdr");

    println!("cargo:rerun-if-changed=wrapper.h");

    let bindings = bindgen::Builder::default()
        .header("wrapper.h")
        .parse_callbacks(Box::new(bindgen::CargoCallbacks))
        .generate()
        .expect("Unable to generate bindings");

    let output_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(output_path.join("bindings.rs"))
        .expect("Could not write bindings");
}
