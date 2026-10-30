//! OMSI 2 plugin for the launcher (32-bit; OMSI loads it from `plugins\` at start).
//!
//! When the launcher asks (by writing a command into `%APPDATA%\OMSI2-Launcher\plugin\request`), it presses a control of OMSI's start
//! screen from inside the game, by calling the control's own click handler on the game's main thread. The launcher would otherwise
//! have to click at screen coordinates. Commands: `start`, `radio0`, `radio1`, `radio2` (the three load modes), `datetime`
//! ("Cambiar fecha y hora") and `tutorials`.
//!
//! Safety: everything it relies on is checked before it acts. The start form object must have the expected class table, the handler
//! must begin with the expected bytes (this identifies OMSI 2.3.004) and a control passed to a handler must look like an object;
//! otherwise it only writes a message to its status file and does nothing. All reads of OMSI's memory go through
//! `ReadProcessMemory`, which fails instead of crashing.
//!
//! Status file: `started:<cmd>` just before the handler is called (some handlers do not return until the map is loaded or a dialog is
//! closed), `done:<cmd>` when it returned, `error: ...` when the request was refused.

#![allow(non_snake_case)]

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicIsize, AtomicU32, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::System::DataExchange::GlobalFindAtomW;
use windows_sys::Win32::System::Diagnostics::Debug::{ReadProcessMemory, WriteProcessMemory};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::Threading::{GetCurrentProcess, GetCurrentProcessId};
use windows_sys::Win32::UI::WindowsAndMessaging::*;

/// Address of the `Tform_start` class table in Omsi.exe 2.3.004: the first 4 bytes of the form object.
const FORM_VMT: u32 = 0x0067_59C8;

/// Address of the `Tform_tutmenu` (tutorial menu) class table, and where every VCL form keeps its `ModalResult`: a modal form ends
/// when it becomes non-zero. The four tutorial buttons make it 100..103, closing the menu makes it 99.
const TUTMENU_VMT: u32 = 0x0064_A654;
const MODAL_RESULT: u32 = 0x2C0;

/// One thing the launcher can ask for.
struct Command {
    name: &'static str,
    /// Address of the Delphi method to call (register convention: EAX = Self = the form, EDX = Sender).
    handler: u32,
    /// The bytes the method must start with.
    prologue: &'static [u8],
    /// Offset in the form object of the control passed as Sender (0: Sender is nil).
    sender_field: u32,
}

const COMMANDS: &[Command] = &[
    // "¡Iniciar!": Button1Click
    Command { name: "start", handler: 0x0067_68F8, prologue: &[0x55, 0x8B, 0xEC, 0x6A, 0x00, 0x53, 0x56, 0x8B, 0xF0, 0x33, 0xC0, 0x55, 0x68], sender_field: 0 },
    // the three load-mode radio buttons: RadioButton1Click with each radio as Sender
    Command { name: "radio0", handler: 0x0067_8A88, prologue: &[0x55, 0x8B, 0xEC, 0x6A, 0x00, 0x6A, 0x00, 0x53, 0x56, 0x8B, 0xF2, 0x8B, 0xD8], sender_field: 0x434 },
    Command { name: "radio1", handler: 0x0067_8A88, prologue: &[0x55, 0x8B, 0xEC, 0x6A, 0x00, 0x6A, 0x00, 0x53, 0x56, 0x8B, 0xF2, 0x8B, 0xD8], sender_field: 0x438 },
    Command { name: "radio2", handler: 0x0067_8A88, prologue: &[0x55, 0x8B, 0xEC, 0x6A, 0x00, 0x6A, 0x00, 0x53, 0x56, 0x8B, 0xF2, 0x8B, 0xD8], sender_field: 0x43C },
    // "Cambiar fecha y hora": Button8Click, opens the date/time dialog and returns when it is closed
    Command { name: "datetime", handler: 0x0067_9308, prologue: &[0x55, 0x8B, 0xEC, 0x53, 0x8B, 0xD8, 0xB2, 0x01, 0x8B, 0x83, 0x40, 0x04], sender_field: 0 },
    // "Tutoriales": opens the tutorial menu and starts the one that is picked
    Command { name: "tutorials", handler: 0x0067_6B30, prologue: &[0x55, 0x8B, 0xEC, 0x53, 0x56, 0x8B, 0xF0, 0xA1, 0xA0, 0x9D, 0x85, 0x00], sender_field: 0 },
];

const WM_RUN_COMMAND: u32 = WM_APP + 0x4C;

static STARTED: AtomicBool = AtomicBool::new(false);
static STOP: AtomicBool = AtomicBool::new(false);
static IN_HANDLER: AtomicBool = AtomicBool::new(false);
static PREV_PROC: AtomicIsize = AtomicIsize::new(0);
static FORM_OBJECT: AtomicU32 = AtomicU32::new(0);
static SENDER_OBJECT: AtomicU32 = AtomicU32::new(0);
static CURRENT: Mutex<Option<usize>> = Mutex::new(None);

fn dir() -> PathBuf {
    let base = std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
    base.join("OMSI2-Launcher").join("plugin")
}

fn status(text: &str) {
    let d = dir();
    let _ = std::fs::create_dir_all(&d);
    let _ = std::fs::write(d.join("status"), text);
    log(text);
}

fn log(text: &str) {
    use std::io::Write;
    let d = dir();
    let _ = std::fs::create_dir_all(&d);
    if std::fs::metadata(d.join("plugin.log")).is_ok_and(|m| m.len() > 100_000) {
        let _ = std::fs::remove_file(d.join("plugin.log"));
    }
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(d.join("plugin.log")) {
        let _ = writeln!(f, "{text}");
    }
}

/// Reads `n` bytes of this process at `addr`; None if the memory is not readable.
fn read_mem(addr: u32, n: usize) -> Option<Vec<u8>> {
    let mut buf = vec![0u8; n];
    let mut got = 0usize;
    let ok = unsafe { ReadProcessMemory(GetCurrentProcess(), addr as usize as *const _, buf.as_mut_ptr().cast(), n, &mut got) };
    (ok != 0 && got == n).then_some(buf)
}

fn read_u32(addr: u32) -> Option<u32> {
    read_mem(addr, 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

fn class_name(h: HWND) -> String {
    let mut b = [0u16; 64];
    let n = unsafe { GetClassNameW(h, b.as_mut_ptr(), 64) };
    String::from_utf16_lossy(&b[..n.max(0) as usize])
}

struct Search {
    class: &'static str,
    found: Option<HWND>,
}

unsafe extern "system" fn collect(h: HWND, l: LPARAM) -> i32 {
    let search = unsafe { &mut *(l as *mut Search) };
    let mut pid = 0u32;
    unsafe { GetWindowThreadProcessId(h, &mut pid) };
    if pid == unsafe { GetCurrentProcessId() } && class_name(h) == search.class && unsafe { IsWindowVisible(h) } != 0 {
        search.found = Some(h);
        return 0;
    }
    1
}

fn find_window(class: &'static str) -> Option<HWND> {
    let mut search = Search { class, found: None };
    unsafe { EnumWindows(Some(collect), &mut search as *mut _ as LPARAM) };
    search.found
}

/// The Delphi object behind a window. Two independent ways, each accepted only if the object has the start form's class table:
/// the data of the window procedure thunk (VCL's `MakeObjectInstance`), and the `ControlOfs...` window property.
fn window_object(hwnd: HWND, vmt: u32) -> Option<u32> {
    let is_form = |p: u32| p != 0 && read_u32(p) == Some(vmt);
    // the VCL window procedure is a thunk: E8 rel32, next(4), method.code(4), method.data(4) -> data is the object
    let wp = unsafe { GetWindowLongPtrW(hwnd, GWLP_WNDPROC) } as u32;
    if let Some(t) = read_mem(wp, 17) {
        if t[0] == 0xE8 {
            let data = u32::from_le_bytes([t[13], t[14], t[15], t[16]]);
            if is_form(data) {
                return Some(data);
            }
        }
    }
    let tid = unsafe { GetWindowThreadProcessId(hwnd, std::ptr::null_mut()) };
    let hinst = unsafe { GetModuleHandleW(std::ptr::null()) } as usize as u32;
    let name: Vec<u16> = format!("ControlOfs{hinst:08X}{tid:08X}").encode_utf16().chain(Some(0)).collect();
    let atom = unsafe { GlobalFindAtomW(name.as_ptr()) };
    if atom != 0 {
        let p = unsafe { GetPropW(hwnd, atom as usize as *const u16) } as usize as u32;
        if is_form(p) {
            return Some(p);
        }
    }
    None
}

/// Calls the Delphi method `handler` of the object `me` (register convention: EAX = Self, EDX = Sender).
unsafe fn call_delphi_method(handler: u32, me: u32, sender: u32) {
    unsafe {
        std::arch::asm!("call {f}", f = in(reg) handler, in("eax") me, in("edx") sender, clobber_abi("system"));
    }
}

/// Window procedure we put on the start form for a moment: on our private message it runs the chosen handler on the game's own
/// thread, then it puts the original procedure back.
unsafe extern "system" fn command_proc(hwnd: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    let prev = PREV_PROC.load(Ordering::SeqCst);
    if msg == WM_RUN_COMMAND {
        unsafe { SetWindowLongPtrW(hwnd, GWLP_WNDPROC, prev as i32) };
        let index = CURRENT.lock().ok().and_then(|c| *c);
        let Some(cmd) = index.and_then(|i| COMMANDS.get(i)) else { return 0 };
        let (me, sender) = (FORM_OBJECT.load(Ordering::SeqCst), SENDER_OBJECT.load(Ordering::SeqCst));
        // Some handlers do not return until OMSI has loaded the map or a dialog is closed, so the launcher is told BEFORE the call
        // that the action is under way; otherwise it would think nothing happened and act a second time.
        status(&format!("started:{}", cmd.name));
        unsafe { call_delphi_method(cmd.handler, me, sender) };
        IN_HANDLER.store(false, Ordering::SeqCst);
        status(&format!("done:{}", cmd.name));
        return 0;
    }
    let f: WNDPROC = unsafe { std::mem::transmute::<isize, WNDPROC>(prev) };
    unsafe { CallWindowProcW(f, hwnd, msg, w, l) }
}

/// Picks tutorial `n` (1 to 4) in the open tutorial menu the way its buttons do: by giving the modal form its result (100 + n - 1).
/// The menu's own message loop sees it and closes; then the "tutorials" handler starts that tutorial. The menu is woken with an
/// empty message because the loop may be waiting for one.
fn pick_tutorial(n: u32) {
    let Some(hwnd) = find_window("Tform_tutmenu") else {
        status("error: the tutorial menu is not open");
        return;
    };
    let Some(form) = window_object(hwnd, TUTMENU_VMT) else {
        status("error: the tutorial menu was not recognised");
        return;
    };
    // the menu must still be waiting (a result of 0 means "not decided yet")
    if read_u32(form + MODAL_RESULT) != Some(0) {
        status("error: the tutorial menu is already closing");
        return;
    }
    let value = 99 + n;
    let mut written = 0usize;
    let ok = unsafe { WriteProcessMemory(GetCurrentProcess(), (form + MODAL_RESULT) as usize as *mut _, (&value as *const u32).cast(), 4, &mut written) };
    if ok == 0 || written != 4 {
        status("error: could not choose the tutorial");
        return;
    }
    unsafe { PostMessageW(hwnd, WM_NULL, 0, 0) };
    status(&format!("done:tutorial{n}"));
}

fn worker() {
    status("loaded");
    let request = dir().join("request");
    while !STOP.load(Ordering::SeqCst) {
        std::thread::sleep(Duration::from_millis(150));
        if !request.exists() {
            continue;
        }
        // Choosing a tutorial happens while the tutorial menu is open, that is, while the "tutorials" handler is still running.
        let wanted = std::fs::read_to_string(&request).unwrap_or_default();
        if let Some(n) = wanted.trim().strip_prefix("tutorial").and_then(|d| d.parse::<u32>().ok()).filter(|n| (1..=4).contains(n)) {
            let _ = std::fs::remove_file(&request);
            pick_tutorial(n);
            continue;
        }
        // one action at a time: while a handler is running (a dialog is open, the map is loading) requests wait
        if IN_HANDLER.load(Ordering::SeqCst) {
            continue;
        }
        // a request waits until the start screen is there
        let Some(hwnd) = find_window("Tform_start") else { continue };
        let text = std::fs::read_to_string(&request).unwrap_or_default();
        let _ = std::fs::remove_file(&request);
        let name = text.trim();
        let Some(index) = COMMANDS.iter().position(|c| c.name == name) else {
            status(&format!("error: unknown command \"{name}\""));
            continue;
        };
        let cmd = &COMMANDS[index];
        let Some(me) = window_object(hwnd, FORM_VMT) else {
            status("error: the start form was not recognised");
            continue;
        };
        if read_mem(cmd.handler, cmd.prologue.len()).as_deref() != Some(cmd.prologue) {
            status("error: unsupported OMSI version (the handler is not where it was expected)");
            continue;
        }
        let mut sender = 0;
        if cmd.sender_field != 0 {
            sender = read_u32(me + cmd.sender_field).unwrap_or(0);
            // the control must look like a Delphi object: its class table lies inside the program
            if sender == 0 || !read_u32(sender).is_some_and(|vmt| (0x0040_0000..0x0090_0000).contains(&vmt)) {
                status("error: the control to press was not found");
                continue;
            }
        }
        FORM_OBJECT.store(me, Ordering::SeqCst);
        SENDER_OBJECT.store(sender, Ordering::SeqCst);
        if let Ok(mut c) = CURRENT.lock() {
            *c = Some(index);
        }
        IN_HANDLER.store(true, Ordering::SeqCst);
        let prev = unsafe { SetWindowLongPtrW(hwnd, GWLP_WNDPROC, command_proc as *const () as usize as i32) };
        PREV_PROC.store(prev as isize, Ordering::SeqCst);
        if unsafe { PostMessageW(hwnd, WM_RUN_COMMAND, 0, 0) } == 0 {
            unsafe { SetWindowLongPtrW(hwnd, GWLP_WNDPROC, prev) };
            IN_HANDLER.store(false, Ordering::SeqCst);
            status("error: could not queue the request");
        }
    }
}

// ------------------------------------------------------------------------------------------------ the plugin interface OMSI calls

#[no_mangle]
pub extern "system" fn PluginStart(_owner: u32) {
    if !STARTED.swap(true, Ordering::SeqCst) {
        std::thread::spawn(worker);
    }
}

#[no_mangle]
pub extern "system" fn PluginFinalize() {
    STOP.store(true, Ordering::SeqCst);
}

// The variable and trigger accessors are required by OMSI but this plugin has no variables or triggers.
#[no_mangle]
pub extern "system" fn AccessVariable(_index: u32, _value: *mut f32, _write: *mut u8) {}

#[no_mangle]
pub extern "system" fn AccessSystemVariable(_index: u32, _value: *mut f32, _write: *mut u8) {}

#[no_mangle]
pub extern "system" fn AccessStringVariable(_index: u32, _text: *mut u16, _write: *mut u8) {}

#[no_mangle]
pub extern "system" fn AccessTrigger(_index: u32, _active: *mut u8) {}
