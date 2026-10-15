//! Embeds the DevPanel icon into both Windows executables, so Explorer, the
//! taskbar and Start menu shortcuts show it. scripts/render-icons.ps1 renders
//! the `.ico` from assets/devpanel.svg.

fn main() {
    println!("cargo:rerun-if-changed=assets/devpanel.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    winresource::WindowsResource::new()
        .set_icon("assets/devpanel.ico")
        .compile()
        .expect("embedding the Windows icon");
}
