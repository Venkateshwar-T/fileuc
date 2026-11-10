use std::env;
use std::path::PathBuf;

fn main() {
    if env::var("CARGO_CFG_TARGET_OS").unwrap_or_default() == "windows" {
        let mut res = winres::WindowsResource::new();
        res.set_icon("icon.ico");
        res.set("ProductName", "FileUC");
        res.set("FileDescription", "FileUC - Universal File Converter");
        res.set("LegalCopyright", "Copyright (C) 2026");

        let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
        if let Err(e) = res.compile() {
            eprintln!("Warning: Failed to compile Windows resources: {}", e);
        }

        // On GNU toolchains (x86_64-pc-windows-gnu), ld strips static libraries
        // without referenced symbols. Passing the object file directly guarantees
        // the .rsrc section is linked into the final .exe.
        let obj_file = out_dir.join("resource.o");
        if obj_file.exists() {
            println!("cargo:rustc-link-arg={}", obj_file.display());
        }
    }
}
