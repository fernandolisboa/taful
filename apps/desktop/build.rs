//! Compiles `taful.rc` (the app icon) into `taful.exe`. Only a Windows build
//! has resources; for any other target there is nothing to do.

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=taful.rc");
    println!("cargo:rerun-if-changed=assets/icon/taful.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        // An executable without its icon still runs, but it would ship with
        // the wrong face: fail the build instead.
        embed_resource::compile("taful.rc", embed_resource::NONE)
            .manifest_required()
            .expect("compile taful.rc (needs a resource compiler: rc.exe from the Windows SDK)");
    }
}
