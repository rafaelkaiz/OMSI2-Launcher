//! The weather stations OMSI knows: `Weather\ICAO.txt`, one `CODE - City` per line (ISO-8859-1), read-only.

use std::path::Path;

pub fn stations(omsi: &Path) -> Vec<String> {
    let Ok(bytes) = std::fs::read(omsi.join("Weather").join("ICAO.txt")) else { return vec![] };
    let text: String = bytes.iter().map(|&b| b as char).collect();
    let mut v: Vec<String> = text.lines().map(|l| l.trim().to_string()).filter(|l| l.len() >= 4).collect();
    v.sort_by_key(|s| s.to_lowercase());
    v
}

/// The ICAO code at the start of a `CODE - City` line.
pub fn code(entry: &str) -> &str {
    entry.split_whitespace().next().unwrap_or("")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_real_station_list() {
        let omsi = Path::new(r"C:\Program Files (x86)\Steam\steamapps\common\OMSI 2");
        if !omsi.join("Weather").join("ICAO.txt").is_file() {
            return;
        }
        let v = stations(omsi);
        assert!(v.len() > 50);
        let berlin = v.iter().find(|s| code(s) == "EDDT").unwrap();
        assert_eq!(berlin, "EDDT - Berlin");
        assert!(v.iter().any(|s| s.contains("Brüssel")));
    }
}
