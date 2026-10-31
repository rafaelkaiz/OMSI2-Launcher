//! The launcher's own settings (not OMSI's): kept in `%APPDATA%\OMSI2-Launcher\settings.ini`, never in the OMSI folder.

use crate::i18n::{trf, Lang};
use crate::theme::Theme;
use std::path::PathBuf;

pub struct Settings {
    pub theme: Theme,
    pub omsi_dir: Option<PathBuf>,
    pub windowed: bool,
    pub lang: Lang,
    /// Close the launcher once OMSI has started loading.
    pub close_on_start: bool,
    /// Keep the launcher window above other windows (also above OMSI while it opens).
    pub always_on_top: bool,
    /// Open the launcher full screen, with the interface centred like OMSI's own start screen (F11 toggles).
    pub fullscreen: bool,
    /// Delete the "closecheck" file OMSI leaves in its folder after a bad shutdown, when the launcher opens and before starting OMSI.
    pub remove_closecheck: bool,
}

fn file() -> PathBuf {
    let base = std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
    base.join("OMSI2-Launcher").join("settings.ini")
}

/// The Windows display language, if it is one the launcher has; English otherwise.
pub fn detect_lang() -> Lang {
    let mut buf = [0u16; 85];
    let n = unsafe { windows_sys::Win32::Globalization::GetUserDefaultLocaleName(buf.as_mut_ptr(), buf.len() as i32) };
    let name = String::from_utf16_lossy(&buf[..(n.max(1) as usize - 1).min(buf.len())]).to_lowercase();
    lang_from_locale(&name)
}

fn lang_from_locale(name: &str) -> Lang {
    match name.split(['-', '_']).next().unwrap_or("") {
        "es" => Lang::Es,
        "de" => Lang::De,
        "pl" => Lang::Pl,
        "cs" => Lang::Cs,
        "fr" => Lang::Fr,
        "pt" => Lang::Pt,
        _ => Lang::En,
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self { theme: Theme::Light, omsi_dir: None, windowed: false, lang: Lang::Es, close_on_start: true, always_on_top: true, fullscreen: true, remove_closecheck: true }
    }
}

impl Settings {
    /// Reads the settings. The first time (no file yet) the language is taken from Windows and the file is created.
    pub fn load() -> Self {
        let mut s = Self::default();
        let Ok(text) = std::fs::read_to_string(file()) else {
            s.lang = detect_lang();
            let _ = s.save();
            return s;
        };
        for l in text.lines() {
            let Some((k, v)) = l.split_once('=') else { continue };
            let v = v.trim();
            match k.trim() {
                "theme" => s.theme = Theme::from_key(v),
                "omsi_dir" if !v.is_empty() => s.omsi_dir = Some(PathBuf::from(v)),
                "windowed" => s.windowed = v == "1",
                "language" => s.lang = Lang::from_code(v),
                "close_on_start" => s.close_on_start = v != "0",
                "always_on_top" => s.always_on_top = v != "0",
                "fullscreen" => s.fullscreen = v != "0",
                "remove_closecheck" | "remove_nocheck" => s.remove_closecheck = v != "0", // the old name of the key is still read
                _ => {}
            }
        }
        s
    }

    pub fn save(&self) -> Result<(), String> {
        let path = file();
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| trf("No se pudo crear la carpeta de ajustes: {}", &[&e.to_string()]))?;
        }
        let text = format!(
            "theme={}\nlanguage={}\nomsi_dir={}\nwindowed={}\nclose_on_start={}\nalways_on_top={}\nfullscreen={}\nremove_closecheck={}\n",
            self.theme.key(),
            self.lang.code(),
            self.omsi_dir.as_ref().map(|p| p.display().to_string()).unwrap_or_default(),
            u8::from(self.windowed),
            u8::from(self.close_on_start),
            u8::from(self.always_on_top),
            u8::from(self.fullscreen),
            u8::from(self.remove_closecheck),
        );
        std::fs::write(path, text).map_err(|e| trf("No se pudieron guardar los ajustes: {}", &[&e.to_string()]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_windows_locales_to_launcher_languages() {
        assert_eq!(lang_from_locale("es-es"), Lang::Es);
        assert_eq!(lang_from_locale("es-mx"), Lang::Es);
        assert_eq!(lang_from_locale("pt-br"), Lang::Pt);
        assert_eq!(lang_from_locale("cs-cz"), Lang::Cs);
        assert_eq!(lang_from_locale("ja-jp"), Lang::En);
    }
}
