//! The page shown in the web panel, assembled into one HTML document.
//!
//! The web panel takes a single HTML string, so the stylesheet and script
//! are inlined at their markers. They are compiled into the binary; setting
//! `DEVPANEL_WEB_DIR` to the `web/` folder reads them from disk instead, so
//! the UI can be edited and reloaded without rebuilding. The display fonts
//! for repository names ride along the same way, generated into `fonts.css`
//! and `fonts.js` by `scripts/build-fonts.ps1`.

const INDEX: &str = include_str!("../web/index.html");
const STYLE: &str = include_str!("../web/style.css");
const SCRIPT: &str = include_str!("../web/app.js");
const ICONS: &str = include_str!("../web/icons.js");
const FONT_FACES: &str = include_str!("../web/fonts.css");
const FONT_LIST: &str = include_str!("../web/fonts.js");

pub fn html() -> String {
    let from_disk = std::env::var_os("DEVPANEL_WEB_DIR").map(std::path::PathBuf::from);
    let read = |name: &str, built_in: &str| -> String {
        from_disk
            .as_ref()
            .and_then(|dir| std::fs::read_to_string(dir.join(name)).ok())
            .unwrap_or_else(|| built_in.to_string())
    };
    let script = format!("{}\n{}\n{}", read("icons.js", ICONS), read("fonts.js", FONT_LIST), read("app.js", SCRIPT));
    let style = format!("{}\n{}", read("fonts.css", FONT_FACES), read("style.css", STYLE));
    read("index.html", INDEX)
        .replace("/*@STYLE@*/", &style)
        .replace("/*@SCRIPT@*/", &script)
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_page_carries_the_display_fonts() {
        let page = super::html();
        assert!(page.contains("@font-face { font-family: 'dp-orbitron'"));
        assert!(page.contains("const FONTS = ["));
        assert!(page.find("const FONTS").unwrap() < page.find("const HEALTH").unwrap(), "the catalogue loads before the app");
    }
}
