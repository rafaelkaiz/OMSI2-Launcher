//! The optional OMSI plugin (see `plugin\` in the project): installing and removing it, and asking it to press "¡Iniciar!".
//! Installing adds exactly two NEW files to OMSI's `plugins\` folder (the DLL and its `.opl` manifest) and removing deletes exactly
//! those two; nothing else in the OMSI folder is touched.

use crate::i18n::{tr, trf};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

const DLL_NAME: &str = "OMSI2LauncherPlugin.dll";
const OPL_NAME: &str = "OMSI2Launcher.opl";
const OPL_TEXT: &str = "[dll]\r\nOMSI2LauncherPlugin.dll\r\n";
static DLL: &[u8] = include_bytes!("../plugin/dist/OMSI2LauncherPlugin.dll");

fn plugins_dir(omsi: &Path) -> PathBuf {
    omsi.join("plugins")
}

pub fn installed(omsi: &Path) -> bool {
    let d = plugins_dir(omsi);
    d.join(DLL_NAME).is_file() && d.join(OPL_NAME).is_file()
}

/// True when the installed DLL is the one built into this launcher.
pub fn up_to_date(omsi: &Path) -> bool {
    std::fs::read(plugins_dir(omsi).join(DLL_NAME)).is_ok_and(|b| b == DLL)
}

pub fn install(omsi: &Path) -> Result<(), String> {
    let d = plugins_dir(omsi);
    std::fs::create_dir_all(&d).map_err(|e| trf("No se pudo instalar el plugin: {}", &[&e.to_string()]))?;
    std::fs::write(d.join(DLL_NAME), DLL).map_err(|e| trf("No se pudo instalar el plugin: {}", &[&e.to_string()]))?;
    std::fs::write(d.join(OPL_NAME), OPL_TEXT).map_err(|e| trf("No se pudo instalar el plugin: {}", &[&e.to_string()]))?;
    Ok(())
}

pub fn uninstall(omsi: &Path) -> Result<(), String> {
    let d = plugins_dir(omsi);
    for f in [OPL_NAME, DLL_NAME] {
        let p = d.join(f);
        if p.exists() {
            std::fs::remove_file(&p).map_err(|e| trf("No se pudo quitar el plugin: {}", &[&e.to_string()]))?;
        }
    }
    Ok(())
}

/// Where launcher and plugin exchange messages (never inside the OMSI folder).
fn exchange_dir() -> PathBuf {
    let base = std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
    base.join("OMSI2-Launcher").join("plugin")
}

/// Asks the plugin to press a control of the start screen (`start`, `radio0`..`radio2`, `datetime`, `tutorials`) and waits for its
/// answer. `Ok(true)`: it did (with `wait_done` it has also finished; without it the action is only known to be under way, which is
/// what is needed for actions that open a dialog or load the map and do not return before that is over). `Ok(false)`: no answer in
/// time (plugin not loaded), the request is withdrawn so the caller can click instead. `Err`: the plugin refused (for example another
/// OMSI version).
pub fn request(cmd: &str, wait_done: bool, timeout: Duration) -> Result<bool, String> {
    let d = exchange_dir();
    std::fs::create_dir_all(&d).map_err(|e| e.to_string())?;
    let (status, request) = (d.join("status"), d.join("request"));
    let _ = std::fs::remove_file(&status);
    std::fs::write(&request, cmd).map_err(|e| e.to_string())?;
    let (started, done) = (format!("started:{cmd}"), format!("done:{cmd}"));
    let until = Instant::now() + timeout;
    while Instant::now() < until {
        std::thread::sleep(Duration::from_millis(100));
        let s = std::fs::read_to_string(&status).unwrap_or_default();
        let s = s.trim();
        if s == done || (!wait_done && s == started) {
            return Ok(true);
        }
        if s.starts_with("error") {
            return Err(s.to_string());
        }
    }
    // not answered: withdraw the request unless the plugin has already begun (then it must not be repeated)
    let _ = std::fs::remove_file(&request);
    let s = std::fs::read_to_string(&status).unwrap_or_default();
    Ok(s.trim() == started || s.trim() == done)
}

pub fn status_text(omsi: Option<&Path>) -> String {
    match omsi {
        Some(o) if installed(o) && up_to_date(o) => tr("Plugin instalado.").to_string(),
        Some(o) if installed(o) => tr("Plugin instalado (versión antigua: vuelve a instalarlo).").to_string(),
        _ => tr("Plugin no instalado.").to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn install_and_uninstall_touch_only_two_new_files() {
        let d = crate::testutil::unique_dir("plugin");
        std::fs::create_dir_all(d.join("plugins")).unwrap();
        std::fs::write(d.join("plugins").join("Other.opl"), "x").unwrap();
        assert!(!installed(&d));
        install(&d).unwrap();
        assert!(installed(&d) && up_to_date(&d));
        let mut names: Vec<String> = std::fs::read_dir(d.join("plugins")).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
        names.sort();
        assert_eq!(names, ["OMSI2Launcher.opl", "OMSI2LauncherPlugin.dll", "Other.opl"]);
        uninstall(&d).unwrap();
        assert!(!installed(&d));
        assert!(d.join("plugins").join("Other.opl").is_file());
    }

    #[test]
    fn the_embedded_dll_is_a_32_bit_pe() {
        assert_eq!(&DLL[..2], b"MZ");
        let pe = u32::from_le_bytes([DLL[0x3c], DLL[0x3d], DLL[0x3e], DLL[0x3f]]) as usize;
        assert_eq!(&DLL[pe..pe + 4], b"PE\0\0");
        assert_eq!(u16::from_le_bytes([DLL[pe + 4], DLL[pe + 5]]), 0x14C);
    }
}
