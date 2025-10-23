// Copyright (C) 2025 Piers Finlayson <piers@piers.rocks>
//
// MIT License

// Only runs on Windows
#[cfg(not(target_os = "windows"))]
compile_error!("This crate only runs on Windows x86 64-bit.");

fn main() {
    println!("cargo:rerun-if-changed=libwdi");
    println!("cargo:rerun-if-changed=build.rs");

    println!("cargo:rustc-link-search=native=libwdi\\lib");
    println!("cargo:rustc-link-lib=advapi32");
    println!("cargo:rustc-link-lib=user32");
    println!("cargo:rustc-link-lib=ole32");
    println!("cargo:rustc-link-lib=setupapi");
    println!("cargo:rustc-link-lib=newdev");
    println!("cargo:rustc-link-lib=shell32");
}