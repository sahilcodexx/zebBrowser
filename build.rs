fn main() {
    let libs = pkg_config::Config::new()
        .atleast_version("2.50")
        .probe("webkitgtk-6.0")
        .expect("webkitgtk-6.0 development files not found. Install webkitgtk-6.0.");

    for path in &libs.include_paths {
        println!("cargo:include={}", path.display());
    }
    for path in &libs.link_paths {
        println!("cargo:rustc-link-search=native={}", path.display());
    }
    for lib in &libs.libs {
        println!("cargo:rustc-link-lib={}", lib);
    }
    println!("cargo:rerun-if-changed=build.rs");
}
