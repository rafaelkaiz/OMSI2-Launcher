//! OMSI's option presets (`option_presets\*.oop`): same format as options.cfg. Loading one only changes the in-memory options;
//! nothing is written until the user saves. Like in OMSI itself, a preset sets everything it contains: also the language, the last map
//! and driver, the radio and the weather station.

use crate::cfg::Options;
use crate::i18n::{tr, trf};
use std::path::{Path, PathBuf};

pub struct Preset {
    pub name: String,
    /// (key, section header word, value lines)
    pub entries: Vec<(String, String, Vec<String>)>,
}


pub fn list(omsi: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(omsi.join("option_presets"))
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("oop")))
        .collect();
    v.sort_by_key(|p| p.file_name().map(|n| n.to_string_lossy().to_lowercase()));
    v
}

pub fn read(path: &Path) -> Option<Preset> {
    let bytes = std::fs::read(path).ok()?;
    let text: String = bytes.iter().map(|&b| b as char).collect();
    let mut entries: Vec<(String, String, Vec<String>)> = vec![];
    let mut section = String::new();
    let mut open = false;
    for l in text.lines().map(|l| l.trim_end_matches('\r')) {
        let t = l.trim();
        if t.contains("----") {
            section = t.split_whitespace().next().unwrap_or("").to_string();
            open = false;
        } else if t.starts_with('[') && t.ends_with(']') {
            entries.push((t[1..t.len() - 1].to_string(), section.clone(), vec![]));
            open = true;
        } else if t.is_empty() {
            open = false;
        } else if open {
            if let Some(e) = entries.last_mut() {
                e.2.push(t.to_string());
            }
        }
    }
    let name = path.file_stem()?.to_string_lossy().into_owned();
    Some(Preset { name, entries })
}

/// Applies the preset to the in-memory options. `managed_flags` are the on/off keys the launcher knows about: the ones the preset
/// does not mention are switched off, because a preset describes the complete state.
pub fn apply(options: &mut Options, preset: &Preset, managed_flags: &[&str]) {
    for (key, section, values) in &preset.entries {
        // OMSI does not take these from a preset (checked with a preset that had other values)
        if IGNORED_BY_OMSI.iter().any(|k| k.eq_ignore_ascii_case(key)) {
            continue;
        }
        if values.is_empty() {
            options.set_flag(key, section, true);
        } else {
            for (i, v) in values.iter().enumerate() {
                options.set(key, section, i, v);
            }
        }
    }
    for k in managed_flags {
        if !preset.entries.iter().any(|(pk, _, _)| pk.eq_ignore_ascii_case(k)) {
            options.set_flag(k, "", false);
        }
    }
    // These values exist in options.cfg only while they are in use (a radio link, a manual screen ratio): a preset without them
    // means "none", so what was there before must not stay.
    for k in OPTIONAL_VALUES {
        if !preset.entries.iter().any(|(pk, _, _)| pk.eq_ignore_ascii_case(k)) {
            options.remove(k);
        }
    }
}

const OPTIONAL_VALUES: [&str; 2] = ["radio", "screenratio"];

/// Settings the game itself saves while playing: checked in OMSI, loading a preset leaves them as they are (`texture` is the one of
/// this kind that OMSI does take from a preset). `gamectrleron` (game controller on) belongs to the computer, not to the preset: with a
/// wheel connected OMSI turns it on by itself, so a preset must not switch it.
const IGNORED_BY_OMSI: [&str; 6] = ["nopreview", "texFilter", "font_typewriter", "Editor_LinkAerial", "no_tex_low_high_switch", "gamectrleron"];

/// Saves the current options as a NEW preset `option_presets\<name>.oop`. Never overwrites an existing preset.
pub fn save_new(omsi: &Path, name: &str, options: &Options) -> Result<PathBuf, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err(tr("Escribe un nombre para el preset.").into());
    }
    if !crate::cfg::valid_name(name) {
        return Err(tr("Nombre no válido (no puede estar vacío ni contener \\ / : * ? \" < > | [ ]).").into());
    }
    let dir = omsi.join("option_presets");
    let path = dir.join(format!("{name}.oop"));
    if path.exists() {
        return Err(tr("Ya existe un preset con ese nombre.").into());
    }
    std::fs::create_dir_all(&dir).map_err(|e| trf("No se pudo guardar el preset: {}", &[&e.to_string()]))?;
    // create_new fails if the file appeared in the meantime, so an existing preset can never be replaced
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .map_err(|e| trf("No se pudo guardar el preset: {}", &[&e.to_string()]))?;
    std::io::Write::write_all(&mut f, &options.to_bytes()).map_err(|e| trf("No se pudo guardar el preset: {}", &[&e.to_string()]))?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saving_a_preset_never_overwrites() {
        let real = Path::new(r"C:\Program Files (x86)\Steam\steamapps\common\OMSI 2");
        let Some(o) = Options::load(real) else { return };
        let d = std::env::temp_dir().join(format!("omsi2-preset-test-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let p = save_new(&d, "Mi preset", &o).unwrap();
        assert_eq!(std::fs::read(&p).unwrap(), o.to_bytes());
        assert!(save_new(&d, "Mi preset", &o).is_err());
        assert!(save_new(&d, "a/b", &o).is_err());
        assert!(save_new(&d, "  ", &o).is_err());
        // and it reads back as a preset
        assert!(read(&p).unwrap().entries.iter().any(|e| e.0 == "maxFPS"));
    }

    #[test]
    fn preset_sets_everything_it_contains() {
        let omsi = Path::new(r"C:\Program Files (x86)\Steam\steamapps\common\OMSI 2");
        // a preset that OMSI itself installs (the "PC 20xx" ones); add-on presets are not on every computer
        let Some(p) = list(omsi).into_iter().find(|p| p.file_stem().is_some_and(|n| n.to_string_lossy().starts_with("PC 20"))) else { return };
        let preset = read(&p).unwrap();
        assert!(preset.entries.iter().any(|e| e.0 == "performance_maxObjDist"));
        let mut o = Options::load(omsi).unwrap();
        let (map, language) = (preset.entries.iter().find(|e| e.0 == "last_map").map(|e| e.2[0].clone()), preset.entries.iter().find(|e| e.0 == "language").map(|e| e.2[0].clone()));
        let want = preset.entries.iter().find(|e| e.0 == "performance_maxObjDist").unwrap().2[0].clone();
        apply(&mut o, &preset, &["sunglow", "noAutoSave", "useActDate"]);
        assert_eq!(o.get("performance_maxObjDist", 0), Some(want));
        assert_eq!(o.get("last_map", 0), map);
        assert_eq!(o.get("language", 0), language);
    }
}

#[cfg(test)]
mod optional_tests {
    use super::*;

    #[test]
    fn a_preset_without_a_radio_link_removes_the_current_one() {
        let text = "[radio]\nhttp://example.test/radio\n\n[maxFPS]\n60\n";
        let d = std::env::temp_dir().join(format!("omsi2l-radio-{}-{}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|t| t.as_nanos()).unwrap_or(0)));
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("options.cfg"), text).unwrap();
        let mut o = Options::load(&d).unwrap();
        let preset = Preset { name: "x".into(), entries: vec![("maxFPS".into(), "GRAPHICS".into(), vec!["30".into()])] };
        apply(&mut o, &preset, &[]);
        assert!(!o.has("radio"));
        assert_eq!(o.get("maxFPS", 0).as_deref(), Some("30"));
        // and one that has a link sets it
        let with = Preset { name: "y".into(), entries: vec![("radio".into(), "GENERAL".into(), vec!["http://other.test".into()])] };
        apply(&mut o, &with, &[]);
        assert_eq!(o.get("radio", 0).as_deref(), Some("http://other.test"));
    }
}

#[cfg(test)]
mod ignored_tests {
    use super::*;

    #[test]
    fn a_preset_does_not_turn_nopreview_on() {
        let d = std::env::temp_dir().join(format!("omsi2l-nopreview-{}-{}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|t| t.as_nanos()).unwrap_or(0)));
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("options.cfg"), "[maxFPS]\n60\n").unwrap();
        let mut o = Options::load(&d).unwrap();
        let preset = Preset {
            name: "x".into(),
            entries: vec![("nopreview".into(), "GENERAL".into(), vec![]), ("maxFPS".into(), "GRAPHICS".into(), vec!["30".into()])],
        };
        apply(&mut o, &preset, &[]);
        assert!(!o.has("nopreview"));
        assert_eq!(o.get("maxFPS", 0).as_deref(), Some("30"));
    }
}
