fn main() {
    // Rebuild trigger for icon change
    println!("cargo:rerun-if-changed=icons/icon.ico");
    tauri_build::build()
}
