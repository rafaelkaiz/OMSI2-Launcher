//! Rules for the tests that read a real OMSI 2 installation, and a test that keeps them.
//!
//! The tests may use a real installation, but only what OMSI 2 itself installs: the maps Grundorf and Berlin-Spandau, their
//! situations, the "PC 20xx" presets, the languages in `Languages`, the tutorials and the weather list. Add-ons (other maps,
//! add-on presets, the user's drivers or options) are not on every computer, and neither are the values the user has chosen
//! (frame rate, sun glow, language...): a test must not expect them, it must read what is there and compare against that. When
//! what a test needs is not there, it returns without failing.

#[cfg(test)]
mod tests {
    /// Names of well-known add-ons and of this program's author: they must not appear in the code, tests included.
    const NOT_DEFAULT: [&str; 12] = [
        "X10 Berlin",
        "Wien ",
        "Novi Sad",
        "HafenCity",
        "St-Servan",
        "Wuppertal",
        "Hamburg",
        "Gladbeck",
        "Bremen-Nord",
        "Thüringer",
        "Ruhrgebiet",
        "Rafael C",
    ];

    macro_rules! sources {
        ($($name:literal),* $(,)?) => { [$(($name, include_str!(concat!($name, ".rs")))),*] };
    }

    #[test]
    fn code_does_not_rely_on_add_ons_or_on_the_user() {
        let files = sources![
            "app_icon", "cfg", "chrono", "i18n", "icons", "log", "logo", "main", "olf", "omsi", "options_layout", "options_ui", "paper", "pick", "plugin",
            "presets", "settings", "situations", "theme", "tutorials", "update", "weather"
        ];
        let mut found = vec![];
        for (name, text) in files {
            for (i, line) in text.lines().enumerate() {
                if let Some(word) = NOT_DEFAULT.iter().find(|w| line.contains(**w)) {
                    found.push(format!("src/{name}.rs:{}: {word}", i + 1));
                }
            }
        }
        assert!(found.is_empty(), "tests must only use what OMSI 2 installs (see src/testutil.rs):\n{}", found.join("\n"));
    }
}
