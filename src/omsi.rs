//! Everything that touches OMSI 2: finding it, reading its maps/drivers and driving its start window.
//! Nothing here writes to the OMSI folder; the map is chosen by sending Win32 messages to OMSI's own start screen.

use crate::i18n::{tr, trf};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use windows_sys::Win32::Foundation::{HWND, LPARAM, RECT};
use windows_sys::Win32::UI::WindowsAndMessaging::*;

#[derive(Clone)]
pub struct MapInfo {
    pub folder: String,
    pub name: String,
    /// `[friendlyname]`, the other name OMSI may show for the map.
    pub friendly: String,
    pub description: String,
    pub picture: Option<PathBuf>,
    /// The map has a `laststn.osn` (a saved last state): only those maps can be loaded with "last state".
    pub has_last_state: bool,
}

/// What the start screen does with the map: the first two radio buttons of OMSI's own screen.
#[derive(Clone, Copy, PartialEq)]
pub enum LoadMode {
    LastState,
    NoBuses,
    SavedSituation,
    /// Opens OMSI's tutorial menu and starts tutorial 1 to 4.
    Tutorial(u8),
}

pub fn find_omsi_dir(saved: Option<PathBuf>) -> Option<PathBuf> {
    let from_arg = std::env::args().nth(1).map(PathBuf::from);
    let from_env = std::env::var_os("OMSI_DIR").map(PathBuf::from);
    let defaults = [
        r"C:\Program Files (x86)\Steam\steamapps\common\OMSI 2",
        r"C:\Program Files\Steam\steamapps\common\OMSI 2",
        r"C:\Program Files (x86)\Aerosoft\OMSI 2",
    ]
    .map(PathBuf::from);
    from_arg
        .into_iter()
        .chain(from_env)
        .chain(saved)
        .chain(defaults)
        .find(|p| p.join("Omsi.exe").is_file())
}

/// OMSI writes its .cfg files as UTF-16 (with BOM) or plain ANSI depending on the author.
fn read_cfg(path: &Path) -> Option<String> {
    let b = std::fs::read(path).ok()?;
    if b.starts_with(&[0xFF, 0xFE]) {
        let u: Vec<u16> = b[2..].chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
        Some(String::from_utf16_lossy(&u))
    } else {
        Some(b.iter().map(|&c| c as char).collect())
    }
}

fn cfg_value(cfg: &str, key: &str) -> Option<String> {
    let mut lines = cfg.lines();
    while let Some(l) = lines.next() {
        if l.trim().eq_ignore_ascii_case(key) {
            return lines.next().map(|v| v.trim().to_string()).filter(|v| !v.is_empty());
        }
    }
    None
}

/// Lines under `key` up to the next `[tag]` line (used for the map description).
fn cfg_block(cfg: &str, key: &str) -> String {
    let is_tag = |l: &str| l.starts_with('[') && l.ends_with(']') && !l.contains(' ');
    let mut lines = cfg.lines().map(str::trim_end);
    lines.by_ref().find(|l| l.trim().eq_ignore_ascii_case(key));
    let body: Vec<&str> = lines.take_while(|l| !is_tag(l.trim())).collect();
    body.join("\n").trim().to_string()
}

pub fn list_maps(omsi: &Path) -> Vec<MapInfo> {
    let mut maps: Vec<MapInfo> = std::fs::read_dir(omsi.join("maps"))
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let dir = e.path();
            let cfg = read_cfg(&dir.join("global.cfg"))?;
            let name = cfg_value(&cfg, "[name]")?;
            let friendly = cfg_value(&cfg, "[friendlyname]").unwrap_or_else(|| name.clone());
            let description = cfg_block(&cfg, "[description]");
            let picture = Some(dir.join("picture.jpg")).filter(|p| p.is_file());
            let folder = dir.file_name()?.to_string_lossy().into_owned();
            let has_last_state = dir.join("laststn.osn").is_file();
            Some(MapInfo { folder, name, friendly, description, picture, has_last_state })
        })
        .collect();
    maps.sort_by_key(|m| m.name.to_lowercase());
    maps
}

fn class_name(h: HWND) -> String {
    let mut b = [0u16; 128];
    let n = unsafe { GetClassNameW(h, b.as_mut_ptr(), 128) };
    String::from_utf16_lossy(&b[..n.max(0) as usize])
}

unsafe extern "system" fn collect(h: HWND, l: LPARAM) -> i32 {
    let v = unsafe { &mut *(l as *mut Vec<HWND>) };
    v.push(h);
    1
}

fn top_windows() -> Vec<HWND> {
    let mut v: Vec<HWND> = Vec::new();
    unsafe { EnumWindows(Some(collect), &mut v as *mut _ as LPARAM) };
    v
}

fn children(parent: HWND) -> Vec<HWND> {
    let mut v: Vec<HWND> = Vec::new();
    unsafe { EnumChildWindows(parent, Some(collect), &mut v as *mut _ as LPARAM) };
    v
}

/// True while an `Omsi.exe` process exists. Asks Windows for the process list directly: starting a program such as `tasklist` for
/// this would flash a console window every time.
pub fn omsi_running() -> bool {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS};
    unsafe {
        let snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snap.is_null() || snap as isize == -1 {
            return false;
        }
        let mut entry: PROCESSENTRY32W = std::mem::zeroed();
        entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
        let mut found = false;
        let mut ok = Process32FirstW(snap, &mut entry) != 0;
        while ok {
            let len = entry.szExeFile.iter().position(|&c| c == 0).unwrap_or(entry.szExeFile.len());
            if String::from_utf16_lossy(&entry.szExeFile[..len]).eq_ignore_ascii_case("Omsi.exe") {
                found = true;
                break;
            }
            ok = Process32NextW(snap, &mut entry) != 0;
        }
        CloseHandle(snap);
        found
    }
}

/// OMSI's start screen ("<Start...>") once it is visible.
fn find_start_form() -> Option<HWND> {
    top_windows()
        .into_iter()
        .find(|&h| class_name(h) == "Tform_start" && unsafe { IsWindowVisible(h) } != 0)
}

fn combo_items(h: HWND) -> Vec<String> {
    let n = unsafe { SendMessageW(h, CB_GETCOUNT, 0, 0) };
    (0..n.max(0))
        .map(|i| {
            let len = unsafe { SendMessageW(h, CB_GETLBTEXTLEN, i as usize, 0) }.max(0) as usize;
            let mut b = vec![0u16; len + 1];
            unsafe { SendMessageW(h, CB_GETLBTEXT, i as usize, b.as_mut_ptr() as LPARAM) };
            String::from_utf16_lossy(&b[..len])
        })
        .collect()
}

/// Picks `name` in a combo box of OMSI's start form and notifies the form like a real click would.
fn select_in_combo(form: HWND, combo: HWND, index: usize) {
    unsafe {
        SendMessageW(combo, CB_SETCURSEL, index, 0);
        let id = (GetDlgCtrlID(combo) as usize) & 0xFFFF;
        SendMessageW(form, WM_COMMAND, id | ((CBN_SELCHANGE as usize) << 16), combo as LPARAM);
    }
}

/// OMSI leaves a file called "closecheck" in its folder when it is closed badly; it is removed so the next start is normal.
/// Never touched while OMSI is running.
pub fn remove_closecheck(omsi: &Path) {
    if omsi_running() {
        return;
    }
    let Ok(entries) = std::fs::read_dir(omsi) else { return };
    for e in entries.flatten() {
        if e.file_name().to_string_lossy().eq_ignore_ascii_case("closecheck") && e.path().is_file() {
            match std::fs::remove_file(e.path()) {
                Ok(()) => crate::log::line("closecheck eliminado"),
                Err(err) => crate::log::line(&format!("no se pudo eliminar closecheck: {err}")),
            }
        }
    }
}

/// Starts OMSI if it is not running, then chooses map and driver on its start screen and presses "Start".
pub fn launch(
    omsi: &Path,
    map: &MapInfo,
    driver: Option<&str>,
    windowed: bool,
    mode: LoadMode,
    time: &TimeChoice,
    situation: Option<(&str, usize)>,
    spawn: Option<&str>,
    status: impl Fn(&str),
    on_start_screen: impl Fn(),
) -> Result<(), String> {
    if !omsi_running() {
        status(tr("Abriendo OMSI 2..."));
        let mut cmd = Command::new(omsi.join("Omsi.exe"));
        cmd.current_dir(omsi);
        if windowed {
            cmd.arg("-windowed");
        }
        cmd.spawn().map_err(|e| trf("No se pudo abrir Omsi.exe: {}", &[&e.to_string()]))?;
    } else {
        status(tr("OMSI 2 ya está abierto, esperando su pantalla de inicio..."));
    }

    let deadline = Instant::now() + Duration::from_secs(600);
    let form = loop {
        if let Some(f) = find_start_form() {
            break f;
        }
        if Instant::now() > deadline {
            return Err(tr("OMSI no llegó a su pantalla de inicio (10 min).").into());
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    // OMSI has created its 3D device by now: the launcher may go back in front of its start screen.
    on_start_screen();
    status(tr("Pantalla de inicio detectada, eligiendo mapa..."));
    std::thread::sleep(Duration::from_millis(500));

    // The buttons and radio buttons are not windows, they are drawn by the form. With the optional plugin installed they are pressed
    // from inside OMSI; otherwise (or as soon as the plugin does not answer) they are clicked where they are drawn, at positions
    // measured on OMSI's 1000x768 start screen and scaled to the real client size.
    let mut plugin = crate::plugin::installed(omsi);
    // The map list depends on the load mode: with "last state" OMSI hides the maps that have no last state. When the user wants
    // "load map without buses" it is chosen FIRST (it lists every map), then the map. The other modes keep the usual order.
    if mode == LoadMode::NoBuses {
        press_control(form, &mut plugin, "radio1", true, 587, 112, &status);
        std::thread::sleep(Duration::from_millis(500));
    }

    // The two combo boxes on the start form: the one that has the map name is the map one, the other is the driver one.
    let combos: Vec<HWND> = children(form).into_iter().filter(|&c| class_name(c) == "TComboBox").collect();
    let map_combo = combos
        .iter()
        .copied()
        .find_map(|c| {
            combo_items(c)
                .iter()
                .position(|i| i.eq_ignore_ascii_case(&map.name) || i.eq_ignore_ascii_case(&map.friendly))
                .map(|i| (c, i))
        })
        .ok_or_else(|| trf("OMSI no lista el mapa \"{}\".", &[&map.name]))?;
    select_in_combo(form, map_combo.0, map_combo.1);

    if let Some(d) = driver {
        if let Some((c, i)) = combos
            .iter()
            .copied()
            .filter(|&c| c != map_combo.0)
            .find_map(|c| combo_items(c).iter().position(|x| x == d).map(|i| (c, i)))
        {
            select_in_combo(form, c, i);
        }
    }
    std::thread::sleep(Duration::from_millis(800));
    // The date and time go LAST: choosing a load mode (or a situation in the list) makes OMSI recompute the time shown from that
    // situation, which would throw away a time chosen before.
    match mode {
        LoadMode::LastState => press_control(form, &mut plugin, "radio0", true, 587, 74, &status),
        LoadMode::NoBuses => {} // already chosen before the map
        LoadMode::SavedSituation => press_control(form, &mut plugin, "radio2", true, 587, 149, &status),
        LoadMode::Tutorial(_) => {}
    }
    std::thread::sleep(Duration::from_millis(300));
    if let (LoadMode::SavedSituation, Some((name, occurrence))) = (mode, situation) {
        pick_situation(form, name, occurrence, &status)?;
    }
    if *time != TimeChoice::Situation && !matches!(mode, LoadMode::Tutorial(_)) {
        set_time(form, time, &mut plugin, &status)?;
        std::thread::sleep(Duration::from_millis(300));
    }
    status(tr("Pulsando ¡Iniciar!..."));
    // "Tutoriales" is the second button of the bottom row, "¡Iniciar!" the first.
    if matches!(mode, LoadMode::Tutorial(_)) {
        press_control(form, &mut plugin, "tutorials", false, 370, 750, &status);
    } else {
        press_control(form, &mut plugin, "start", false, 120, 750, &status);
    }
    if let LoadMode::Tutorial(n) = mode {
        pick_tutorial(n, plugin, &status)?;
        return Ok(());
    }
    if let (LoadMode::NoBuses, Some(place)) = (mode, spawn) {
        pick_spawn(place, &status)?;
    }
    status(tr("Cargando el mapa en OMSI 2..."));
    Ok(())
}

/// After "Cargar situación guardada" OMSI shows a list of situations: select `name` (its `occurrence`-th entry if the name repeats).
fn pick_situation(form: HWND, name: &str, occurrence: usize, status: &impl Fn(&str)) -> Result<(), String> {
    status(tr("Eligiendo la situación guardada..."));
    let deadline = Instant::now() + Duration::from_secs(8);
    loop {
        let list = children(form).into_iter().find(|&c| class_name(c) == "TListBox" && unsafe { IsWindowVisible(c) } != 0);
        if let Some(list) = list {
            let n = unsafe { SendMessageW(list, LB_GETCOUNT, 0, 0) }.max(0) as usize;
            let items: Vec<String> = (0..n)
                .map(|i| {
                    let len = unsafe { SendMessageW(list, LB_GETTEXTLEN, i, 0) }.max(0) as usize;
                    let mut b = vec![0u16; len + 1];
                    unsafe { SendMessageW(list, LB_GETTEXT, i, b.as_mut_ptr() as LPARAM) };
                    String::from_utf16_lossy(&b[..len])
                })
                .collect();
            if n > 0 {
                let at = items.iter().enumerate().filter(|(_, t)| t.trim().eq_ignore_ascii_case(name.trim())).map(|(i, _)| i).nth(occurrence);
                let at = at.or_else(|| items.iter().position(|t| t.trim().eq_ignore_ascii_case(name.trim())));
                let Some(i) = at else { return Err(trf("OMSI no lista la situación \"{}\".", &[name])) };
                unsafe {
                    SendMessageW(list, LB_SETCURSEL, i, 0);
                    let id = (GetDlgCtrlID(list) as usize) & 0xFFFF;
                    SendMessageW(form, WM_COMMAND, id | ((LBN_SELCHANGE as usize) << 16), list as LPARAM);
                }
                std::thread::sleep(Duration::from_millis(500));
                return Ok(());
            }
        }
        if Instant::now() > deadline {
            return Err(tr("No apareció la lista de situaciones de OMSI.").into());
        }
        std::thread::sleep(Duration::from_millis(200));
    }
}

/// In OMSI's tutorial menu (792x484) the four tutorials are drawn buttons stacked on the left; click the n-th one (1 to 4).
fn pick_tutorial(n: u8, plugin: bool, status: &impl Fn(&str)) -> Result<(), String> {
    status(tr("Abriendo el menú de tutoriales..."));
    let deadline = Instant::now() + Duration::from_secs(10);
    let menu = loop {
        if let Some(m) = top_windows().into_iter().find(|&h| class_name(h) == "Tform_tutmenu" && unsafe { IsWindowVisible(h) } != 0) {
            break m;
        }
        if Instant::now() > deadline {
            return Err(tr("No se abrió el menú de tutoriales de OMSI.").into());
        }
        std::thread::sleep(Duration::from_millis(200));
    };
    std::thread::sleep(Duration::from_millis(600));
    // With the plugin the tutorial is chosen from inside OMSI (the menu closes with that result); otherwise its button is clicked.
    let chosen = plugin && matches!(crate::plugin::request(&format!("tutorial{}", n.clamp(1, 4)), true, Duration::from_secs(4)), Ok(true));
    if !chosen {
        let y = [74, 182, 290, 398][(n.clamp(1, 4) - 1) as usize];
        click_in(menu, 210, y, 792, 484);
    }
    status(tr("Cargando el tutorial en OMSI 2..."));
    Ok(())
}

/// Presses a control of the start form: through the plugin (`cmd`) if it is installed and answers, otherwise by clicking at (x, y) of
/// the 1000x768 layout. `wait_done`: wait until the plugin has finished (not for actions that open a dialog or load the map).
/// Once the plugin fails to answer it is not tried again in this run, so the mouse takes over for the rest.
fn press_control(form: HWND, plugin: &mut bool, cmd: &str, wait_done: bool, x: i32, y: i32, status: &impl Fn(&str)) {
    if *plugin {
        match crate::plugin::request(cmd, wait_done, Duration::from_secs(if wait_done { 5 } else { 6 })) {
            Ok(true) => return,
            Ok(false) => status(tr("El plugin no respondió: se pulsa con el ratón.")),
            Err(e) => status(&e),
        }
        *plugin = false;
    }
    click(form, x, y);
}

/// Left click on the start form at (x, y) of its 1000x768 layout.
fn click(form: HWND, x: i32, y: i32) {
    click_in(form, x, y, 1000, 768);
}

/// Left click at (x, y) of a form laid out at `w` x `h` (scaled to its real client size).
fn click_in(form: HWND, x: i32, y: i32, w: i32, h: i32) {
    let mut r = RECT { left: 0, top: 0, right: 0, bottom: 0 };
    unsafe { GetClientRect(form, &mut r) };
    let (x, y) = (x * r.right / w, y * r.bottom / h);
    let pos = (((y as usize) & 0xFFFF) << 16) | (x as usize & 0xFFFF);
    unsafe {
        PostMessageW(form, WM_LBUTTONDOWN, 1, pos as LPARAM);
        std::thread::sleep(Duration::from_millis(80));
        PostMessageW(form, WM_LBUTTONUP, 0, pos as LPARAM);
    }
}

/// "2.3.004"-style version read from Omsi.exe (it carries the string "OMSI 2.x.yyy").
pub fn version(omsi: &Path) -> Option<String> {
    let b = std::fs::read(omsi.join("Omsi.exe")).ok()?;
    // The version string is stored as UTF-16.
    let pat: Vec<u8> = "OMSI 2.".encode_utf16().flat_map(u16::to_le_bytes).collect();
    let at = b.windows(pat.len()).position(|w| w == pat.as_slice())?;
    let s: String = b[at + 10..].chunks_exact(2).take(8).map(|c| c[0] as char).take_while(|c| c.is_ascii_digit() || *c == '.').collect();
    (!s.is_empty()).then_some(s)
}

/// Which date/time OMSI should start with. `Situation` leaves OMSI's own choice alone.
#[derive(Clone, Copy, PartialEq)]
pub enum TimeChoice {
    Situation,
    /// Day, month, year, hour, minute, second.
    Custom(u32, u32, u32, u32, u32, u32),
    /// Only the time of day (hour, minute, second); OMSI keeps its date.
    TimeOnly(u32, u32, u32),
}

fn rect_of(h: HWND) -> RECT {
    let mut r = RECT { left: 0, top: 0, right: 0, bottom: 0 };
    unsafe { GetWindowRect(h, &mut r) };
    r
}

fn set_text(h: HWND, s: &str) {
    let w: Vec<u16> = s.encode_utf16().chain(Some(0)).collect();
    unsafe { SendMessageW(h, WM_SETTEXT, 0, w.as_ptr() as LPARAM) };
}

fn press(h: HWND) {
    unsafe { PostMessageW(h, BM_CLICK, 0, 0) };
}

/// Opens OMSI's "Seleccionar hora y fecha..." dialog from the start screen, fills it in and accepts it.
/// The dialog's controls are real windows; they are told apart by position, so this works in any OMSI language.
fn set_time(form: HWND, time: &TimeChoice, plugin: &mut bool, status: &impl Fn(&str)) -> Result<(), String> {
    status(tr("Eligiendo fecha y hora..."));
    press_control(form, plugin, "datetime", false, 83, 382, status); // the "Cambiar fecha y hora" radio button opens the dialog
    let deadline = Instant::now() + Duration::from_secs(10);
    let dlg = loop {
        if let Some(d) = top_windows().into_iter().find(|&h| class_name(h) == "Tform_daytime" && unsafe { IsWindowVisible(h) } != 0) {
            break d;
        }
        if Instant::now() > deadline {
            return Err(tr("No se abrió el diálogo de fecha y hora de OMSI.").into());
        }
        std::thread::sleep(Duration::from_millis(200));
    };
    std::thread::sleep(Duration::from_millis(500));

    let mut edits: Vec<HWND> = children(dlg).into_iter().filter(|&c| class_name(c) == "TEdit").collect();
    let mut buttons: Vec<HWND> = children(dlg).into_iter().filter(|&c| class_name(c) == "TButton").collect();
    let key = |h: &HWND| {
        let r = rect_of(*h);
        (r.top / 8, r.left)
    };
    edits.sort_by_key(key);
    buttons.sort_by_key(key);
    // edits: hour, minute, second / day, month, year.  buttons: current time, Day, Dusk, Night, Dawn, current date, current year, Accept, Cancel.
    if edits.len() != 6 || buttons.len() != 9 {
        press(buttons.last().copied().unwrap_or(dlg));
        return Err(tr("El diálogo de fecha y hora de OMSI no tiene el aspecto esperado.").into());
    }
    match *time {
        TimeChoice::Custom(d, m, y, h, mi, s) => {
            for (e, v) in [(edits[5], y), (edits[4], m), (edits[3], d), (edits[0], h), (edits[1], mi), (edits[2], s)] {
                set_text(e, &v.to_string());
            }
        }
        TimeChoice::TimeOnly(h, mi, s) => {
            for (e, v) in [(edits[0], h), (edits[1], mi), (edits[2], s)] {
                set_text(e, &v.to_string());
            }
        }
        TimeChoice::Situation => {}
    }
    std::thread::sleep(Duration::from_millis(400));
    press(buttons[7]); // Accept
    let deadline = Instant::now() + Duration::from_secs(5);
    while top_windows().into_iter().any(|h| class_name(h) == "Tform_daytime" && unsafe { IsWindowVisible(h) } != 0) {
        if Instant::now() > deadline {
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    std::thread::sleep(Duration::from_millis(300));
    Ok(())
}

#[cfg(test)]
mod tests {
    /// The direct process-list query must agree with what Windows' own `tasklist` says.
    #[test]
    fn omsi_running_agrees_with_tasklist() {
        let out = std::process::Command::new("tasklist").args(["/FI", "IMAGENAME eq Omsi.exe", "/NH"]).output().unwrap();
        let expected = String::from_utf8_lossy(&out.stdout).contains("Omsi.exe");
        assert_eq!(super::omsi_running(), expected);
    }
}

/// Minimises this program's own window through Windows. egui's own minimise command leaves a full-screen window that does not
/// repaint after it is restored; minimising it the way the taskbar does does not.
pub fn minimize_own_window() {
    if let Some(h) = own_window() {
        unsafe { ShowWindow(h, SW_MINIMIZE) };
    }
}

/// This program's main window (the visible top-level window of this process that has no owner).
fn own_window() -> Option<HWND> {
    use windows_sys::Win32::System::Threading::GetCurrentProcessId;
    unsafe extern "system" fn find(h: HWND, l: LPARAM) -> i32 {
        let mut pid = 0u32;
        unsafe { GetWindowThreadProcessId(h, &mut pid) };
        if pid == unsafe { GetCurrentProcessId() } && unsafe { IsWindowVisible(h) } != 0 && unsafe { GetWindow(h, GW_OWNER) }.is_null() {
            unsafe { *(l as *mut Option<HWND>) = Some(h) };
            return 0;
        }
        1
    }
    let mut found: Option<HWND> = None;
    unsafe { EnumWindows(Some(find), &mut found as *mut _ as LPARAM) };
    found
}

/// Windows' own "choose a folder" dialog, owned by the launcher window so it is not hidden behind it.
pub fn browse_folder(title: &str) -> Option<PathBuf> {
    use windows_sys::Win32::System::Com::CoTaskMemFree;
    use windows_sys::Win32::UI::Shell::{BIF_NEWDIALOGSTYLE, BIF_RETURNONLYFSDIRS, BROWSEINFOW, SHBrowseForFolderW, SHGetPathFromIDListW};
    let title: Vec<u16> = title.encode_utf16().chain(Some(0)).collect();
    unsafe {
        let info = BROWSEINFOW {
            hwndOwner: own_window().unwrap_or(std::ptr::null_mut()),
            pidlRoot: std::ptr::null_mut(),
            pszDisplayName: std::ptr::null_mut(),
            lpszTitle: title.as_ptr(),
            ulFlags: BIF_RETURNONLYFSDIRS | BIF_NEWDIALOGSTYLE,
            lpfn: None,
            lParam: 0,
            iImage: 0,
        };
        let pidl = SHBrowseForFolderW(&info);
        if pidl.is_null() {
            return None;
        }
        let mut buf = [0u16; 520];
        let ok = SHGetPathFromIDListW(pidl, buf.as_mut_ptr()) != 0;
        CoTaskMemFree(pidl as *const _);
        let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
        ok.then(|| PathBuf::from(String::from_utf16_lossy(&buf[..len])))
    }
}

/// Opens a folder (or a web address) with Windows.
pub fn open_folder(dir: &Path) {
    let _ = Command::new("explorer").arg(dir).spawn();
}

/// The places OMSI offers to appear at when a map is loaded without buses: the names of the map's `[entrypoints]` (global.cfg), once
/// each and in alphabetical order, the way OMSI's "Establecer posición" window lists them. Empty when the map has none.
pub fn entry_points(omsi: &Path, map: &MapInfo) -> Vec<String> {
    let Some(cfg) = read_cfg(&omsi.join("maps").join(&map.folder).join("global.cfg")) else { return vec![] };
    let mut names: Vec<String> = Vec::new();
    let mut inside = false;
    for line in cfg.lines().map(str::trim) {
        // a section tag is a lone "[word]"; a place name may also start with brackets ("[25 DIM] Conleau")
        let is_tag = line.starts_with('[') && line.ends_with(']') && !line.contains(' ') && line.matches('[').count() == 1;
        if is_tag {
            inside = line.eq_ignore_ascii_case("[entrypoints]");
            continue;
        }
        // every entry has one text line (its name); the rest are numbers
        if inside && !line.is_empty() && line.parse::<f64>().is_err() && !names.iter().any(|n| n.eq_ignore_ascii_case(line)) {
            names.push(line.to_string());
        }
    }
    names.sort_by_key(|n| n.to_lowercase());
    names
}

/// Answers OMSI's "Establecer posición" window (shown after ¡Iniciar! when the map is loaded without buses): picks `place` in its
/// list and presses Accept. Waits while OMSI loads; gives up if OMSI closes.
fn pick_spawn(place: &str, status: &impl Fn(&str)) -> Result<(), String> {
    status(tr("Esperando la ventana de posición inicial de OMSI..."));
    let deadline = Instant::now() + Duration::from_secs(1800);
    let window = loop {
        let found = top_windows().into_iter().find(|&w| class_name(w) == "Tform_setpos" && unsafe { IsWindowVisible(w) } != 0);
        if let Some(w) = found {
            break w;
        }
        if !omsi_running() {
            return Err(tr("OMSI se cerró antes de pedir la posición inicial.").into());
        }
        if Instant::now() > deadline {
            return Err(tr("No apareció la ventana de posición inicial de OMSI.").into());
        }
        std::thread::sleep(Duration::from_millis(200));
    };
    status(tr("Eligiendo el punto de aparición..."));
    std::thread::sleep(Duration::from_millis(400));
    let kids = children(window);
    let Some(list) = kids.iter().copied().find(|&c| class_name(c) == "TListBox") else {
        return Err(tr("La ventana de posición inicial de OMSI no tiene la lista esperada.").into());
    };
    let n = unsafe { SendMessageW(list, LB_GETCOUNT, 0, 0) }.max(0) as usize;
    let items: Vec<String> = (0..n)
        .map(|i| {
            let len = unsafe { SendMessageW(list, LB_GETTEXTLEN, i, 0) }.max(0) as usize;
            let mut b = vec![0u16; len + 1];
            unsafe { SendMessageW(list, LB_GETTEXT, i, b.as_mut_ptr() as LPARAM) };
            String::from_utf16_lossy(&b[..len])
        })
        .collect();
    let Some(at) = items.iter().position(|t| t.trim().eq_ignore_ascii_case(place.trim())) else {
        return Err(trf("OMSI no lista el punto \"{}\".", &[place]));
    };
    unsafe {
        SendMessageW(list, LB_SETCURSEL, at, 0);
        let id = (GetDlgCtrlID(list) as usize) & 0xFFFF;
        SendMessageW(window, WM_COMMAND, id | ((LBN_SELCHANGE as usize) << 16), list as LPARAM);
    }
    std::thread::sleep(Duration::from_millis(400));
    // the Accept button is the left one of the two buttons
    let accept = kids.iter().copied().filter(|&c| class_name(c) == "TButton").min_by_key(|&c| rect_of(c).left);
    match accept {
        Some(b) => press(b),
        None => return Err(tr("No se encontró el botón Aceptar de OMSI.").into()),
    }
    Ok(())
}

#[cfg(test)]
mod spawn_tests {
    use super::*;

    #[test]
    fn grundorf_entry_points_match_omsis_list() {
        let Some(omsi) = find_omsi_dir(None) else { return };
        let Some(map) = list_maps(&omsi).into_iter().find(|m| m.name == "Grundorf") else { return };
        assert_eq!(entry_points(&omsi, &map), ["Einsteindorf Krankenhaus", "Nordspitze Bauernhof", "Nordspitze Bhf."]);
    }
}

#[cfg(test)]
mod spawn_name_tests {
    use super::*;

    #[test]
    fn place_names_may_start_with_brackets() {
        let dir = crate::testutil::unique_dir("spawn");
        let map_dir = dir.join("maps").join("T");
        std::fs::create_dir_all(&map_dir).unwrap();
        std::fs::write(
            map_dir.join("global.cfg"),
            "[name]\nT\n\n[entrypoints]\n2\n1\n10\n0\n1.0\n2.0\n3.0\n0\n0\n0\n1\n5\n[25 DIM] Conleau\n7\n11\n0\n1.0\n2.0\n3.0\n0\n0\n0\n1\n5\nAuray [200][201]\n\n[map]\n1\n",
        )
        .unwrap();
        let map = MapInfo { folder: "T".into(), name: "T".into(), friendly: "T".into(), description: String::new(), picture: None, has_last_state: false };
        assert_eq!(entry_points(&dir, &map), ["[25 DIM] Conleau", "Auray [200][201]"]);
        let _ = std::fs::remove_dir_all(&dir);
    }
}

/// The local date and time of this computer: (year, month, day, hour, minute, second).
pub fn local_now() -> (u32, u32, u32, u32, u32, u32) {
    use windows_sys::Win32::System::SystemInformation::GetLocalTime;
    let mut t = unsafe { std::mem::zeroed() };
    unsafe { GetLocalTime(&mut t) };
    (t.wYear as u32, t.wMonth as u32, t.wDay as u32, t.wHour as u32, t.wMinute as u32, t.wSecond as u32)
}
