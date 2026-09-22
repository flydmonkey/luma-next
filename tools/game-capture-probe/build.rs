fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        println!("cargo:rerun-if-changed=probe.c");
        println!("cargo:rustc-link-lib=d3d11");
        println!("cargo:rustc-link-lib=dxgi");
        println!("cargo:rustc-link-lib=dxguid");
        println!("cargo:rustc-link-lib=user32");
        cc::Build::new().file("probe.c").compile("luma_game_probe");
    }
}
