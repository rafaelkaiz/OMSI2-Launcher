//! The "Opciones" window: edits the `options.cfg` keys OMSI's own options dialog edits, with the same tabs, labels and order
//! (Teclado, Controlador de juego and Addons are left to OMSI). Anything else in the file is left as it is.

use crate::i18n::{tr, trf};
use crate::cfg::{self, Options};
use crate::olf;
use crate::options_layout as layout;
use crate::presets;
use crate::theme::pal;
use eframe::egui::{self, Color32, RichText};
use std::path::{Path, PathBuf};

#[derive(Default)]
pub struct State {
    pub open: bool,
    /// Unsaved changes in the in-memory options (from this window or from the start screen).
    pub dirty: bool,
    tab: usize,
    was_open: bool,
    presets: Vec<PathBuf>,
    preset: usize,
    new_preset: String,
    omsi_labels: std::sync::Arc<std::collections::HashMap<String, String>>,
    /// The options as they were when the window opened, restored by Cancelar.
    snapshot: Option<Options>,
    snapshot_dirty: bool,
}

const TABS: [&str; 6] = ["General", "Avanzado", "Gráficos", "Gráficos (avanzado)", "Sonido", "Tráfico IA"];

/// Every on/off key the dialog edits (what a preset has to switch off when it does not mention them).
const MANAGED_FLAGS: &[&str] = &[
    "driverview_moving",
    "driverview_smooth",
    "altView",
    "autoCenter",
    "no_automaticClutch",
    "redSteerSpd",
    "no_collision",
    "no_schedAnaPopUp",
    "see_own_driver",
    "no_ticketinfo_visible",
    "noAutoSave",
    "showerrormessages",
    "no_collision_terrain",
    "no_collision_pedastrians",
    "no_collision_vehToVeh",
    "no_multithreading_calculate",
    "no_multithreading_texload",
    "loadAllTiles",
    "sunglow",
    "no_stencilbuffer",
    "no_rain_refl",
    "no_humans_on_rain_refl",
    "texture_uselow",
    "texmax256",
    "no_nightmap",
    "no_lightmap",
    "no_lightmap_terr",
    "no_reflmap",
    "no_bumpmap",
    "sound_ai",
    "sound_scenery",
    "sound_noreverb",
    "useLowAIList",
];

fn managed_flags() -> Vec<&'static str> {
    MANAGED_FLAGS.to_vec()
}

impl State {
    /// Selects a tab (used by the screenshot helper).
    pub fn set_tab(&mut self, tab: usize) {
        self.tab = tab.min(TABS.len() - 1);
    }
}

/// Returns a status message when something was saved or restored.
pub fn show(ctx: &egui::Context, st: &mut State, options: &mut Option<Options>, can_write: bool, omsi: Option<&Path>) -> Option<String> {
    // Re-read the file every time the window opens (OMSI rewrites it when it exits), unless there are unsaved edits.
    if st.open && !st.was_open {
        st.presets = omsi.map(presets::list).unwrap_or_default();
        st.omsi_labels = std::sync::Arc::new(omsi.map(|d| olf::load(d, crate::i18n::lang())).unwrap_or_default());
        st.preset = st.preset.min(st.presets.len().saturating_sub(1));
    }
    if st.open && !st.was_open && !st.dirty {
        if let Some(o) = omsi.and_then(Options::load) {
            *options = Some(o);
        }
    }
    if st.open && !st.was_open {
        st.snapshot = options.clone();
        st.snapshot_dirty = st.dirty;
    }
    st.was_open = st.open;
    if !st.open {
        return None;
    }

    let mut msg = None;
    let mut open = st.open;
    let (mut accept, mut cancel) = (false, false);
    egui::Window::new(tr("Opciones")).id(egui::Id::unique("win-options")).pivot(egui::Align2::CENTER_CENTER).default_pos(ctx.content_rect().center()).open(&mut open).collapsible(false).resizable(false).show(ctx, |ui| {
        let Some(options) = options.as_mut() else {
            ui.label(tr("No se pudo leer options.cfg."));
            return;
        };
        ui.horizontal_wrapped(|ui| {
            for (i, t) in TABS.iter().enumerate() {
                ui.selectable_value(&mut st.tab, i, tr(t));
            }
        });
        ui.separator();
        if !can_write {
            ui.label(RichText::new(tr("OMSI 2 está abierto: ciérralo para poder guardar cambios.")).italics());
        }
        // the page of the dialog: every control at the position it has in OMSI's own window
        let (page, _) = ui.allocate_exact_size(egui::vec2(layout::PAGE.0, layout::PAGE.1), egui::Sense::hover());
        let labels = st.omsi_labels.clone();
        let cv = layout::Canvas { origin: page.min, omsi: &labels };
        if st.tab == 0 {
            // OMSI's "Cargar/Guardar" box: presets, loaded and saved here
            cv.group(ui, 17.0, 50.0, 557.0, 97.0, &cv.t("DG_Options_GroupBox10", "Cargar/Guardar"));
            cv.label(ui, 29.0, 81.0, 230.0, &cv.t("DG_Options_Label73", "Opciones de carga"));
            let names: Vec<String> = st.presets.iter().map(|p| p.file_stem().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()).collect();
            cv.put(ui, cv.rect(262.0, 74.0, 184.0, 22.0), |ui: &mut egui::Ui| {
                crate::pick::pick(ui, "preset", 170.0, &names, &mut st.preset);
                ui.response()
            });
            if cv.put(ui, cv.rect(455.0, 73.0, 94.0, 24.0), egui::Button::new(tr("Cargar")).min_size(egui::vec2(94.0, 24.0))).clicked() && !names.is_empty() {
                if let Some(preset) = st.presets.get(st.preset).and_then(|p| presets::read(p)) {
                    presets::apply(options, &preset, &managed_flags());
                    st.dirty = true;
                    msg = Some(trf("Preset \"{}\" cargado (sin guardar: pulsa Guardar en Opciones o ¡Iniciar!).", &[&preset.name]));
                }
            }
            cv.label(ui, 29.0, 115.0, 230.0, &cv.t("DG_Options_Label74", "Guardar opciones actuales"));
            cv.put(ui, cv.rect(262.0, 107.0, 184.0, 22.0), egui::TextEdit::singleline(&mut st.new_preset).desired_width(184.0).hint_text(tr("Nombre del preset")));
            if cv.put(ui, cv.rect(455.0, 106.0, 94.0, 24.0), egui::Button::new(cv.t("DG_Options_Button8", "¡Guardar!")).min_size(egui::vec2(94.0, 24.0))).clicked() {
                if let Some(dir) = omsi {
                    match presets::save_new(dir, &st.new_preset, options) {
                        Ok(path) => {
                            st.presets = presets::list(dir);
                            st.preset = st.presets.iter().position(|p| *p == path).unwrap_or(0);
                            msg = Some(trf("Preset \"{}\" guardado en option_presets.", &[st.new_preset.trim()]));
                            st.new_preset.clear();
                        }
                        Err(e) => msg = Some(e),
                    }
                }
            }
        }
        st.dirty |= layout::draw(&cv, ui, options, st.tab);
        // footer buttons at fixed places under the page, like OMSI's dialog
        let fy = layout::PAGE.1 + 6.0;
        let accept_btn = egui::Button::new(RichText::new(tr("Aceptar")).color(Color32::WHITE)).fill(pal().dark);
        if cv.put(ui, cv.rect(11.0, fy, 78.0, 26.0), accept_btn).clicked() {
            if can_write && st.dirty {
                match options.save() {
                    Ok(()) => {
                        st.dirty = false;
                        msg = Some(tr("Opciones guardadas (copia de seguridad en %APPDATA%\\OMSI2-Launcher\\backups).").to_string());
                    }
                    Err(e) => msg = Some(e),
                }
            }
            accept = true;
        }
        if cv.put(ui, cv.rect(95.0, fy, 76.0, 26.0), egui::Button::new(tr("Cancelar"))).clicked() {
            cancel = true;
        }
        let backup = cfg::latest_backup("options.cfg");
        let restore = ui
            .add_enabled_ui(can_write && backup.is_some(), |ui| cv.put(ui, cv.rect(177.0, fy, 170.0, 26.0), egui::Button::new(tr("Restaurar última copia"))))
            .inner
            .clicked();
        if restore {
            if let (Some(b), Some(dir)) = (backup, omsi) {
                match cfg::restore(&b, &dir.join("options.cfg")) {
                    Ok(()) => {
                        if let Some(o) = Options::load(dir) {
                            *options = o;
                        }
                        st.dirty = false;
                        msg = Some(tr("options.cfg restaurado desde la última copia.").into());
                    }
                    Err(e) => msg = Some(e),
                }
            }
        }
        if st.dirty {
            cv.note(ui, 355.0, fy + 4.0, 245.0, 20.0, tr("Hay cambios sin guardar (se guardan al pulsar ¡Iniciar!)."));
        }
    });
    // closing with the X is the same as Cancelar: drop what was changed while the window was open
    if !open {
        cancel = true;
    }
    if cancel {
        if let Some(s) = st.snapshot.take() {
            *options = Some(s);
            st.dirty = st.snapshot_dirty;
        }
    }
    st.open = !(accept || cancel);
    msg
}

