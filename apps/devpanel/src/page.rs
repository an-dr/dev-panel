//! The page shown in the web panel, assembled into one HTML document.
//!
//! The web panel takes a single HTML string, so the stylesheet and script
//! are inlined at their markers. They are compiled into the binary; setting
//! `DEVPANEL_WEB_DIR` to the `web/` folder reads them from disk instead, so
//! the UI can be edited and reloaded without rebuilding.

const INDEX: &str = include_str!("../web/index.html");
const STYLE: &str = include_str!("../web/style.css");
const SCRIPT: &str = include_str!("../web/app.js");
const ICONS: &str = include_str!("../web/icons.js");

pub fn html() -> String {
    let from_disk = std::env::var_os("DEVPANEL_WEB_DIR").map(std::path::PathBuf::from);
    let read = |name: &str, built_in: &str| -> String {
        from_disk
            .as_ref()
            .and_then(|dir| std::fs::read_to_string(dir.join(name)).ok())
            .unwrap_or_else(|| built_in.to_string())
    };
    let script = format!("{}\n{}", read("icons.js", ICONS), read("app.js", SCRIPT));
    read("index.html", INDEX)
        .replace("/*@STYLE@*/", &read("style.css", STYLE))
        .replace("/*@SCRIPT@*/", &script)
}
