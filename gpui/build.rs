fn main() {
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os == "windows" {
        let manifest_dir = std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
        let rc_path = manifest_dir.join("icon.rc");
        let icon_path = manifest_dir.join("icon.ico");
        println!("cargo:rerun-if-changed={}", rc_path.display());
        println!("cargo:rerun-if-changed={}", icon_path.display());
        let _ = embed_resource::compile(&rc_path, embed_resource::NONE);
    }
}