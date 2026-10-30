//! Editing of the two kinds of user data the launcher may write: OMSI's `options.cfg` and the driver files in `Drivers\`.
//! Every write first copies the current file to `%APPDATA%\OMSI2-Launcher\backups`, and nothing is written while OMSI is running
//! (OMSI rewrites both files itself when it exits).

use crate::i18n::{tr, trf};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub fn backup_dir() -> PathBuf {
    let base = std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
    base.join("OMSI2-Launcher").join("backups")
}

fn stamp() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// Copies `file` into the backup folder as `<stamp>-<file name>`.
pub fn backup(file: &Path) -> Result<PathBuf, String> {
    let dir = backup_dir();
    std::fs::create_dir_all(&dir).map_err(|e| trf("No se pudo crear la carpeta de copias: {}", &[&e.to_string()]))?;
    let name = file.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let millis = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0);
    let dest = dir.join(format!("{millis:013}-{name}"));
    std::fs::copy(file, &dest).map_err(|e| trf("No se pudo hacer la copia de seguridad: {}", &[&e.to_string()]))?;
    prune_backups(3, 0);
    Ok(dest)
}

/// Newest backup of `file_name` (options.cfg or a driver file), if any.
pub fn latest_backup(file_name: &str) -> Option<PathBuf> {
    let suffix = format!("-{file_name}");
    std::fs::read_dir(backup_dir())
        .ok()?
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().ends_with(&suffix))
        .max_by_key(|e| e.file_name())
        .map(|e| e.path())
}

pub fn restore(backup_file: &Path, target: &Path) -> Result<(), String> {
    std::fs::copy(backup_file, target).map(|_| ()).map_err(|e| trf("No se pudo restaurar: {}", &[&e.to_string()]))
}

/// Keeps the newest `keep` backups of each file and deletes the others once they are older than `max_age_days`.
/// Only touches the launcher's own backup folder. Returns how many files were deleted.
pub fn prune_backups(keep: usize, max_age_days: u64) -> usize {
    prune_in(&backup_dir(), keep, max_age_days, SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0))
}

fn prune_in(dir: &Path, keep: usize, max_age_days: u64, now_ms: u64) -> usize {
    let Ok(rd) = std::fs::read_dir(dir) else { return 0 };
    // group by original file name; the backup name is `<stamp>-<name>`
    let mut groups: std::collections::HashMap<String, Vec<(u64, PathBuf)>> = std::collections::HashMap::new();
    for e in rd.flatten() {
        let fname = e.file_name().to_string_lossy().into_owned();
        let Some((stamp, name)) = fname.split_once('-') else { continue };
        let Ok(stamp) = stamp.parse::<u64>() else { continue };
        let ms = if stamp < 1_000_000_000_000 { stamp * 1000 } else { stamp }; // older copies used seconds
        groups.entry(name.to_string()).or_default().push((ms, e.path()));
    }
    let limit = max_age_days * 86_400_000;
    let mut deleted = 0;
    for (_, mut files) in groups {
        files.sort_by(|a, b| b.0.cmp(&a.0));
        for (ms, path) in files.into_iter().skip(keep) {
            if now_ms.saturating_sub(ms) > limit && std::fs::remove_file(path).is_ok() {
                deleted += 1;
            }
        }
    }
    deleted
}

// ---------------------------------------------------------------------------------------------------------------- options.cfg

/// `options.cfg` kept as its original lines, so everything the launcher does not touch is written back unchanged.
#[derive(Clone)]
pub struct Options {
    path: PathBuf,
    lines: Vec<String>,
}

impl Options {
    pub fn load(omsi: &Path) -> Option<Self> {
        let path = omsi.join("options.cfg");
        let bytes = std::fs::read(&path).ok()?;
        let text: String = bytes.iter().map(|&b| b as char).collect(); // ANSI
        let lines = text.split('\n').map(|l| l.trim_end_matches('\r').to_string()).collect();
        Some(Self { path, lines })
    }

    fn find(&self, key: &str) -> Option<usize> {
        let tag = format!("[{key}]");
        self.lines.iter().position(|l| l.trim().eq_ignore_ascii_case(&tag))
    }

    pub fn has(&self, key: &str) -> bool {
        self.find(key).is_some()
    }

    /// The `idx`-th value line under `[key]`.
    pub fn get(&self, key: &str, idx: usize) -> Option<String> {
        let at = self.find(key)? + 1 + idx;
        let l = self.lines.get(at)?.trim();
        (!l.is_empty() && !l.starts_with('[')).then(|| l.to_string())
    }

    /// Removes `[key]` together with its value lines and the blank line after them.
    pub fn remove(&mut self, key: &str) {
        let Some(i) = self.find(key) else { return };
        let mut end = i + 1;
        while self.lines.get(end).is_some_and(|l| !l.trim().is_empty() && !l.trim().starts_with('[')) {
            end += 1;
        }
        if self.lines.get(end).is_some_and(|l| l.trim().is_empty()) {
            end += 1;
        }
        self.lines.drain(i..end);
    }

    /// Turns a flag key (a bare `[key]` with no value) on or off. A new flag goes right under the `section` header.
    pub fn set_flag(&mut self, key: &str, section: &str, on: bool) {
        match (self.find(key), on) {
            (Some(i), false) => {
                self.lines.remove(i);
                if self.lines.get(i).is_some_and(|l| l.trim().is_empty()) {
                    self.lines.remove(i);
                }
            }
            (None, true) => {
                let at = self.lines.iter().position(|l| l.contains(section)).map_or(self.lines.len(), |p| p + 1);
                self.lines.splice(at..at, [String::new(), format!("[{key}]")]);
            }
            _ => {}
        }
    }

    /// Sets the `idx`-th value line under `[key]`, creating the key under `section` if it does not exist.
    pub fn set(&mut self, key: &str, section: &str, idx: usize, value: &str) {
        let start = match self.find(key) {
            Some(i) => i,
            None => {
                let at = self.lines.iter().position(|l| l.contains(section)).map_or(self.lines.len(), |p| p + 1);
                self.lines.splice(at..at, [String::new(), format!("[{key}]"), String::new()]);
                at + 1
            }
        };
        let at = start + 1 + idx;
        while self.lines.len() <= at {
            self.lines.push(String::new());
        }
        // A new value line must not swallow the blank separator before the next key.
        if self.lines[at].trim().is_empty() || self.lines[at].trim().starts_with('[') {
            self.lines.insert(at, value.to_string());
        } else {
            self.lines[at] = value.to_string();
        }
    }

    /// The file as OMSI writes it (ANSI, CRLF).
    pub fn to_bytes(&self) -> Vec<u8> {
        self.lines.join("\r\n").chars().map(|c| if (c as u32) < 256 { c as u8 } else { b'?' }).collect()
    }

    pub fn save(&self) -> Result<(), String> {
        backup(&self.path)?;
        std::fs::write(&self.path, self.to_bytes()).map_err(|e| trf("No se pudo guardar options.cfg: {}", &[&e.to_string()]))
    }
}

// ---------------------------------------------------------------------------------------------------------------- drivers

#[derive(Clone, PartialEq)]
pub struct Driver {
    pub name: String,
    pub female: bool,
    /// `yyyymmdd`
    pub birth: String,
    pub path: PathBuf,
    pub hektometers: u64,
    pub stops: u64,
}

fn read_utf16(path: &Path) -> Option<String> {
    let b = std::fs::read(path).ok()?;
    let b = b.strip_prefix(&[0xFF, 0xFE]).unwrap_or(&b);
    let u: Vec<u16> = b.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
    Some(String::from_utf16_lossy(&u))
}

fn write_utf16(path: &Path, text: &str) -> Result<(), String> {
    let mut b = vec![0xFF, 0xFE];
    for u in text.encode_utf16() {
        b.extend(u.to_le_bytes());
    }
    std::fs::write(path, b).map_err(|e| trf("No se pudo escribir el conductor: {}", &[&e.to_string()]))
}

fn tag_line(lines: &[&str], tag: &str) -> Option<usize> {
    lines.iter().position(|l| l.trim() == tag)
}

pub fn read_driver(path: &Path) -> Option<Driver> {
    let text = read_utf16(path)?;
    let lines: Vec<&str> = text.lines().collect();
    let i = tag_line(&lines, "[ident]")?;
    let num = |tag: &str, off: usize| tag_line(&lines, tag).and_then(|p| lines.get(p + 1 + off)?.trim().parse().ok()).unwrap_or(0);
    Some(Driver {
        name: lines.get(i + 1)?.trim().to_string(),
        female: lines.get(i + 2)?.trim().eq_ignore_ascii_case("W") || lines.get(i + 2)?.trim().eq_ignore_ascii_case("F"),
        birth: lines.get(i + 3)?.trim().to_string(),
        path: path.to_path_buf(),
        hektometers: num("[hektom]", 0),
        stops: num("[busstops]", 0),
    })
}

pub fn list_drivers(omsi: &Path) -> Vec<Driver> {
    let mut v: Vec<Driver> = std::fs::read_dir(omsi.join("Drivers"))
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().ends_with(".odr"))
        .filter_map(|e| read_driver(&e.path()))
        .collect();
    v.sort_by_key(|d| d.name.to_lowercase());
    v
}

pub fn valid_birth(s: &str) -> bool {
    s.len() == 8 && s.chars().all(|c| c.is_ascii_digit()) && {
        let (m, d) = (s[4..6].parse::<u32>().unwrap_or(0), s[6..8].parse::<u32>().unwrap_or(0));
        (1..=12).contains(&m) && (1..=31).contains(&d)
    }
}

pub fn valid_name(n: &str) -> bool {
    !n.trim().is_empty() && !n.contains(['\\', '/', ':', '*', '?', '"', '<', '>', '|', '[', ']'])
}

/// (year, month, day) in UTC from the system clock.
pub fn today() -> (i64, u32, u32) {
    let days = (stamp() / 86_400) as i64;
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}

fn sex_letter(female: bool) -> &'static str {
    if female { "W" } else { "M" }
}

/// Creates `Drivers\<name>.odr` with empty statistics, like OMSI does for a new driver.
pub fn create_driver(omsi: &Path, name: &str, female: bool, birth: &str) -> Result<(), String> {
    let name = name.trim();
    if !valid_name(name) {
        return Err(tr("Nombre no válido (no puede estar vacío ni contener \\ / : * ? \" < > | [ ]).").into());
    }
    if !valid_birth(birth) {
        return Err(tr("La fecha de nacimiento debe ser aaaammdd (por ejemplo 19950606).").into());
    }
    let path = omsi.join("Drivers").join(format!("{name}.odr"));
    if path.exists() {
        return Err(tr("Ya existe un conductor con ese nombre.").into());
    }
    let (y, m, d) = today();
    let text = format!(
        "-----------------------\r\nDriver File\r\n-----------------------\r\n\r\nCreated with Omsi Version 2.3.004\r\nDate: {d:02}/{m:02}/{y}\r\n\r\n\
         [ident]\r\n{name}\r\n{}\r\n{birth}\r\n{y}{m:02}{d:02}\r\n\r\n[busstops]\r\n0\r\n0\r\n0\r\n\r\n[hektom]\r\n0\r\n\r\n\
         [crashs]\r\n0\r\n0\r\n0\r\n0\r\n\r\n[tickets]\r\n0\r\n0\r\n\r\n[rating]\r\n0\r\n0\r\n0\r\n0\r\n0\r\n\r\n",
        sex_letter(female)
    );
    write_utf16(&path, &text)
}

/// Changes name, sex and birth date of an existing driver, keeping all its statistics. Renames the file if the name changed.
pub fn update_driver(old: &Driver, name: &str, female: bool, birth: &str) -> Result<(), String> {
    let name = name.trim();
    if !valid_name(name) {
        return Err(tr("Nombre no válido (no puede estar vacío ni contener \\ / : * ? \" < > | [ ]).").into());
    }
    if !valid_birth(birth) {
        return Err(tr("La fecha de nacimiento debe ser aaaammdd (por ejemplo 19950606).").into());
    }
    let text = read_utf16(&old.path).ok_or(tr("No se pudo leer el conductor."))?;
    let mut lines: Vec<String> = text.lines().map(str::to_string).collect();
    let i = lines.iter().position(|l| l.trim() == "[ident]").ok_or(tr("Archivo de conductor no válido."))?;
    if lines.len() < i + 4 {
        return Err(tr("Archivo de conductor no válido.").into());
    }
    let new_path = old.path.with_file_name(format!("{name}.odr"));
    if new_path != old.path && new_path.exists() {
        return Err(tr("Ya existe un conductor con ese nombre.").into());
    }
    backup(&old.path)?;
    lines[i + 1] = name.to_string();
    lines[i + 2] = sex_letter(female).to_string();
    lines[i + 3] = birth.to_string();
    write_utf16(&new_path, &(lines.join("\r\n") + "\r\n"))?;
    if new_path != old.path {
        std::fs::remove_file(&old.path).map_err(|e| trf("No se pudo quitar el archivo anterior: {}", &[&e.to_string()]))?;
    }
    Ok(())
}

/// Everything OMSI keeps in a driver's file, for the staff-file window.
#[derive(Default)]
pub struct Record {
    pub hired: String,
    pub stops: u64,
    pub late: u64,
    pub early: u64,
    pub hektometers: u64,
    /// accidents, hit and runs, heavy hit and runs, injured pedestrians
    pub crashes: [u64; 4],
    pub tickets_sold: u64,
    pub cash: f64,
    pub rating: [i64; 5],
    /// (bus, times driven)
    pub buses: Vec<(String, u64)>,
}

impl Record {
    /// Passenger comfort rating in percent (comfort points obtained / possible), if there is one.
    pub fn comfort(&self) -> Option<(f64, i64, i64)> {
        (self.rating[0] > 0).then(|| (self.rating[1] as f64 * 100.0 / self.rating[0] as f64, self.rating[1], self.rating[0]))
    }

    /// 100 % when the driver has no accidents or hit and runs at all; otherwise OMSI's formula is not known here.
    pub fn driving(&self) -> Option<f64> {
        (self.crashes.iter().all(|&c| c == 0) && self.rating[2..].iter().all(|&r| r == 0)).then_some(100.0)
    }
}

/// The lines under `tag` up to the next blank line or tag.
fn block<'a>(lines: &[&'a str], tag: &str) -> Vec<&'a str> {
    let Some(i) = lines.iter().position(|l| l.trim() == tag) else { return vec![] };
    lines[i + 1..].iter().map(|l| l.trim()).take_while(|l| !l.is_empty() && !l.starts_with('[')).collect()
}

pub fn read_record(path: &Path) -> Option<Record> {
    let text = read_utf16(path)?;
    let lines: Vec<&str> = text.lines().collect();
    let num = |tag: &str, n: usize| block(&lines, tag).get(n).and_then(|s| s.parse::<u64>().ok()).unwrap_or(0);
    let ident = block(&lines, "[ident]");
    let mut rec = Record {
        hired: ident.get(3).map(|s| s.to_string()).unwrap_or_default(),
        stops: num("[busstops]", 0),
        late: num("[busstops]", 1),
        early: num("[busstops]", 2),
        hektometers: num("[hektom]", 0),
        crashes: [num("[crashs]", 0), num("[crashs]", 1), num("[crashs]", 2), num("[crashs]", 3)],
        tickets_sold: num("[tickets]", 0),
        cash: block(&lines, "[tickets]").get(1).and_then(|s| s.parse().ok()).unwrap_or(0.0),
        ..Record::default()
    };
    for (n, r) in rec.rating.iter_mut().enumerate() {
        *r = block(&lines, "[rating]").get(n).and_then(|s| s.parse().ok()).unwrap_or(0);
    }
    // each [perbusinfo] is the bus name (one or more lines) followed by how many times it was driven
    let mut i = 0;
    while i < lines.len() {
        if lines[i].trim() == "[perbusinfo]" {
            let body: Vec<&str> = lines[i + 1..].iter().map(|l| l.trim()).take_while(|l| !l.is_empty() && !l.starts_with('[')).collect();
            if let Some((count, name)) = body.split_last() {
                rec.buses.push((name.join(" "), count.parse().unwrap_or(0)));
            }
        }
        i += 1;
    }
    Some(rec)
}

/// Deletes a driver's file after keeping a copy in the backup folder. The driver's photo, if any, is left where it is.
pub fn delete_driver(d: &Driver) -> Result<PathBuf, String> {
    let copy = backup(&d.path)?;
    std::fs::remove_file(&d.path).map_err(|e| trf("No se pudo quitar el archivo anterior: {}", &[&e.to_string()]))?;
    Ok(copy)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn staff_record_is_consistent() {
        // any driver file of the installed OMSI (nothing to check when OMSI is not installed)
        let Some(omsi) = crate::omsi::find_omsi_dir(None) else { return };
        let Some(p) = list_drivers(&omsi).first().map(|d| d.path.clone()) else { return };
        let Some(r) = read_record(&p) else { return };
        // the file is the real one and changes whenever the user drives, so only what must always hold is checked
        assert!(r.stops >= r.late + r.early);
        assert_eq!(r.hired.len(), 8);
        if let Some((pct, got, max)) = r.comfort() {
            assert!(got <= max && (pct - 100.0 * got as f64 / max as f64).abs() < 0.05);
        }
    }

    #[test]
    fn prune_keeps_the_newest_and_the_recent() {
        let d = sandbox("prune");
        let b = d.join("b");
        std::fs::create_dir_all(&b).unwrap();
        let day = 86_400_000u64;
        let now = 1_700_000_000_000u64;
        // 5 old copies of a.cfg (60..64 days old), 1 recent, plus one old copy of another file
        for i in 0..5u64 {
            std::fs::write(b.join(format!("{:013}-a.cfg", now - (60 + i) * day)), b"x").unwrap();
        }
        std::fs::write(b.join(format!("{:013}-a.cfg", now - day)), b"x").unwrap();
        std::fs::write(b.join(format!("{:013}-b.cfg", now - 90 * day)), b"x").unwrap();
        let n = prune_in(&b, 2, 30, now);
        assert_eq!(n, 4); // a.cfg: keeps newest 2 (recent + one 60-day), deletes 4 old; b.cfg keeps its only copy
        assert_eq!(std::fs::read_dir(&b).unwrap().count(), 3);
    }

    #[test]
    fn deleting_a_driver_keeps_a_backup() {
        let d = sandbox("del");
        unsafe { std::env::set_var("APPDATA", d.join("appdata")) };
        create_driver(&d, "Borrame", false, "19900101").unwrap();
        let drv = list_drivers(&d).remove(0);
        let copy = delete_driver(&drv).unwrap();
        assert!(!drv.path.exists() && copy.is_file());
        assert!(list_drivers(&d).is_empty());
    }

    fn sandbox(name: &str) -> PathBuf {
        // unique per process and per call: two runs within the same second must not share a folder
        let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map(|t| t.as_nanos()).unwrap_or(0);
        let d = std::env::temp_dir().join(format!("omsi2-launcher-test-{name}-{}-{nanos}", std::process::id()));
        std::fs::create_dir_all(d.join("Drivers")).unwrap();
        d
    }

    #[test]
    fn options_edit_keeps_everything_else() {
        let real = Path::new(r"C:\Program Files (x86)\Steam\steamapps\common\OMSI 2\options.cfg");
        let Ok(orig) = std::fs::read(real) else { return };
        let d = sandbox("opt");
        std::fs::write(d.join("options.cfg"), &orig).unwrap();
        unsafe { std::env::set_var("APPDATA", d.join("appdata")) };
        let mut o = Options::load(&d).unwrap();
        assert_eq!(o.get("maxFPS", 0).as_deref(), Some("60"));
        o.set("maxFPS", "GRAPHICS", 0, "120");
        o.set("texFilter", "GRAPHICS", 1, "8");
        o.set_flag("newflag1", "GRAPHICS", true);
        o.set_flag("newflag2", "GENERAL", true);
        o.set("brand_new_key", "GRAPHICS", 0, "7");
        o.save().unwrap();
        let o2 = Options::load(&d).unwrap();
        assert_eq!(o2.get("maxFPS", 0).as_deref(), Some("120"));
        assert_eq!(o2.get("texFilter", 0).as_deref(), Some("3"));
        assert_eq!(o2.get("texFilter", 1).as_deref(), Some("8"));
        assert!(o2.has("newflag1") && o2.has("newflag2") && o2.has("sunglow"));
        assert_eq!(o2.get("brand_new_key", 0).as_deref(), Some("7"));
        // turning flags back off and values back restores the original bytes
        let mut o3 = Options::load(&d).unwrap();
        o3.set("maxFPS", "GRAPHICS", 0, "60");
        o3.set("texFilter", "GRAPHICS", 1, "16");
        o3.set_flag("newflag1", "GRAPHICS", false);
        o3.set_flag("newflag2", "GENERAL", false);
        o3.set_flag("brand_new_key", "GRAPHICS", false);
        o3.save().unwrap();
        let back = std::fs::read(d.join("options.cfg")).unwrap();
        let text = |b: &[u8]| b.iter().map(|&c| c as char).collect::<String>();
        let strip = |s: String| s.replace("\r\n", "\n").lines().filter(|l| !l.trim().is_empty() && !l.contains("brand_new_key") && l.trim() != "7").collect::<Vec<_>>().join("\n");
        assert_eq!(strip(text(&back)), strip(text(&orig)));
        assert!(backup_dir().read_dir().unwrap().count() >= 2);
    }

    #[test]
    fn remove_drops_key_and_its_values() {
        let d = sandbox("rm");
        std::fs::write(d.join("options.cfg"), b"[a]
1
2

[b]

[c]
x
").unwrap();
        let mut o = Options::load(&d).unwrap();
        o.remove("a");
        assert!(!o.has("a") && o.has("b") && o.get("c", 0).as_deref() == Some("x"));
        o.remove("c");
        assert!(!o.has("c") && o.has("b"));
    }

    #[test]
    fn drivers_create_and_update() {
        let d = sandbox("drv");
        unsafe { std::env::set_var("APPDATA", d.join("appdata")) };
        create_driver(&d, "Test Uno", true, "19900203").unwrap();
        assert!(create_driver(&d, "Test Uno", true, "19900203").is_err());
        assert!(create_driver(&d, "Mal/Nombre", true, "19900203").is_err());
        assert!(create_driver(&d, "X", true, "1990-02-03").is_err());
        let v = list_drivers(&d);
        assert_eq!(v.len(), 1);
        assert!(v[0].female && v[0].birth == "19900203" && v[0].name == "Test Uno");
        update_driver(&v[0], "Test Dos", false, "19851231").unwrap();
        let v = list_drivers(&d);
        assert_eq!(v.len(), 1);
        assert!(!v[0].female && v[0].birth == "19851231" && v[0].name == "Test Dos");
        assert!(d.join("Drivers").join("Test Dos.odr").is_file() && !d.join("Drivers").join("Test Uno.odr").exists());
    }
}
