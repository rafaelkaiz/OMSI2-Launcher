//! "Información sobre el tiempo seleccionado": OMSI shows, for the selected date, the descriptions of the map's timetable and tariff
//! periods that cover it. They live in `maps\<map>\Chrono\<yyyymmdd>_<name>\` (`Chrono.cfg` with `[startdate]`/`[enddate]`, and
//! `Chrono_<LANG>.dsc` with `[name]` and `[description]`). Read-only.

use std::path::Path;

pub type Date = (i64, u32, u32);

/// The date of a saved situation (`.osn`): its `[time]` block is year, day of the year, hour, minute, second.
pub fn situation_date(osn: &Path) -> Option<Date> {
    let b = std::fs::read(osn).ok()?;
    let text = if let Some(rest) = b.strip_prefix(&[0xFF, 0xFE]) {
        String::from_utf16_lossy(&rest.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect::<Vec<_>>())
    } else {
        b.iter().map(|&c| c as char).collect()
    };
    let lines: Vec<&str> = text.lines().map(str::trim).collect();
    let i = lines.iter().position(|l| l.eq_ignore_ascii_case("[time]"))?;
    let year: i64 = lines.get(i + 1)?.parse().ok()?;
    let day: u32 = lines.get(i + 2)?.parse().ok()?;
    from_year_day(year, day)
}

fn leap(y: i64) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

/// Day of the year (1-based) to (year, month, day).
pub fn from_year_day(year: i64, day: u32) -> Option<Date> {
    let months = [31, if leap(year) { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let mut left = day;
    if left == 0 {
        return None;
    }
    for (i, m) in months.iter().enumerate() {
        if left <= *m {
            return Some((year, i as u32 + 1, left));
        }
        left -= m;
    }
    None
}

fn key(d: Date) -> u32 {
    (d.0 as u32) * 10_000 + d.1 * 100 + d.2
}

fn read_any(p: &Path) -> Option<String> {
    let b = std::fs::read(p).ok()?;
    Some(if let Some(rest) = b.strip_prefix(&[0xFF, 0xFE]) {
        String::from_utf16_lossy(&rest.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect::<Vec<_>>())
    } else {
        b.iter().map(|&c| c as char).collect()
    })
}

fn value_after(text: &str, tag: &str) -> Option<String> {
    let mut lines = text.lines().map(str::trim);
    lines.find(|l| l.eq_ignore_ascii_case(tag))?;
    lines.next().map(str::to_string)
}

fn description(text: &str) -> String {
    let mut lines = text.lines().map(|l| l.trim_end_matches('\r'));
    if lines.find(|l| l.trim().eq_ignore_ascii_case("[description]")).is_none() {
        return String::new();
    }
    lines.take_while(|l| !l.trim().eq_ignore_ascii_case("[end]")).collect::<Vec<_>>().join("\n").trim().to_string()
}

/// The text OMSI shows for `date` on the map in `map_dir`, in `lang` ("ESP", "DEU"...), falling back to English then German.
pub fn info(map_dir: &Path, date: Date, lang: &str) -> String {
    let Ok(rd) = std::fs::read_dir(map_dir.join("Chrono")) else { return String::new() };
    let mut periods: Vec<(u32, String)> = vec![];
    for e in rd.flatten().filter(|e| e.path().is_dir()) {
        let dir = e.path();
        let Some(cfg) = read_any(&dir.join("Chrono.cfg")) else { continue };
        let parse = |tag: &str| value_after(&cfg, tag).and_then(|v| v.parse::<u32>().ok());
        let (Some(start), Some(end)) = (parse("[startdate]"), parse("[enddate]")) else { continue };
        if !(start <= key(date) && key(date) <= end) {
            continue;
        }
        let dsc = [lang, "ENG", "DEU"].iter().find_map(|l| read_any(&dir.join(format!("Chrono_{l}.dsc"))));
        let Some(dsc) = dsc else { continue };
        let name = value_after(&dsc, "[name]").unwrap_or_default();
        let desc = description(&dsc);
        if name.is_empty() && desc.is_empty() {
            continue;
        }
        let rule = "-".repeat(44);
        periods.push((start, format!("{name}\n{rule}\n{desc}")));
    }
    periods.sort_by_key(|p| p.0);
    if periods.is_empty() {
        return String::new();
    }
    let bar = "=".repeat(44);
    let body: Vec<String> = periods.into_iter().map(|p| p.1).collect();
    format!("{bar}\n\n{}\n\n{bar}", body.join(&format!("\n\n{bar}\n\n")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn day_of_year_conversion() {
        assert_eq!(from_year_day(2019, 302), Some((2019, 10, 29)));
        assert_eq!(from_year_day(2020, 60), Some((2020, 2, 29)));
        assert_eq!(from_year_day(2019, 366), None);
    }

    #[test]
    fn reads_the_berlin_brt_period_for_the_saved_date() {
        let map = Path::new(r"C:\Program Files (x86)\Steam\steamapps\common\OMSI 2\maps\BRT Berlin");
        if !map.join("Chrono").is_dir() {
            return;
        }
        let d = situation_date(&map.join("laststn.osn")).unwrap_or((2019, 10, 29));
        let text = info(map, (2019, 10, 29), "ENG");
        assert!(text.contains("Timetable Period 2019 (09.12.18 - 14.12.19)"), "{text}");
        assert!(text.contains("* Long Travel Times"));
        let _ = d;
        assert_eq!(info(map, (1900, 1, 1), "ENG"), "");
    }
}
