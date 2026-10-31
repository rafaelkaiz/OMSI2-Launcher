//! The "4 GB patch" for `Omsi.exe`: turns on (or off) the "large address aware" flag in the program's PE header, which lets a 32-bit
//! program use up to 4 GB of memory on 64-bit Windows instead of 2 GB. Only that one bit and the header checksum change.
//! This is the only place where the launcher writes to a file OMSI installs, so it works under strict rules: the user presses the
//! button, a copy goes to `%APPDATA%\OMSI2-Launcher\backups` first, nothing is written while OMSI runs, and the new file is written
//! next to the old one and swapped in only when it is complete.

use crate::cfg;
use crate::i18n::trf;
use std::path::Path;

/// `IMAGE_FILE_LARGE_ADDRESS_AWARE` in the file header's Characteristics.
const LARGE_ADDRESS_AWARE: u16 = 0x20;
const MACHINE_I386: u16 = 0x14C;

fn u16_at(b: &[u8], o: usize) -> Option<u16> {
    b.get(o..o + 2).map(|s| u16::from_le_bytes([s[0], s[1]]))
}

fn u32_at(b: &[u8], o: usize) -> Option<u32> {
    b.get(o..o + 4).map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

/// Offset of the "PE\0\0" signature of a 32-bit program, or why the file is not one.
fn pe_offset(b: &[u8]) -> Result<usize, String> {
    let bad = || "Omsi.exe no tiene el formato esperado".to_string();
    if b.get(..2) != Some(b"MZ") {
        return Err(bad());
    }
    let pe = u32_at(b, 0x3C).ok_or_else(bad)? as usize;
    if b.get(pe..pe + 4) != Some(b"PE\0\0") || u16_at(b, pe + 4) != Some(MACHINE_I386) || b.len() < pe + 24 + 68 {
        return Err(bad());
    }
    Ok(pe)
}

/// The checksum Windows stores in the header (what `CheckSumMappedFile` computes): 16-bit words added with carry, the checksum field
/// itself skipped, plus the file length.
fn pe_checksum(b: &[u8], field: usize) -> u32 {
    let mut sum: u32 = 0;
    let mut i = 0;
    while i < b.len() {
        if i != field && i != field + 2 {
            let w = if i + 1 < b.len() { u16::from_le_bytes([b[i], b[i + 1]]) } else { b[i] as u16 };
            sum += w as u32;
            sum = (sum & 0xFFFF) + (sum >> 16);
        }
        i += 2;
    }
    sum = (sum & 0xFFFF) + (sum >> 16);
    sum.wrapping_add(b.len() as u32)
}

/// Whether the program in `bytes` has the flag, or an error when it is not a 32-bit program.
pub fn is_large_address_aware(bytes: &[u8]) -> Result<bool, String> {
    let pe = pe_offset(bytes)?;
    Ok(u16_at(bytes, pe + 22).unwrap_or(0) & LARGE_ADDRESS_AWARE != 0)
}

/// The same bytes with the flag set or cleared (and the checksum brought up to date when the header had one).
pub fn with_flag(bytes: &[u8], on: bool) -> Result<Vec<u8>, String> {
    let pe = pe_offset(bytes)?;
    let mut out = bytes.to_vec();
    let flags = u16_at(&out, pe + 22).unwrap_or(0);
    let flags = if on { flags | LARGE_ADDRESS_AWARE } else { flags & !LARGE_ADDRESS_AWARE };
    out[pe + 22..pe + 24].copy_from_slice(&flags.to_le_bytes());
    let field = pe + 24 + 64;
    if u32_at(&out, field) != Some(0) {
        let sum = pe_checksum(&out, field);
        out[field..field + 4].copy_from_slice(&sum.to_le_bytes());
    }
    Ok(out)
}

/// State of `<omsi>\Omsi.exe`: `Some(flag)`, or `None` when it cannot be read.
pub fn status(omsi: &Path) -> Option<bool> {
    // only the header is needed
    let mut head = vec![0u8; 4096];
    let n = std::io::Read::read(&mut std::fs::File::open(omsi.join("Omsi.exe")).ok()?, &mut head).ok()?;
    head.truncate(n);
    is_large_address_aware(&head).ok()
}

/// Sets or clears the flag in `<omsi>\Omsi.exe`. The caller makes sure OMSI is not running. Returns the path of the backup copy.
pub fn apply(omsi: &Path, on: bool) -> Result<std::path::PathBuf, String> {
    let exe = omsi.join("Omsi.exe");
    let fail = |e: String| trf("No se pudo modificar Omsi.exe: {}", &[&e]);
    let bytes = std::fs::read(&exe).map_err(|e| fail(e.to_string()))?;
    let patched = with_flag(&bytes, on).map_err(fail)?;
    if patched == bytes {
        return Err(fail("ya estaba así".to_string()));
    }
    let copy = cfg::backup(&exe)?;
    let tmp = omsi.join("Omsi.exe.new");
    std::fs::write(&tmp, &patched).map_err(|e| fail(e.to_string()))?;
    if let Err(e) = std::fs::rename(&tmp, &exe) {
        let _ = std::fs::remove_file(&tmp);
        return Err(fail(e.to_string()));
    }
    Ok(copy)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A tiny 32-bit program header: MZ, PE at 0x80, optional-header checksum field filled in.
    fn fake_exe(flags: u16, with_checksum: bool) -> Vec<u8> {
        let mut b = vec![0u8; 0x400 + 3];
        b[..2].copy_from_slice(b"MZ");
        b[0x3C..0x40].copy_from_slice(&0x80u32.to_le_bytes());
        b[0x80..0x84].copy_from_slice(b"PE\0\0");
        b[0x84..0x86].copy_from_slice(&MACHINE_I386.to_le_bytes());
        b[0x80 + 22..0x80 + 24].copy_from_slice(&flags.to_le_bytes());
        for (i, v) in b.iter_mut().enumerate().skip(0x200) {
            *v = (i % 251) as u8;
        }
        if with_checksum {
            let f = 0x80 + 24 + 64;
            let s = pe_checksum(&b, f);
            b[f..f + 4].copy_from_slice(&s.to_le_bytes());
        }
        b
    }

    #[test]
    fn flag_is_read_set_and_cleared() {
        let off = fake_exe(0x0102, false);
        assert_eq!(is_large_address_aware(&off), Ok(false));
        let on = with_flag(&off, true).unwrap();
        assert_eq!(is_large_address_aware(&on), Ok(true));
        assert_eq!(u16_at(&on, 0x80 + 22), Some(0x0122));
        let back = with_flag(&on, false).unwrap();
        assert_eq!(back, off);
    }

    #[test]
    fn only_the_flag_and_the_checksum_change() {
        let off = fake_exe(0x0102, true);
        let on = with_flag(&off, true).unwrap();
        let changed: Vec<usize> = (0..off.len()).filter(|&i| off[i] != on[i]).collect();
        let field = 0x80 + 24 + 64;
        assert!(changed.iter().all(|&i| i == 0x80 + 22 || (field..field + 4).contains(&i)), "{changed:?}");
        // the stored checksum is the right one for the new bytes, and going back restores the old file exactly
        assert_eq!(u32_at(&on, field), Some(pe_checksum(&on, field)));
        assert_eq!(with_flag(&on, false).unwrap(), off);
    }

    #[test]
    fn a_zero_checksum_stays_zero() {
        let on = with_flag(&fake_exe(0, false), true).unwrap();
        assert_eq!(u32_at(&on, 0x80 + 24 + 64), Some(0));
    }

    #[test]
    fn files_that_are_not_32_bit_programs_are_refused() {
        assert!(is_large_address_aware(b"hello").is_err());
        let mut x64 = fake_exe(0, false);
        x64[0x84..0x86].copy_from_slice(&0x8664u16.to_le_bytes());
        assert!(with_flag(&x64, true).is_err());
    }

    #[test]
    fn apply_backs_up_swaps_the_file_and_leaves_no_temporary() {
        let d = crate::testutil::unique_dir("exepatch");
        let exe = d.join("Omsi.exe");
        let original = fake_exe(0x0102, true);
        std::fs::write(&exe, &original).unwrap();
        assert_eq!(status(&d), Some(false));
        let copy = apply(&d, true).unwrap();
        assert_eq!(std::fs::read(&copy).unwrap(), original);
        assert_eq!(status(&d), Some(true));
        assert!(!d.join("Omsi.exe.new").exists());
        assert!(apply(&d, true).is_err(), "nothing to change the second time");
        apply(&d, false).unwrap();
        assert_eq!(std::fs::read(&exe).unwrap(), original);
    }
}
