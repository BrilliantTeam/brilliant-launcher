fn main() {
    println!("cargo:rerun-if-env-changed=BRILLIANT_PLAY");
    tauri_build::build()
}
