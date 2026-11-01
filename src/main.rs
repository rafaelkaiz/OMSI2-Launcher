#![windows_subsystem = "windows"]

mod cfg;
mod chrono;
mod exepatch;
mod free;
mod app_icon;
mod i18n;
mod log;
mod icons;
mod olf;
mod omsi;
mod options_layout;
mod options_ui;
mod paper;
mod pick;
mod plugin;
mod presets;
mod settings;
mod situations;
mod logo;
mod testutil;
mod theme;
mod update;
mod tutorials;

use eframe::egui::{self, Color32, RichText};
use omsi::LoadMode;
use i18n::{tr, trf};
use theme::pal;
use std::path::PathBuf;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

struct DriverEdit {
    existing: Option<cfg::Driver>,
    name: String,
    female: bool,
    day: u32,
    month: u32,
    year: u32,
    error: String,
}

impl DriverEdit {
    fn new(existing: Option<&cfg::Driver>) -> Self {
        let (name, female, birth) = existing
            .map(|d| (d.name.clone(), d.female, d.birth.clone()))
            .unwrap_or_else(|| (String::new(), false, "19900101".into()));
        let num = |r: std::ops::Range<usize>, default: u32| birth.get(r).and_then(|s| s.parse().ok()).unwrap_or(default);
        Self {
            existing: existing.cloned(),
            name,
            female,
            year: num(0..4, 1990),
            month: num(4..6, 1),
            day: num(6..8, 1),
            error: String::new(),
        }
    }

    fn birth(&self) -> String {
        format!("{:04}{:02}{:02}", self.year, self.month, self.day)
    }
}

#[derive(Clone, Copy, PartialEq)]
enum TimeMode {
    Situation,
    Custom,
}

struct App {
    settings: settings::Settings,
    settings_open: bool,
    settings_tab: usize,
    tut_open: bool,
    tut_sel: usize,
    tut_entries: Vec<tutorials::Entry>,
    time_mode: TimeMode,
    /// The date in the "Elegir fecha y hora" dialog was chosen by the user (otherwise OMSI keeps the date of the situation).
    date_touched: bool,
    sec: u32,
    time_dialog: Option<TimeDialog>,
    date: (u32, u32, u32),
    clock: (u32, u32),
    omsi: Option<PathBuf>,
    version: String,
    maps: Vec<omsi::MapInfo>,
    drivers: Vec<cfg::Driver>,
    map: usize,
    driver: usize,
    mode: LoadMode,
    status: Arc<Mutex<String>>,
    busy: Arc<Mutex<bool>>,
    /// The tutorial (1..4) being started, so the loading screen names it instead of the selected map.
    loading_tutorial: Option<u8>,
    running: (Instant, bool),
    options: Option<cfg::Options>,
    options_ui: options_ui::State,
    driver_edit: Option<DriverEdit>,
    staff_open: bool,
    staff_sel: usize,
    staff_rec: Option<(PathBuf, cfg::Record)>,
    staff_labels: Arc<HashMap<String, String>>,
    confirm_delete: Option<usize>,
    situations: Vec<situations::Situation>,
    sit_sel: Option<usize>,
    staff_page: u8,
    spawn_sel: usize,
    spawn_points: Vec<String>,
    spawn_map: Option<usize>,
    sit_only_map: bool,
    chrono_cache: Option<((String, chrono::Date, &'static str), String)>,
    close_requested: Arc<AtomicBool>,
    minimize_requested: Arc<AtomicBool>,
    restore_view: Arc<AtomicBool>,
    raise_view: Arc<AtomicBool>,
    plugin_ok: bool,
    /// The 4 GB patch asked for in Settings and waiting for the user's confirmation (`true`: apply, `false`: remove).
    patch_confirm: Option<bool>,
    plugin_checked: Instant,
    update: update::Shared,
    locate_dir: String,
    locate_error: bool,
    born: Instant,
    entered_full: bool,
    load_started: Option<Instant>,
}

pub fn big_button(ui: &mut egui::Ui, text: &str, width: f32) -> egui::Response {
    ui.add(
        egui::Button::new(RichText::new(text).size(22.0).color(pal().light))
            .fill(pal().dark)
            .corner_radius(0.0)
            .min_size(egui::vec2(width, 40.0)),
    )
}

/// Paints `icon` at the left end of a big button.
fn paint_icon(ui: &egui::Ui, r: &egui::Response, icon: icons::Icon, color: Color32) {
    let c = egui::pos2(r.rect.left() + 30.0, r.rect.center().y);
    icons::paint(ui.painter(), icon, c, 10.0, color);
}

fn big_icon_button(ui: &mut egui::Ui, text: &str, width: f32, icon: icons::Icon) -> egui::Response {
    let r = big_button(ui, text, width);
    paint_icon(ui, &r, icon, pal().light);
    r
}

/// A block of the start screen: content followed by a thick orange rule, like OMSI's own panels.
fn section(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::new().inner_margin(egui::Margin::symmetric(16, 10)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        add(ui);
    });
    let r = ui.max_rect();
    let y = ui.cursor().top();
    ui.painter().rect_filled(egui::Rect::from_min_max(egui::pos2(r.left(), y), egui::pos2(r.right(), y + 6.0)), 0.0, pal().line);
    ui.add_space(6.0);
}

impl App {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        egui_extras::install_image_loaders(&cc.egui_ctx);
        let settings = settings::Settings::load();
        i18n::set_lang(settings.lang);
        theme::apply(&cc.egui_ctx, settings.theme);
        // keep the backup folder tidy without slowing the start
        std::thread::spawn(|| {
            let n = cfg::prune_backups(3, 0);
            if n > 0 {
                log::line(&format!("pruned {n} old backups"));
            }
        });
        let omsi = omsi::find_omsi_dir(settings.omsi_dir.clone()).filter(|_| std::env::var("OMSI2_OPEN").as_deref() != Ok("missing")); // dev: OMSI2_OPEN=missing shows the "where is OMSI" screen
        if let Some(d) = &omsi {
            olf::init(d);
        }
        if let (Some(d), true) = (&omsi, settings.remove_closecheck) {
            omsi::remove_closecheck(d);
        }
        let (maps, drivers, options) = match &omsi {
            Some(d) => (omsi::list_maps(d), cfg::list_drivers(d), cfg::Options::load(d)),
            None => (vec![], vec![], None),
        };
        let last = |key: &str| options.as_ref().and_then(|o| o.get(key, 0)).unwrap_or_default().to_lowercase();
        let (last_map, last_driver) = (last("last_map"), last("last_driver"));
        let map = maps.iter().position(|m| last_map.contains(&format!("\\{}\\", m.folder.to_lowercase()))).unwrap_or(0);
        let driver = drivers
            .iter()
            .position(|d| last_driver.ends_with(&d.path.file_name().unwrap_or_default().to_string_lossy().to_lowercase()))
            .unwrap_or(0);
        let status = if omsi.is_some() {
            tr("Listo.")
        } else {
            tr("No encuentro OMSI 2. Pasa su carpeta como argumento o en OMSI_DIR.")
        };
        let (y, m, d) = cfg::today();
        let sit_list = omsi.as_deref().map(situations::list).unwrap_or_default();
        let mut options_ui = options_ui::State::default();
        options_ui.open = std::env::var("OMSI2_OPEN").as_deref() == Ok("options"); // dev shortcut for screenshots
        options_ui.set_tab(std::env::var("OMSI2_TAB").ok().and_then(|t| t.parse().ok()).unwrap_or(0));
        Self {
            time_mode: TimeMode::Situation,
            date_touched: false,
            sec: 0,
            time_dialog: if std::env::var("OMSI2_OPEN").as_deref() == Ok("time") { Some(TimeDialog { time: (7, 14, 14), date: (7, 1, 2014), date_touched: true }) } else { None }, // dev shortcut for screenshots
            date: (d, m, y as u32),
            clock: (8, 0),
            version: omsi.as_deref().and_then(omsi::version).unwrap_or_default(),
            omsi,
            maps,
            drivers,
            map,
            driver,
            mode: LoadMode::LastState,
            settings,
            settings_open: std::env::var("OMSI2_OPEN").as_deref() == Ok("settings"), // dev shortcut for screenshots
            settings_tab: std::env::var("OMSI2_TAB").ok().and_then(|t| t.parse().ok()).unwrap_or(0),
            tut_open: false,
            tut_sel: 0,
            tut_entries: vec![],
            staff_open: std::env::var("OMSI2_OPEN").as_deref() == Ok("staff"), // dev shortcut for screenshots (OMSI2_TAB = page)
            staff_sel: driver,
            staff_rec: None,
            staff_labels: Arc::new(HashMap::new()),
            confirm_delete: None,
            situations: sit_list,
            sit_sel: None,
            staff_page: if std::env::var("OMSI2_OPEN").as_deref() == Ok("staff") { std::env::var("OMSI2_TAB").ok().and_then(|t| t.parse().ok()).unwrap_or(0).min(1) } else { 0 },
            spawn_sel: 0,
            spawn_points: Vec::new(),
            spawn_map: None,
            sit_only_map: true,
            chrono_cache: None,
            close_requested: Arc::new(AtomicBool::new(false)),
            minimize_requested: Arc::new(AtomicBool::new(false)),
            restore_view: Arc::new(AtomicBool::new(false)),
            raise_view: Arc::new(AtomicBool::new(false)),
            plugin_ok: false,
            patch_confirm: None,
            plugin_checked: Instant::now() - Duration::from_secs(10),
            update: std::sync::Arc::new(std::sync::Mutex::new(update::State::Idle)),
            locate_dir: String::new(),
            locate_error: false,
            born: Instant::now(),
            entered_full: false,
            load_started: None,
            status: Arc::new(Mutex::new(status.into())),
            // dev shortcut for screenshots: OMSI2_OPEN=loading shows the loading screen
            busy: Arc::new(Mutex::new(matches!(std::env::var("OMSI2_OPEN").as_deref(), Ok("loading") | Ok("loadingtut")))),
            loading_tutorial: (std::env::var("OMSI2_OPEN").as_deref() == Ok("loadingtut")).then_some(1), // dev: OMSI2_OPEN=loadingtut
            running: (Instant::now() - Duration::from_secs(10), false),
            options,
            options_ui,
            driver_edit: None,
        }
    }

    /// The saved situation that counts: the one picked in the list, but only while it is one the list shows. With "only the selected
    /// map's situations" on, a situation of another map (kept from before the map was changed) does not count, so a map without
    /// situations has none selected.
    fn selected_situation(&self) -> Option<&situations::Situation> {
        let s = self.sit_sel.and_then(|i| self.situations.get(i))?;
        let folder = self.maps.get(self.map).map(|m| m.folder.as_str()).unwrap_or_default();
        (!self.sit_only_map || s.map_folder.eq_ignore_ascii_case(folder)).then_some(s)
    }

    fn set_status(&self, s: &str) {
        *self.status.lock().unwrap() = s.into();
    }

    fn reload_drivers(&mut self, select: Option<&str>) {
        let Some(dir) = &self.omsi else { return };
        self.drivers = cfg::list_drivers(dir);
        self.driver = select.and_then(|n| self.drivers.iter().position(|d| d.name == n)).unwrap_or(0);
    }

    fn start(&mut self, ctx: &egui::Context, mode: LoadMode) {
        log::line(&format!("start: map #{} driver #{} situation {:?}", self.map, self.driver, self.sit_sel));
        self.loading_tutorial = if let LoadMode::Tutorial(n) = mode { Some(n) } else { None };
        let (Some(dir), Some(map)) = (self.omsi.clone(), self.maps.get(self.map).cloned()) else { return };
        if !self.plugin_ok {
            return;
        }
        // a date or time chosen here must not be overruled by the "use current date/time" options of OMSI
        if self.time_mode == TimeMode::Custom {
            if let Some(o) = self.options.as_mut() {
                let flags: &[&str] = if self.date_touched { &["useActTime", "useActDate", "useActYear"] } else { &["useActTime"] };
                for k in flags {
                    if o.has(k) {
                        o.set_flag(k, "GENERAL", false);
                        self.options_ui.dirty = true;
                    }
                }
            }
        }
        // Settings changed on the start screen (date, time...) are written before OMSI reads them.
        if self.options_ui.dirty && !self.running.1 {
            if let Some(o) = &self.options {
                match o.save() {
                    Ok(()) => self.options_ui.dirty = false,
                    Err(e) => return self.set_status(&e),
                }
            }
        }
        let (driver, windowed) = (self.drivers.get(self.driver).map(|d| d.name.clone()), self.settings.windowed);
        let time = match (self.time_mode, self.date_touched) {
            (TimeMode::Custom, true) => omsi::TimeChoice::Custom(self.date.0, self.date.1, self.date.2, self.clock.0, self.clock.1, self.sec),
            (TimeMode::Custom, false) => omsi::TimeChoice::TimeOnly(self.clock.0, self.clock.1, self.sec),
            _ => omsi::TimeChoice::Situation,
        };
        let situation = self.selected_situation().map(|s| (s.name.clone(), s.occurrence));
        let spawn = (mode == LoadMode::NoBuses).then(|| self.spawn_points.get(self.spawn_sel).cloned()).flatten();
        let (restore_flag, raise_flag) = (self.restore_view.clone(), self.raise_view.clone());
        let (close_flag, close_on, minimize_flag) = (self.close_requested.clone(), self.settings.close_on_start, self.minimize_requested.clone());
        self.load_started = Some(Instant::now());
        if self.settings.remove_closecheck {
            omsi::remove_closecheck(&dir);
        }
        // The loading screen stays full screen, but not topmost: OMSI creates its own Direct3D device while it starts, and a
        // window forced above everything may be what made that fail with D3DERR_DEVICELOST.
        ctx.send_viewport_cmd(egui::ViewportCommand::WindowLevel(egui::WindowLevel::Normal));
        let (status, busy, ctx) = (self.status.clone(), self.busy.clone(), ctx.clone());
        *busy.lock().unwrap() = true;
        std::thread::spawn(move || {
            let set = |s: &str| {
                log::line(s);
                *status.lock().unwrap() = s.into();
                ctx.request_repaint();
            };
            let sit = situation.as_ref().map(|(n, o)| (n.as_str(), *o));
            match omsi::launch(&dir, &map, driver.as_deref(), windowed, mode, &time, sit, spawn.as_deref(), &set, || {
                raise_flag.store(true, Ordering::Relaxed);
                ctx.request_repaint();
            }) {
                Err(e) => {
                    set(&e);
                    restore_flag.store(true, Ordering::Relaxed);
                }
                // OMSI is loading the map: the launcher has done its job
                Ok(()) if close_on => close_flag.store(true, Ordering::Relaxed),
                // OMSI is loading the map and must be seen: step out of its way
                Ok(()) => minimize_flag.store(true, Ordering::Relaxed),
            }
            *busy.lock().unwrap() = false;
            ctx.request_repaint();
        });
    }

    /// A window like OMSI's "Expediente del personal": the statistics OMSI keeps in a driver's file, with the same labels.
    fn staff_window(&mut self, ctx: &egui::Context, running: bool) {
        if !self.staff_open || self.drivers.is_empty() {
            self.staff_open = false;
            return;
        }
        self.staff_sel = self.staff_sel.min(self.drivers.len() - 1);
        let path = self.drivers[self.staff_sel].path.clone();
        if self.staff_rec.as_ref().map(|(p, _)| p) != Some(&path) {
            self.staff_rec = cfg::read_record(&path).map(|r| (path.clone(), r));
        }
        if self.staff_labels.is_empty() {
            self.staff_labels = Arc::new(self.omsi.as_deref().map(|d| olf::load(d, self.settings.lang)).unwrap_or_default());
        }
        let labels = self.staff_labels.clone();
        let l = |k: &str| labels.get(k).cloned().unwrap_or_else(|| k.to_string());
        let open;
        let mut close_staff = false;
        let names: Vec<String> = self.drivers.iter().map(|d| d.name.clone()).collect();
        let title = if labels.contains_key("DG_Driver") { l("DG_Driver") } else { tr("Expediente del personal").to_string() };
        let monitor_h = ctx.input(|i| i.viewport().monitor_size).map_or(1080.0, |m| m.y);
        // the sheet is smaller when the screen is short
        let k = ((monitor_h - 140.0 - 140.0) / paper::HEIGHT).clamp(0.55, 1.0);
        let size = egui::vec2(paper::WIDTH * k + 30.0, paper::HEIGHT * k + 140.0);
        open = free::window(ctx, "staff", &title, size, |ui| {
            ui.horizontal(|ui| {
                ui.label(l("DG_Driver_Label1"));
                pick::pick(ui, "staff-driver", 220.0, &names, &mut self.staff_sel);
            });
            ui.horizontal_wrapped(|ui| {
                let current = self.staff_sel == self.driver;
                let set = if current { l("DG_Driver_Button1_NotEnabled") } else { l("DG_Driver_Button1_Enabled") };
                if ui.add_enabled(!current, egui::Button::new(set)).clicked() {
                    self.driver = self.staff_sel;
                }
                if ui.add_enabled(!running, egui::Button::new(l("DG_Driver_Button2"))).clicked() {
                    self.driver_edit = Some(DriverEdit::new(None));
                }
                if ui.add_enabled(!running, egui::Button::new(l("DG_Driver_Button3"))).clicked() {
                    self.driver_edit = Some(DriverEdit::new(self.drivers.get(self.staff_sel)));
                }
                if ui.add_enabled(!running, egui::Button::new(l("DG_Driver_Button4"))).clicked() {
                    self.confirm_delete = Some(self.staff_sel);
                }
            });
            if running {
                ui.label(RichText::new(tr("Cierra OMSI 2 para editar conductores.")).italics());
            }
            ui.separator();
            let d = self.drivers[self.staff_sel].clone();
            let date = |s: &str| if s.len() == 8 { format!("{}/{}/{}", &s[6..8], &s[4..6], &s[0..4]) } else { s.to_string() };
            let photo = d.path.with_extension("bmp");
            let photo = if photo.is_file() { Some(photo) } else { self.omsi.as_ref().map(|o| o.join("Drivers").join("nopic.bmp")) };
            let (page, _) = ui.allocate_exact_size(egui::vec2(paper::WIDTH * k, paper::HEIGHT * k), egui::Sense::hover());
            let p = ui.painter_at(page);
            paper::paint(&p, page, k);
            let at = |x: f32, y: f32| page.left_top() + egui::vec2(x * k, y * k);
            let ink = Color32::from_rgb(18, 18, 18);
            let mono = egui::FontId::monospace(13.0 * k);
            // the paper's lettering is heavy: each text is drawn twice, a hair apart
            let bold = |pos: egui::Pos2, anchor: egui::Align2, text: &str, font: &egui::FontId, color: Color32| {
                p.text(pos, anchor, text, font.clone(), color);
                p.text(pos + egui::vec2(0.8 * k, 0.0), anchor, text, font.clone(), color);
            };
            let title_text = l("DG_Driver");
            let title_size = (30.0 * k).min(270.0 * k / (title_text.chars().count() as f32 * 0.5).max(1.0));
            bold(at(185.0, 30.0), egui::Align2::CENTER_CENTER, &title_text, &egui::FontId::proportional(title_size), ink);
            bold(at(350.0, 21.0), egui::Align2::LEFT_CENTER, &trf("Página {}", &[&(self.staff_page + 1).to_string()]), &egui::FontId::proportional(14.0 * k), ink);
            if let Some((_, r)) = &self.staff_rec {
                if self.staff_page == 0 {
                    if let Some(img) = photo.filter(|f| f.is_file()) {
                        let uri = format!("file:///{}", img.display().to_string().replace('\\', "/"));
                        let rect = egui::Rect::from_min_size(at(59.0, 73.0), egui::vec2(120.0 * k, 160.0 * k));
                        egui::Image::new(uri).fit_to_exact_size(rect.size()).paint_at(ui, rect);
                        p.rect_stroke(rect, 0.0, egui::Stroke::new(1.0, Color32::from_rgb(60, 55, 45)), egui::StrokeKind::Outside);
                    }
                    bold(at(270.0, 256.0), egui::Align2::LEFT_CENTER, &if d.female { l("DG_Driver_Mrs") } else { l("DG_Driver_Mr") }, &mono, ink);
                    let driving = r.driving().map(|v| format!("{v:.1} % ({})", l("DG_Driver_Rating_Driving_0"))).unwrap_or_else(|| "-".into());
                    let comfort = r.comfort().map(|(v, got, max)| format!("{v:.1} % ({got}/{max})")).unwrap_or_else(|| "-".into());
                    // (height on the sheet, OMSI's label, value, shown in red)
                    let rows: [(f32, &str, String, bool); 16] = [
                        (276.0, "DG_Driver_Name", d.name.clone(), false),
                        (296.0, "DG_Driver_Birthday", date(&d.birth), false),
                        (316.0, "DG_Driver_DateOfHire", date(&r.hired), false),
                        (356.0, "DG_Driver_Range", format!("{:.1} km", r.hektometers as f64 / 10.0), false),
                        (376.0, "DG_Driver_Busstops", r.stops.to_string(), false),
                        (396.0, "DG_Driver_Busstops_Early", r.early.to_string(), true),
                        (416.0, "DG_Driver_Busstops_Late", r.late.to_string(), true),
                        (456.0, "DG_Driver_Tickets", r.tickets_sold.to_string(), false),
                        (476.0, "DG_Driver_Cash", format!("{:.2}", r.cash), false),
                        (516.0, "DG_Driver_Crashs", r.crashes[0].to_string(), true),
                        (536.0, "DG_Driver_HitAndRun", r.crashes[1].to_string(), true),
                        (556.0, "DG_Driver_HitAndRun_Heavy", r.crashes[2].to_string(), true),
                        (576.0, "DG_Driver_PedCrashs", r.crashes[3].to_string(), true),
                        (616.0, "DG_Driver_Rating_Driving", driving, false),
                        (636.0, "DG_Driver_Rating_Ticket", "-".to_string(), false), // OMSI's ticket-selling rating formula is not known here
                        (656.0, "DG_Driver_Rating_Comfort", comfort, false),
                    ];
                    for (y, key, value, bad) in rows {
                        // long labels (some languages) are shrunk so they stop before the values
                        let label = l(key);
                        let fit = (170.0 * k / (label.chars().count() as f32 * 6.4 * k)).min(1.0);
                        p.text(at(74.0, y), egui::Align2::LEFT_CENTER, &label, egui::FontId::proportional(13.5 * k * fit), ink);
                        bold(at(269.0, y), egui::Align2::LEFT_CENTER, &value, &mono, if bad { Color32::from_rgb(205, 0, 0) } else { ink });
                        p.line_segment([at(249.0, y + 9.0), at(438.0, y + 9.0)], egui::Stroke::new(1.0, Color32::from_rgb(70, 64, 52)));
                    }
                } else {
                    bold(at(84.0, 71.0), egui::Align2::LEFT_CENTER, &l("DG_Driver_Buses"), &egui::FontId::proportional(14.5 * k), ink);
                    // name, dots up to a fixed column, then the distance driven with that bus
                    for (i, (bus, hm)) in r.buses.iter().take(22).enumerate() {
                        let mut name: String = bus.chars().take(32).collect();
                        if bus.chars().count() > 32 {
                            name.push_str("..");
                        }
                        let line = format!("{name:.<34} {:.1} km", *hm as f64 / 10.0);
                        bold(at(78.0, 127.0 + i as f32 * 25.0), egui::Align2::LEFT_CENTER, &line, &mono, ink);
                    }
                }
            }
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                for n in 0..2u8 {
                    if ui.selectable_label(self.staff_page == n, trf("Página {}", &[&(n + 1).to_string()])).clicked() {
                        self.staff_page = n;
                    }
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button(tr("Cerrar")).clicked() {
                        close_staff = true;
                    }
                });
            });
        });
        if !open || close_staff {
            self.staff_open = false;
        }
        self.confirm_window(ctx);
    }

    /// Asks before deleting a driver; the driver's file is copied to the backup folder first.
    fn confirm_window(&mut self, ctx: &egui::Context) {
        let Some(i) = self.confirm_delete else { return };
        let Some(d) = self.drivers.get(i).cloned() else {
            self.confirm_delete = None;
            return;
        };
        let mut done = false;
        let mut delete = false;
        if !free::window(ctx, "confirm", tr("Eliminar"), egui::vec2(360.0, 90.0), |ui| {
            ui.label(trf("¿Eliminar al conductor {}? Se guardará antes una copia de seguridad.", &[&d.name]));
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if big_button(ui, tr("Eliminar"), 120.0).clicked() {
                    delete = true;
                }
                if big_button(ui, tr("Cancelar"), 120.0).clicked() {
                    done = true;
                }
            });
        }) {
            done = true;
        }
        if delete {
            match cfg::delete_driver(&d) {
                Ok(_) => {
                    self.driver = if self.driver > i { self.driver - 1 } else { self.driver };
                    self.reload_drivers(None);
                    self.driver = self.driver.min(self.drivers.len().saturating_sub(1));
                    self.staff_sel = self.staff_sel.min(self.drivers.len().saturating_sub(1));
                    self.staff_rec = None;
                    self.set_status(tr("Conductor eliminado (copia de seguridad en %APPDATA%\\OMSI2-Launcher\\backups)."));
                }
                Err(e) => self.set_status(&e),
            }
            done = true;
        }
        if done {
            self.confirm_delete = None;
        }
    }

    fn load_tutorials(&mut self) {
        let Some(dir) = &self.omsi else { return };
        let lang = self.options.as_ref().and_then(|o| o.get("language", 0)).unwrap_or_else(|| "ENG".into());
        self.tut_entries = tutorials::load(dir, &lang);
        self.tut_sel = 0;
    }

    /// A window like OMSI's own tutorial menu: four tutorials, a picture and a description.
    fn tutorials_window(&mut self, ctx: &egui::Context, can_start: bool) {
        if !self.tut_open {
            return;
        }
        let open;
        let mut start = None;
        open = free::window(ctx, "tutorials", tr("Tutoriales"), egui::vec2(700.0, 330.0), |ui| {
            ui.horizontal_top(|ui| {
                ui.vertical(|ui| {
                    for (i, name) in [tr("Tutorial I"), tr("Tutorial II"), tr("Tutorial III"), tr("Tutorial IV")].iter().enumerate() {
                        let sel = self.tut_sel == i + 1;
                        // drawn by hand: icon tile on the left, the name centred in the space that is left (like OMSI's menu)
                        let (rect, resp) = ui.allocate_exact_size(egui::vec2(260.0, 64.0), egui::Sense::click());
                        let fg = if sel { Color32::WHITE } else { pal().light };
                        ui.painter().rect_filled(rect, 0.0, if sel { pal().line } else if resp.hovered() { pal().dark.gamma_multiply(0.85) } else { pal().dark });
                        let tile = egui::Rect::from_min_size(rect.left_top() + egui::vec2(8.0, 8.0), egui::vec2(48.0, 48.0));
                        ui.painter().rect_filled(tile, 0.0, Color32::from_black_alpha(70));
                        let icon = [icons::Icon::Clock, icons::Icon::Bus, icons::Icon::People, icons::Icon::Fleet][i];
                        icons::paint(ui.painter(), icon, tile.center(), 16.0, fg);
                        let text_area = egui::Rect::from_min_max(egui::pos2(tile.right() + 8.0, rect.top()), rect.right_bottom());
                        ui.painter().text(text_area.center(), egui::Align2::CENTER_CENTER, *name, egui::FontId::proportional(20.0), fg);
                        if resp.clicked() {
                            self.tut_sel = i + 1;
                        }
                        ui.add_space(4.0);
                    }
                });
                ui.add_space(8.0);
                ui.vertical(|ui| {
                    ui.set_width(380.0);
                    if let Some(e) = self.tut_entries.get(self.tut_sel) {
                        if let Some(p) = &e.picture {
                            let uri = format!("file:///{}", p.display().to_string().replace('\\', "/"));
                            ui.add(egui::Image::new(uri).fit_to_exact_size(egui::vec2(380.0, 209.0)));
                        }
                        ui.label(RichText::new(&e.title).strong());
                        egui::ScrollArea::vertical().max_height(120.0).show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.add(egui::Label::new(&e.text).wrap());
                        });
                    }
                });
            });
            ui.add_space(6.0);
            let label = if self.tut_sel == 0 { tr("Elige un tutorial").to_string() } else { format!("{} {}", tr("Iniciar tutorial"), self.tut_sel) };
            if ui.add_enabled(can_start && self.tut_sel > 0, egui::Button::new(RichText::new(label).size(20.0).color(Color32::WHITE)).fill(pal().dark).corner_radius(0.0).min_size(egui::vec2(240.0, 40.0))).clicked() {
                start = Some(self.tut_sel as u8);
            }
        });
        self.tut_open = open;
        if let Some(n) = start {
            self.tut_open = false;
            self.start(ctx, LoadMode::Tutorial(n));
        }
    }

    /// The launcher's own settings (theme, OMSI folder, window mode); kept apart from OMSI's options.
    fn settings_window(&mut self, ctx: &egui::Context) {
        if !self.settings_open {
            return;
        }
        let open;
        let mut changed = false;
        let mut plugin_msg: Option<String> = None;
        let mut open_url: Option<String> = None;
        let mut lang_msg: Option<String> = None;
        // one rounded card per topic, each with its title
        let card = |ui: &mut egui::Ui, title: &str, show: bool, add: &mut dyn FnMut(&mut egui::Ui)| {
            if !show {
                return;
            }
            egui::Frame::new()
                .fill(ui.visuals().faint_bg_color)
                .stroke(ui.visuals().widgets.noninteractive.bg_stroke)
                .corner_radius(6.0)
                .inner_margin(egui::Margin::same(10))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(RichText::new(title).strong().size(15.0).color(pal().dark));
                    ui.add_space(4.0);
                    add(ui);
                });
            ui.add_space(8.0);
        };
        open = free::window(ctx, "settings", tr("Ajustes del launcher"), egui::vec2(820.0, 560.0), |ui| {
            ui.set_min_width(780.0);
            ui.label(RichText::new(tr("Estos ajustes son del launcher; las opciones de OMSI 2 están en el botón Opciones.")).italics().size(12.0));
            ui.add_space(8.0);
            // one button per section, like OMSI's own tabs
            ui.horizontal(|ui| {
                for (i, name) in [tr("Apariencia"), "OMSI 2", "Plugin", tr("Copias de seguridad"), tr("Actualizaciones")].into_iter().enumerate() {
                    let on = self.settings_tab == i;
                    let b = egui::Button::new(RichText::new(name).strong().color(if on { Color32::WHITE } else { pal().text }))
                        .fill(if on { pal().dark } else { pal().field })
                        .min_size(egui::vec2(0.0, 30.0));
                    if ui.add(b).clicked() {
                        self.settings_tab = i;
                    }
                }
            });
            ui.add_space(8.0);
            egui::ScrollArea::vertical().max_height(330.0).min_scrolled_height(330.0).auto_shrink([false, false]).show(ui, |ui| {
                card(ui, tr("Apariencia"), self.settings_tab == 0, &mut |ui| {
                    ui.horizontal(|ui| {
                        ui.label(tr("Idioma del launcher"));
                        egui::ComboBox::from_id_salt("launcher_lang").selected_text(self.settings.lang.name()).show_ui(ui, |ui| {
                            for l in i18n::available(self.omsi.as_deref()) {
                                if ui.selectable_value(&mut self.settings.lang, l, l.name()).changed() {
                                    i18n::set_lang(l);
                                    changed = true;
                                }
                            }
                        });
                    });
                    ui.horizontal(|ui| {
                        ui.add_space(4.0);
                        // another language: its texts go in a file of the "lang" folder (the template has every text to translate)
                        if ui.small_button(tr("Crear plantilla de traducción")).clicked() {
                            match i18n::write_template() {
                                Ok(p) => omsi::open_folder(p.parent().unwrap_or(std::path::Path::new("."))),
                                Err(e) => lang_msg = Some(e),
                            }
                        }
                        if ui.small_button(tr("Abrir carpeta de idiomas")).clicked() {
                            let dir = i18n::lang_dir();
                            let _ = std::fs::create_dir_all(&dir);
                            omsi::open_folder(&dir);
                        }
                    });
                    ui.label(RichText::new(tr("Los idiomas de la carpeta Languages de OMSI aparecen solos; para sus textos del launcher añade un archivo <CÓDIGO>.txt en la carpeta de idiomas.")).italics().size(12.0));
                    ui.horizontal(|ui| {
                        ui.label(tr("Tema"));
                        for t in theme::Theme::ALL {
                            if ui.radio_value(&mut self.settings.theme, t, t.label()).changed() {
                                theme::apply(ui.ctx(), t);
                                changed = true;
                            }
                        }
                    });
                    if ui.checkbox(&mut self.settings.fullscreen, tr("Abrir el launcher a pantalla completa (F11)")).changed() {
                        changed = true;
                        set_fullscreen(ui.ctx(), self.settings.fullscreen);
                    }
                    if ui.checkbox(&mut self.settings.always_on_top, tr("Mantener el launcher al frente mientras OMSI se abre")).changed() {
                        changed = true;
                    }
                });
                card(ui, "OMSI 2", self.settings_tab == 1, &mut |ui| {
                    changed |= ui.checkbox(&mut self.settings.windowed, tr("Abrir OMSI en modo ventana")).changed();
                    changed |= ui.checkbox(&mut self.settings.close_on_start, tr("Cerrar el launcher al iniciar OMSI")).changed();
                    changed |= ui.checkbox(&mut self.settings.remove_closecheck, tr("Eliminar el archivo \"closecheck\" cuando OMSI 2 falle")).changed();
                    ui.add_space(4.0);
                    let folder = self.omsi.as_ref().map(|p| p.display().to_string()).unwrap_or_else(|| tr("no encontrada").into());
                    ui.label(trf("Carpeta: {}", &[&folder]));
                    ui.horizontal(|ui| {
                        ui.label(tr("Otra carpeta:"));
                        let mut dir = self.settings.omsi_dir.as_ref().map(|p| p.display().to_string()).unwrap_or_default();
                        if ui.add(egui::TextEdit::singleline(&mut dir).desired_width(520.0)).changed() {
                            self.settings.omsi_dir = (!dir.trim().is_empty()).then(|| PathBuf::from(dir.trim()));
                            changed = true;
                        }
                        if ui.button(tr("Examinar...")).clicked() {
                            if let Some(p) = omsi::browse_folder(tr("Elige la carpeta de OMSI 2")) {
                                self.settings.omsi_dir = Some(p);
                                changed = true;
                            }
                        }
                    });
                    ui.label(RichText::new(tr("(la carpeta nueva se usa al reiniciar el launcher)")).italics().size(12.0));
                    ui.separator();
                    // the 4 GB patch: the only change the launcher makes to a file OMSI installs, so it asks first
                    ui.label(RichText::new(tr("Parche de 4 GB")).strong());
                    ui.label(RichText::new(tr("Cambia un único bit de la cabecera de Omsi.exe para que OMSI pueda usar hasta 4 GB de memoria en vez de 2 GB. No acelera nada; evita cierres por falta de memoria. Antes se guarda una copia de Omsi.exe en la carpeta de copias. Steam puede deshacerlo al actualizar o verificar los archivos.")).italics().size(12.0));
                    let state = self.omsi.as_deref().and_then(exepatch::status);
                    ui.label(match state {
                        Some(true) => tr("Omsi.exe: parche de 4 GB activado."),
                        Some(false) => tr("Omsi.exe: parche de 4 GB desactivado."),
                        None => tr("Omsi.exe: no se pudo leer."),
                    });
                    if let (Some(on), Some(target)) = (state, self.patch_confirm) {
                        if on == target {
                            self.patch_confirm = None;
                        } else {
                            ui.label(RichText::new(tr("¿Modificar Omsi.exe ahora? Se guarda una copia antes.")).strong());
                            ui.horizontal(|ui| {
                                if ui.add_enabled(!self.running.1, egui::Button::new(tr("Confirmar"))).clicked() {
                                    if let Some(o) = self.omsi.as_deref() {
                                        plugin_msg = Some(match exepatch::apply(o, target) {
                                            Ok(_) if target => tr("Parche de 4 GB aplicado a Omsi.exe (copia en la carpeta de copias).").to_string(),
                                            Ok(_) => tr("Parche de 4 GB quitado de Omsi.exe (copia en la carpeta de copias).").to_string(),
                                            Err(e) => e,
                                        });
                                    }
                                    self.patch_confirm = None;
                                }
                                if ui.button(tr("Cancelar")).clicked() {
                                    self.patch_confirm = None;
                                }
                            });
                        }
                    } else if let Some(on) = state {
                        let idle = !self.running.1;
                        if ui.add_enabled(idle, egui::Button::new(if on { tr("Quitar parche de 4 GB") } else { tr("Aplicar parche de 4 GB") })).clicked() {
                            self.patch_confirm = Some(!on);
                        }
                        if !idle {
                            ui.label(RichText::new(tr("Cierra OMSI para modificar Omsi.exe.")).italics().size(12.0));
                        }
                    }
                });
                card(ui, tr("Inicio directo (plugin)"), self.settings_tab == 2, &mut |ui| {
                    ui.label(RichText::new(tr("Añade dos archivos nuevos a la carpeta plugins de OMSI (OMSI2Launcher.opl y OMSI2LauncherPlugin.dll) y no modifica ninguno existente. Con él, ¡Iniciar! se pulsa dentro de OMSI en vez de hacer clic con el ratón. Funciona la próxima vez que abras OMSI.")).italics().size(12.0));
                    ui.label(plugin::status_text(self.omsi.as_deref()));
                    let can_touch = self.omsi.is_some() && !self.running.1;
                    ui.horizontal(|ui| {
                        let installed = self.omsi.as_deref().is_some_and(plugin::installed);
                        if ui.add_enabled(can_touch, egui::Button::new(tr("Instalar plugin"))).clicked() {
                            if let Some(o) = self.omsi.as_deref() {
                                plugin_msg = Some(match plugin::install(o) {
                                    Ok(()) => tr("Plugin instalado en la carpeta plugins de OMSI. Funciona la próxima vez que abras OMSI.").to_string(),
                                    Err(e) => e,
                                });
                            }
                        }
                        if ui.add_enabled(can_touch && installed, egui::Button::new(tr("Quitar plugin"))).clicked() {
                            if let Some(o) = self.omsi.as_deref() {
                                plugin_msg = Some(match plugin::uninstall(o) {
                                    Ok(()) => tr("Plugin quitado de la carpeta plugins de OMSI.").to_string(),
                                    Err(e) => e,
                                });
                            }
                        }
                    });
                });
                card(ui, tr("Copias de seguridad"), self.settings_tab == 3, &mut |ui| {
                    ui.label(RichText::new(tr("Se conservan las 3 últimas copias de cada archivo; al hacer una nueva se borra la más antigua.")).italics().size(12.0));
                    if ui.button(tr("Abrir carpeta de copias")).clicked() {
                        let dir = cfg::backup_dir();
                        let _ = std::fs::create_dir_all(&dir);
                        omsi::open_folder(&dir);
                    }
                });
                card(ui, tr("Actualizaciones"), self.settings_tab == 4, &mut |ui| {
                    ui.label(trf("Versión instalada: {}", &[update::LABEL]));
                    let state = self.update.lock().unwrap().clone();
                    ui.horizontal(|ui| {
                        let checking = state == update::State::Checking;
                        if ui.add_enabled(!checking, egui::Button::new(tr("Buscar actualizaciones"))).clicked() {
                            let c = ui.ctx().clone();
                            update::start(&self.update, move || c.request_repaint());
                        }
                        match &state {
                            update::State::Idle => {}
                            update::State::Checking => {
                                ui.spinner();
                                ui.label(tr("Buscando..."));
                            }
                            update::State::UpToDate => {
                                ui.label(tr("Tienes la última versión."));
                            }
                            update::State::Newer { tag, url } => {
                                ui.label(RichText::new(trf("Hay una versión nueva: {}", &[tag])).strong());
                                if ui.button(tr("Abrir la página de descarga")).clicked() {
                                    open_url = Some(url.clone());
                                }
                            }
                            update::State::Failed(_) => {
                                ui.label(tr("No se pudo comprobar (¿sin conexión o aún sin versiones publicadas?)."));
                            }
                        }
                    });
                    ui.label(RichText::new(trf("Las versiones se publican en github.com/{}. El launcher solo avisa: no descarga ni cambia nada por sí solo.", &[update::REPO])).italics().size(12.0));
                });
            });
        });
        self.settings_open = open;
        if let Some(u) = open_url {
            omsi::open_folder(std::path::Path::new(&u));
        }
        if let Some(m) = lang_msg {
            self.set_status(&m);
        }
        if let Some(m) = plugin_msg {
            log::line(&m);
            self.set_status(&m);
        }
        if changed {
            if let Err(e) = self.settings.save() {
                self.set_status(&e);
            }
        }
    }

    fn driver_window(&mut self, ctx: &egui::Context) {
        let Some(mut edit) = self.driver_edit.take() else { return };
        let open;
        let mut done = false;
        let mut save = false;
        let title = if edit.existing.is_some() { tr("Editar conductor") } else { tr("Crear nuevo conductor") };
        open = free::window(ctx, "driver", &title, egui::vec2(420.0, 300.0), |ui| {
            egui::Grid::new("driver_grid").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
                ui.label(tr("Nombre:"));
                ui.text_edit_singleline(&mut edit.name);
                ui.end_row();
                ui.label(tr("Sexo:"));
                ui.horizontal(|ui| {
                    ui.radio_value(&mut edit.female, false, tr("Hombre"));
                    ui.radio_value(&mut edit.female, true, tr("Mujer"));
                });
                ui.end_row();
                ui.label(tr("Nacimiento:"));
                ui.horizontal(|ui| {
                    ui.add(egui::DragValue::new(&mut edit.day).range(1..=31));
                    ui.add(egui::DragValue::new(&mut edit.month).range(1..=12));
                    ui.add(egui::DragValue::new(&mut edit.year).range(1900..=2100));
                    ui.label(tr("(día / mes / año)"));
                });
                ui.end_row();
            });
            if !edit.error.is_empty() {
                ui.colored_label(Color32::from_rgb(170, 0, 0), &edit.error);
            }
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if big_button(ui, tr("Guardar"), 120.0).clicked() {
                    save = true;
                }
                if big_button(ui, tr("Cancelar"), 120.0).clicked() {
                    done = true;
                }
            });
        });
        if save {
            let Some(dir) = self.omsi.clone() else { return };
            let res = match &edit.existing {
                Some(old) => cfg::update_driver(old, &edit.name, edit.female, &edit.birth()),
                None => cfg::create_driver(&dir, &edit.name, edit.female, &edit.birth()),
            };
            match res {
                Ok(()) => {
                    self.reload_drivers(Some(edit.name.trim()));
                    self.set_status(tr("Conductor guardado (copia de seguridad en %APPDATA%\\OMSI2-Launcher\\backups)."));
                    done = true;
                }
                Err(e) => edit.error = e,
            }
        }
        if open && !done {
            self.driver_edit = Some(edit);
        }
    }


    fn driver_section(&mut self, ui: &mut egui::Ui, running: bool) {
        let photo = self.drivers.get(self.driver).and_then(|d| {
            let p = d.path.with_extension("bmp");
            let p = if p.is_file() { p } else { self.omsi.as_ref()?.join("Drivers").join("nopic.bmp") };
            p.is_file().then(|| format!("file:///{}", p.display().to_string().replace('\\', "/")))
        });
        ui.horizontal_top(|ui| {
            if let Some(uri) = photo {
                ui.add(egui::Image::new(uri).fit_to_exact_size(egui::vec2(120.0, 160.0)));
            }
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    ui.label(tr("Conductor actual:"));
                    let names: Vec<String> = self.drivers.iter().map(|d| d.name.clone()).collect();
                    pick::pick(ui, "driver", 150.0, &names, &mut self.driver);
                });
                let enabled = !running && self.omsi.is_some();
                // like OMSI's start screen: the staff file first, then a new driver (editing and deleting are in the staff file)
                if ui.add_enabled(!self.drivers.is_empty(), egui::Button::new(tr("Abrir archivo de personal")).min_size(egui::vec2(300.0, 28.0))).clicked() {
                    self.staff_open = true;
                    self.staff_sel = self.driver;
                    self.staff_labels = Arc::new(self.omsi.as_deref().map(|d| olf::load(d, self.settings.lang)).unwrap_or_default());
                }
                if ui.add_enabled(enabled, egui::Button::new(tr("Crear nuevo conductor")).min_size(egui::vec2(300.0, 28.0))).clicked() {
                    self.driver_edit = Some(DriverEdit::new(None));
                }
                if running {
                    ui.label(RichText::new(tr("Cierra OMSI 2 para editar conductores.")).italics());
                }
            });
        });
    }

    /// OMSI's own two choices: "Cambiar fecha y hora" (opens the date and time window) and "Establecer hora de la situación", with the
    /// date and time that will be used shown underneath. Using the current time is an option of OMSI's Options window, not of this screen.
    fn time_section(&mut self, ui: &mut egui::Ui, running: bool) {
        if self.options.is_none() {
            return;
        }
        ui.label(RichText::new(tr("Fecha y hora")).strong());
        let change = ui.radio(self.time_mode == TimeMode::Custom, tr("Cambiar fecha y hora..."));
        if change.clicked() {
            // choosing it (or pressing it again) opens the window; Cancel leaves things as they were
            self.time_dialog = Some(TimeDialog { time: (self.clock.0, self.clock.1, self.sec), date: self.date, date_touched: self.date_touched });
        }
        if ui.radio(self.time_mode == TimeMode::Situation, tr("Establecer hora de la situación")).clicked() {
            self.time_mode = TimeMode::Situation;
        }
        let (date, time) = self.shown_date_time();
        ui.horizontal(|ui| {
            ui.label(format!("{} {}", tr("Fecha:"), date.map(|(y, m, d)| format!("{d:02}/{m:02}/{y}")).unwrap_or_default()));
            ui.add_space(16.0);
            ui.label(format!("{} {}", tr("Hora:"), time.map(|(h, m, s)| format!("{h:02}:{m:02}:{s:02}")).unwrap_or_default()));
        });
        if running && self.time_mode != TimeMode::Custom {
            ui.label(RichText::new(tr("OMSI está abierto: el cambio se aplicará la próxima vez que lo abras.")).italics());
        }
    }

    /// The date and time the start screen shows: the chosen ones, or those of the situation (the selected saved one, else the map's last).
    fn shown_date_time(&self) -> (Option<chrono::Date>, Option<(u32, u32, u32)>) {
        let situation = self.selected_situation().filter(|_| self.mode == LoadMode::SavedSituation);
        let saved = match (&self.omsi, self.maps.get(self.map)) {
            (Some(o), Some(m)) => chrono::situation_datetime(&o.join("maps").join(&m.folder).join("laststn.osn")),
            _ => None,
        };
        let (sit_date, sit_time) = match situation {
            Some(s) => (s.date, s.time),
            None if self.mode == LoadMode::SavedSituation => (None, None),
            None => (saved.map(|x| x.0), saved.map(|x| x.1)),
        };
        match self.time_mode {
            TimeMode::Custom => (
                if self.date_touched { Some((self.date.2 as i64, self.date.1, self.date.0)) } else { sit_date },
                Some((self.clock.0, self.clock.1, self.sec)),
            ),
            TimeMode::Situation => (sit_date, sit_time),
        }
    }


    fn weather_section(&mut self, ui: &mut egui::Ui) {
        ui.label(tr("Información sobre el tiempo seleccionado:"));
        let text = self.chrono_text();
        // the box is always there, empty when the map has no time information for the date (as in OMSI)
        egui::Frame::new().fill(pal().field).stroke(egui::Stroke::new(1.0, pal().dark)).inner_margin(6).show(ui, |ui| {
            egui::ScrollArea::vertical().id_salt("chrono").min_scrolled_height(110.0).max_height(110.0).auto_shrink([false, false]).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.add(egui::Label::new(RichText::new(text).strong().size(12.0)).wrap());
            });
        });
    }

    /// OMSI's "time information": the map's timetable/tariff periods that cover the selected date (cached until something changes).
    fn chrono_text(&mut self) -> String {
        let (Some(omsi), Some(map)) = (self.omsi.as_ref(), self.maps.get(self.map)) else { return String::new() };
        let map_dir = omsi.join("maps").join(&map.folder);
        let situation = self.selected_situation().filter(|_| self.mode == LoadMode::SavedSituation);
        let saved = || chrono::situation_date(&map_dir.join("laststn.osn"));
        let date = match (self.time_mode, self.date_touched, situation) {
            (TimeMode::Custom, true, _) => Some((self.date.2 as i64, self.date.1, self.date.0)),
            (_, _, Some(s)) => s.date.or_else(saved),
            _ if self.mode == LoadMode::SavedSituation => None,
            _ => saved(),
        };
        let Some(date) = date else { return String::new() };
        let lang = olf::omsi_code(self.settings.lang);
        let key = (map.folder.clone(), date, lang);
        if let Some((k, text)) = &self.chrono_cache {
            if *k == key {
                return text.clone();
            }
        }
        let text = chrono::info(&map_dir, date, lang);
        self.chrono_cache = Some((key, text.clone()));
        text
    }
}

impl App {
    /// Shown from the moment ¡Iniciar! is pressed until OMSI has started and begins loading the map.
    /// Shown instead of the menu when OMSI 2 is not found: asks where it is installed, then restarts the launcher to load it.
    fn locate_omsi(&mut self, ui: &mut egui::Ui) {
        egui::CentralPanel::default().frame(egui::Frame::new().fill(pal().bg)).show(ui, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space((ui.available_height() * 0.25).max(10.0));
                let uri = format!("bytes://omsi-logo-{:?}.svg", self.settings.theme);
                ui.add(egui::Image::from_bytes(uri, logo::svg(&pal()).into_bytes()).fit_to_exact_size(egui::vec2(380.0, 100.0)));
                ui.add_space(24.0);
                ui.label(RichText::new(tr("No se encontró OMSI 2")).size(26.0).strong());
                ui.add_space(8.0);
                ui.label(tr("Indica la carpeta donde tienes OMSI 2 instalado (la que contiene Omsi.exe)."));
                ui.add_space(12.0);
                ui.add(egui::TextEdit::singleline(&mut self.locate_dir).desired_width(420.0));
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.add_space(((ui.available_width() - 190.0) / 2.0).max(0.0));
                    if ui.button(tr("Examinar...")).clicked() {
                        if let Some(p) = omsi::browse_folder(tr("Elige la carpeta de OMSI 2")) {
                            self.locate_dir = p.display().to_string();
                            self.locate_error = false;
                        }
                    }
                    if ui.button(tr("Aceptar")).clicked() {
                        let dir = PathBuf::from(self.locate_dir.trim().trim_matches('"'));
                        if dir.join("Omsi.exe").is_file() {
                            self.settings.omsi_dir = Some(dir);
                            let _ = self.settings.save();
                            if let Ok(exe) = std::env::current_exe() {
                                let _ = std::process::Command::new(exe).spawn();
                            }
                            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                        } else {
                            self.locate_error = true;
                        }
                    }
                    if ui.button(tr("Cerrar")).clicked() {
                        ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });
                if self.locate_error {
                    ui.add_space(8.0);
                    ui.colored_label(Color32::from_rgb(190, 0, 0), tr("Esa carpeta no contiene Omsi.exe."));
                }
            });
        });
    }

    fn loading_screen(&mut self, ui: &mut egui::Ui) {
        if self.loading_tutorial.is_some() && self.tut_entries.is_empty() {
            self.load_tutorials();
        }
        let map = self.maps.get(self.map).cloned();
        let elapsed = self.load_started.map(|t| t.elapsed().as_secs()).unwrap_or(0);
        let status = self.status.lock().unwrap().clone();
        egui::CentralPanel::default().frame(egui::Frame::new().fill(pal().bg)).show(ui, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space((ui.available_height() * 0.12).max(10.0));
                let uri = format!("bytes://omsi-logo-{:?}.svg", self.settings.theme);
                ui.add(egui::Image::from_bytes(uri, logo::svg(&pal()).into_bytes()).fit_to_exact_size(egui::vec2(380.0, 100.0)));
                ui.add_space(18.0);
                ui.label(RichText::new(tr("Cargando OMSI 2...")).size(30.0).strong());
                ui.add_space(6.0);
                if let Some(n) = self.loading_tutorial {
                    // a tutorial is loading, not the selected map: show the tutorial's name and picture
                    let entry = self.tut_entries.get(n as usize).filter(|e| !e.title.is_empty());
                    let name = entry.map(|e| e.title.clone()).unwrap_or_else(|| [tr("Tutorial I"), tr("Tutorial II"), tr("Tutorial III"), tr("Tutorial IV")][(n as usize).clamp(1, 4) - 1].to_string());
                    ui.label(RichText::new(name).size(18.0));
                    if let Some(p) = entry.and_then(|e| e.picture.as_ref()) {
                        ui.add_space(10.0);
                        let uri = format!("file:///{}", p.display().to_string().replace('\\', "/"));
                        ui.add(egui::Image::new(uri).max_height(240.0).maintain_aspect_ratio(true));
                    }
                } else if let Some(m) = &map {
                    ui.label(RichText::new(format!("{} {}", tr("Mapa:"), m.name)).size(18.0));
                    if let Some(p) = &m.picture {
                        ui.add_space(10.0);
                        let uri = format!("file:///{}", p.display().to_string().replace('\\', "/"));
                        ui.add(egui::Image::new(uri).max_height(240.0).maintain_aspect_ratio(true));
                    }
                }
                ui.add_space(14.0);
                ui.add(egui::Spinner::new().size(36.0));
                ui.add_space(8.0);
                ui.label(RichText::new(status).size(16.0));
                ui.add_space(4.0);
                ui.label(RichText::new(trf("Tiempo transcurrido: {}", &[&format!("{}:{:02}", elapsed / 60, elapsed % 60)])).italics());
                ui.label(RichText::new(tr("Esto puede tardar un par de minutos.")).italics().size(12.0));
                ui.add_space(14.0);
                // if OMSI shows a message while loading, it must be possible to get at it
                if ui.button(tr("Ocultar el launcher")).clicked() {
                    omsi::minimize_own_window();
                }
            });
        });
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if self.close_requested.load(Ordering::Relaxed) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        if self.running.0.elapsed() > Duration::from_secs(2) {
            self.running = (Instant::now(), omsi::omsi_running());
        }
        ctx.request_repaint_after(Duration::from_secs(2));
        let busy = *self.busy.lock().unwrap();
        let running = self.running.1;
        if self.plugin_checked.elapsed() > Duration::from_millis(800) {
            self.plugin_checked = Instant::now();
            self.plugin_ok = self.omsi.as_deref().is_some_and(|o| plugin::installed(o) && plugin::up_to_date(o));
        }
        ctx.request_repaint_after(Duration::from_secs(1));
        let can_start = !busy && self.omsi.is_some() && !self.maps.is_empty() && self.plugin_ok;
        let can_go = can_start && (self.mode != LoadMode::SavedSituation || self.selected_situation().is_some());

        if ctx.input(|i| i.key_pressed(egui::Key::F11)) {
            self.settings.fullscreen = !self.settings.fullscreen;
            set_fullscreen(&ctx, self.settings.fullscreen);
            let _ = self.settings.save();
        }
        if self.restore_view.swap(false, Ordering::Relaxed) {
            if self.settings.fullscreen {
                ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(true));
            }
            ctx.send_viewport_cmd(egui::ViewportCommand::WindowLevel(egui::WindowLevel::Normal));
        }
        // OMSI's start screen would hide the loading screen: put the launcher back on top of it.
        if self.raise_view.swap(false, Ordering::Relaxed) {
            if self.settings.always_on_top {
                ctx.send_viewport_cmd(egui::ViewportCommand::WindowLevel(egui::WindowLevel::AlwaysOnTop));
            }
            ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
        }
        if self.minimize_requested.swap(false, Ordering::Relaxed) {
            ctx.send_viewport_cmd(egui::ViewportCommand::WindowLevel(egui::WindowLevel::Normal));
            omsi::minimize_own_window();
        }
        // Full screen: the interface sits in the middle of a black screen, like OMSI's own start screen.
        let mut centred;
        let host: &mut egui::Ui = if self.settings.fullscreen {
            let screen = ui.max_rect();
            ui.painter().rect_filled(screen, 0.0, Color32::BLACK);
            let size = egui::vec2(1080.0_f32.min(screen.width()), 780.0_f32.min(screen.height()));
            centred = ui.new_child(egui::UiBuilder::new().max_rect(egui::Rect::from_center_size(screen.center(), size)));
            &mut centred
        } else {
            ui
        };
        if !self.entered_full && self.born.elapsed() >= SPLASH {
            self.entered_full = true;
            if self.settings.fullscreen {
                ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(true));
            }
        }
        // Opening splash: hides the window being created and the images loading, and makes the first black instant look intended.
        if self.born.elapsed() < SPLASH && std::env::var_os("OMSI2_OPEN").is_none() {
            ctx.request_repaint_after(Duration::from_millis(30));
            egui::CentralPanel::default().frame(egui::Frame::new().fill(pal().bg)).show(host, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space((ui.available_height() * 0.30).max(10.0));
                    let uri = format!("bytes://omsi-logo-{:?}.svg", self.settings.theme);
                    ui.add(egui::Image::from_bytes(uri, logo::svg(&pal()).into_bytes()).fit_to_exact_size(egui::vec2(380.0, 100.0)));
                    ui.add_space(24.0);
                    ui.add(egui::Spinner::new().size(32.0));
                    ui.add_space(10.0);
                    ui.label(RichText::new(tr("Cargando...")).size(18.0));
                });
            });
            return;
        }
        if self.omsi.is_none() {
            self.locate_omsi(host);
            return;
        }
        // While OMSI starts, a loading screen replaces the menu; it goes away when the map starts loading (launch() has finished).
        if busy {
            ctx.request_repaint_after(Duration::from_millis(250));
            self.loading_screen(host);
            return;
        }

        egui::Panel::bottom("bar").frame(egui::Frame::new().fill(pal().bg)).show(host, |ui| {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.add_space(16.0);
                if busy {
                    ui.spinner();
                }
                if !self.plugin_ok && self.omsi.is_some() {
                    ui.label(RichText::new(tr("Instala el plugin en Ajustes del launcher para poder iniciar OMSI 2.")).color(Color32::from_rgb(190, 0, 0)).strong());
                } else {
                    ui.label(self.status.lock().unwrap().clone());
                }
            });
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                let w = (ui.available_width() - 3.0 * ui.spacing().item_spacing.x) / 4.0;
                let go = egui::Button::new(RichText::new(tr("¡Iniciar!")).size(22.0).color(Color32::WHITE))
                    .fill(pal().dark)
                    .corner_radius(0.0)
                    .min_size(egui::vec2(w, 40.0));
                let go_resp = ui.add_enabled(can_go, go);
                paint_icon(ui, &go_resp, icons::Icon::Play, Color32::WHITE);
                if go_resp.clicked() {
                    let mode = self.mode;
                    self.start(&ctx, mode);
                }
                if big_icon_button(ui, tr("Tutoriales"), w, icons::Icon::Book).clicked() {
                    self.tut_open = !self.tut_open;
                    if self.tut_open {
                        self.load_tutorials();
                    }
                }
                if big_icon_button(ui, tr("Opciones"), w, icons::Icon::Sliders).clicked() {
                    self.options_ui.open = !self.options_ui.open;
                }
                if big_icon_button(ui, tr("Cerrar"), w, icons::Icon::Close).clicked() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            });
        });

        egui::CentralPanel::default().frame(egui::Frame::new().fill(pal().bg)).show(host, |ui| {
            let full = ui.max_rect();
            ui.columns(2, |cols| {
                // ------------------------------------------------------------------ left: driver, date/time, weather
                let ui = &mut cols[0];
                section(ui, |ui| {
                    ui.horizontal(|ui| {
                        let uri = format!("bytes://omsi-logo-{:?}.svg", self.settings.theme);
                        ui.add(egui::Image::from_bytes(uri, logo::svg(&pal()).into_bytes()).fit_to_exact_size(egui::vec2(250.0, 66.0)));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.vertical(|ui| {
                                ui.label(RichText::new(format!("Launcher\nRelease {}", self.version)).italics());
                                if ui.small_button(tr("Ajustes del launcher")).clicked() {
                                    self.settings_open = !self.settings_open;
                                }
                            });
                        });
                    });
                });
                section(ui, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.hyperlink_to(
                            RichText::new(tr("¿Soporte? ¿Addons? ¿Info? forum.omnibussimulator.de/forum/")).strong().underline(),
                            "https://forum.omnibussimulator.de/forum/",
                        );
                    });
                });
                section(ui, |ui| self.driver_section(ui, running));
                section(ui, |ui| self.time_section(ui, running));
                section(ui, |ui| self.weather_section(ui));

                // ------------------------------------------------------------------ right: map
                let ui = &mut cols[1];
                section(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(tr("Mapa actual:"));
                        let names: Vec<String> = self.maps.iter().map(|m| m.name.clone()).collect();
                        pick::pick(ui, "map", ui.available_width() - 8.0, &names, &mut self.map);
                    });
                    ui.add_space(6.0);
                    ui.radio_value(&mut self.mode, LoadMode::LastState, tr("Cargar último estado del mapa"));
                    ui.radio_value(&mut self.mode, LoadMode::NoBuses, tr("Cargar mapa sin autobuses"));
                    if self.mode == LoadMode::NoBuses {
                        // OMSI's "Establecer posición" window is answered here, before starting: the map's places to appear at
                        if self.spawn_map != Some(self.map) {
                            self.spawn_map = Some(self.map);
                            self.spawn_sel = 0;
                            self.spawn_points = match (&self.omsi, self.maps.get(self.map)) {
                                (Some(o), Some(m)) => omsi::entry_points(o, m),
                                _ => vec![],
                            };
                        }
                        if !self.spawn_points.is_empty() {
                            ui.horizontal(|ui| {
                                ui.add_space(20.0);
                                ui.label(tr("Aparecer en:"));
                                let current = self.spawn_points.get(self.spawn_sel).cloned().unwrap_or_default();
                                egui::ComboBox::from_id_salt("spawn_point").selected_text(current).width(ui.available_width() - 8.0).show_ui(ui, |ui| {
                                    for (i, p) in self.spawn_points.iter().enumerate() {
                                        ui.selectable_value(&mut self.spawn_sel, i, p);
                                    }
                                });
                            });
                        }
                    }
                    ui.radio_value(&mut self.mode, LoadMode::SavedSituation, tr("Cargar situación guardada"));
                    if self.mode == LoadMode::SavedSituation {
                        // OMSI shows the saved situations in place of the map picture.
                        ui.add_space(6.0);
                        ui.checkbox(&mut self.sit_only_map, tr("Solo las situaciones del mapa seleccionado"));
                        let folder = self.maps.get(self.map).map(|m| m.folder.to_lowercase()).unwrap_or_default();
                        let shown: Vec<usize> = (0..self.situations.len())
                            .filter(|&i| !self.sit_only_map || self.situations[i].map_folder.to_lowercase() == folder)
                            .collect();
                        egui::Frame::new().fill(pal().field).stroke(egui::Stroke::new(1.0, pal().dark)).inner_margin(4).show(ui, |ui| {
                            egui::ScrollArea::vertical().id_salt("situations").max_height(240.0).show(ui, |ui| {
                                ui.set_width(ui.available_width());
                                if shown.is_empty() {
                                    ui.label(RichText::new(tr("No hay situaciones guardadas para este mapa.")).italics());
                                }
                                for &i in &shown {
                                    let s = &self.situations[i];
                                    if ui.selectable_label(self.sit_sel == Some(i), &s.name).clicked() {
                                        self.sit_sel = Some(i);
                                        // a situation belongs to a map: show that map
                                        if let Some(m) = self.maps.iter().position(|m| m.folder.eq_ignore_ascii_case(&s.map_folder)) {
                                            self.map = m;
                                        }
                                    }
                                }
                            });
                        });
                    } else if let Some(p) = self.maps.get(self.map).and_then(|m| m.picture.as_ref()) {
                        ui.add_space(8.0);
                        let uri = format!("file:///{}", p.display().to_string().replace('\\', "/"));
                        ui.add(egui::Image::new(uri).max_width(ui.available_width()).max_height(260.0).maintain_aspect_ratio(true));
                    }
                });
                let (heading, body) = if self.mode == LoadMode::SavedSituation {
                    let text = match self.selected_situation() {
                        Some(s) => s.description.clone(),
                        None => tr("¡No hay situación seleccionada!").to_string(),
                    };
                    (tr("Información sobre la situación seleccionada:"), Some(text))
                } else {
                    (tr("Información sobre el mapa seleccionado:"), self.maps.get(self.map).map(|m| m.description.clone()))
                };
                if let Some(body) = body {
                    ui.horizontal(|ui| {
                        ui.add_space(8.0);
                        ui.label(heading);
                    });
                    egui::Frame::new().fill(pal().field).stroke(egui::Stroke::new(1.0, pal().dark)).inner_margin(6).outer_margin(egui::Margin::symmetric(16, 4)).show(ui, |ui| {
                        egui::ScrollArea::vertical().id_salt("desc").max_height((ui.available_height() - 24.0).max(60.0)).show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.add(egui::Label::new(RichText::new(body).strong()).wrap());
                        });
                    });
                }
            });
            // vertical rule between the two columns, as in OMSI's own screen
            let x = full.center().x;
            ui.painter().rect_filled(egui::Rect::from_min_max(egui::pos2(x - 3.0, full.top()), egui::pos2(x + 3.0, full.bottom())), 0.0, pal().line);
        });

        self.driver_window(&ctx);
        self.settings_window(&ctx);
        self.tutorials_window(&ctx, can_start);
        self.staff_window(&ctx, running);
        self.time_dialog_window(&ctx);
        let can_write = !running && self.omsi.is_some();
        if let Some(msg) = options_ui::show(&ctx, &mut self.options_ui, &mut self.options, can_write, self.omsi.as_deref()) {
            self.set_status(&msg);
        }
        if std::mem::take(&mut self.options_ui.preset_loaded) {
            self.sync_from_options();
        }
    }
}

/// How long the opening splash stays up.
const SPLASH: Duration = Duration::from_secs(3);

/// Switches between full screen and a normal window (leaving full screen gives the window a sensible size back).
fn set_fullscreen(ctx: &egui::Context, on: bool) {
    ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(on));
    if !on {
        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(1080.0, 780.0)));
    }
}

fn main() -> eframe::Result {
    // a crash leaves a line in launcher.log (a panic would otherwise close the window without a trace)
    std::panic::set_hook(Box::new(|info| log::line(&format!("PANIC: {info}"))));
    let _ = settings::Settings::load();
    let mut viewport = egui::ViewportBuilder::default()
        .with_title("OMSI 2 Launcher")
        .with_icon(egui::IconData { rgba: app_icon::rgba(64), width: 64, height: 64 });
    // Always opens as a normal, centred window (the splash is shown in it); full screen is entered after the splash.
    viewport = viewport.with_inner_size([1080.0, 780.0]).with_min_inner_size([900.0, 640.0]);
    let options = eframe::NativeOptions { viewport, centered: true, ..Default::default() };
    eframe::run_native("OMSI 2 Launcher", options, Box::new(|cc| Ok(Box::new(App::new(cc)))))
}


/// What is being edited in the "Seleccionar hora y fecha" window until the user presses Accept.
#[derive(Clone)]
struct TimeDialog {
    time: (u32, u32, u32),
    date: (u32, u32, u32),
    date_touched: bool,
}

/// The hours OMSI's own "Día / Tarde / Noche / Mañana" buttons set (measured from OMSI's date/time dialog).
const DAY_TIMES: [(&str, u32); 4] = [("Día", 12), ("Tarde", 17), ("Noche", 0), ("Mañana", 7)];

impl App {
    /// A window like OMSI's "Select time and date...": time with seconds, the four times of day, the date, its weekday and holidays.
    fn time_dialog_window(&mut self, ctx: &egui::Context) {
        let Some(mut dlg) = self.time_dialog.clone() else { return };
        let (mut accept, mut cancel) = (false, false);
        let open;
        let two = |v: f64, _: std::ops::RangeInclusive<usize>| format!("{:02}", v as u32);
        open = free::window(ctx, "time", tr("Seleccionar hora y fecha..."), egui::vec2(520.0, 420.0), |ui| {
            ui.set_width(430.0);
            ui.horizontal(|ui| {
                ui.label(tr("Hora:"));
                ui.add(egui::DragValue::new(&mut dlg.time.0).range(0..=23).custom_formatter(two));
                ui.label(":");
                ui.add(egui::DragValue::new(&mut dlg.time.1).range(0..=59).custom_formatter(two));
                ui.label(":");
                ui.add(egui::DragValue::new(&mut dlg.time.2).range(0..=59).custom_formatter(two));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button(tr("Hora actual")).clicked() {
                        let (_, _, _, h, m, s) = omsi::local_now();
                        dlg.time = (h, m, s);
                    }
                });
            });
            ui.horizontal(|ui| {
                for (name, hour) in DAY_TIMES {
                    if ui.button(tr(name)).clicked() {
                        dlg.time = (hour, 0, 0);
                    }
                }
            });
            ui.separator();
            ui.horizontal(|ui| {
                ui.label(tr("Fecha:"));
                let before = dlg.date;
                ui.add(egui::DragValue::new(&mut dlg.date.0).range(1..=31).custom_formatter(|v, _| format!("{}", v as u32)));
                ui.label("/");
                ui.add(egui::DragValue::new(&mut dlg.date.1).range(1..=12).custom_formatter(|v, _| format!("{}", v as u32)));
                ui.label("/");
                ui.add(egui::DragValue::new(&mut dlg.date.2).range(1900..=2100));
                if dlg.date != before {
                    dlg.date_touched = true;
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button(tr("Año actual")).clicked() {
                        dlg.date.2 = omsi::local_now().0;
                        dlg.date_touched = true;
                    }
                    if ui.button(tr("Fecha actual")).clicked() {
                        let (y, m, d, ..) = omsi::local_now();
                        dlg.date = (d, m, y);
                        dlg.date_touched = true;
                    }
                });
            });
            // a date that does not exist (31 February) is brought back to the last day of that month
            dlg.date.0 = dlg.date.0.min(days_in_month(dlg.date.2, dlg.date.1));
            let day = (dlg.date.2 as i64, dlg.date.1, dlg.date.0);
            let weekdays = ["Lunes", "Martes", "Miércoles", "Jueves", "Viernes", "Sábado", "Domingo"];
            ui.horizontal(|ui| {
                ui.label(tr("Día de la semana:"));
                ui.label(RichText::new(tr(weekdays[chrono::weekday(day)])).strong());
            });
            let holidays = match (&self.omsi, self.maps.get(self.map)) {
                (Some(o), Some(m)) => chrono::holidays(&o.join("maps").join(&m.folder), olf::omsi_code(self.settings.lang), day),
                _ => vec![],
            };
            ui.horizontal(|ui| {
                ui.label(tr("Festivos:"));
                ui.label(RichText::new(holidays.join(", ")).strong());
            });
            if !dlg.date_touched {
                ui.label(RichText::new(tr("Si no cambias la fecha, OMSI usa la de la situación del mapa.")).italics().size(12.0));
            }
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if big_button(ui, tr("Aceptar"), 120.0).clicked() {
                    accept = true;
                }
                if big_button(ui, tr("Cancelar"), 120.0).clicked() {
                    cancel = true;
                }
            });
        });
        if !open {
            cancel = true; // the cross of the window is the same as Cancel
        }
        if accept {
            self.clock = (dlg.time.0, dlg.time.1);
            self.sec = dlg.time.2;
            self.date = dlg.date;
            self.date_touched = dlg.date_touched;
            self.time_mode = TimeMode::Custom;
            self.time_dialog = None;
        } else if cancel {
            self.time_dialog = None;
        } else {
            self.time_dialog = Some(dlg);
        }
    }
}

fn days_in_month(year: u32, month: u32) -> u32 {
    match month {
        2 if (year % 4 == 0 && year % 100 != 0) || year % 400 == 0 => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

impl App {
    /// The start screen follows the options: the map and driver of `last_map` / `last_driver`, and whether the date and time are the
    /// current ones. Used after loading a preset, which (like in OMSI) can change all of them.
    fn sync_from_options(&mut self) {
        let Some(o) = self.options.as_ref() else { return };
        let last = |key: &str| o.get(key, 0).unwrap_or_default().to_lowercase();
        let (last_map, last_driver) = (last("last_map"), last("last_driver"));
        if let Some(i) = self.maps.iter().position(|m| last_map.contains(&format!("\\{}\\", m.folder.to_lowercase()))) {
            self.map = i;
        }
        if let Some(i) = self.drivers.iter().position(|d| last_driver.ends_with(&d.path.file_name().unwrap_or_default().to_string_lossy().to_lowercase())) {
            self.driver = i;
        }
    }
}
