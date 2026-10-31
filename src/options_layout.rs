//! The tabs of the options window laid out like OMSI 2's own "Opciones" dialog: every control at the position it has there
//! (coordinates measured on OMSI's dialog, origin = top-left of the tab's page). Texts come from OMSI's own language files when the
//! launcher language is one OMSI has; otherwise from the launcher's translations.

use crate::cfg::Options;
use crate::i18n::{self, tr, Lang};
use crate::theme::pal;
use eframe::egui::{self, vec2, Color32, Rect, RichText, Ui};
use std::collections::HashMap;

pub struct Canvas<'a> {
    pub origin: egui::Pos2,
    pub omsi: &'a HashMap<String, String>,
}

type Choices = &'static [(&'static str, &'static str)];

impl Canvas<'_> {
    pub fn rect(&self, x: f32, y: f32, w: f32, h: f32) -> Rect {
        Rect::from_min_size(self.origin + vec2(x, y), vec2(w, h))
    }

    /// Places a widget in `rect`, left-aligned and vertically centred, in a text size close to the Windows dialog's.
    pub fn put(&self, ui: &mut Ui, rect: Rect, widget: impl egui::Widget) -> egui::Response {
        let builder = egui::UiBuilder::new().max_rect(rect).layout(egui::Layout::left_to_right(egui::Align::Center));
        ui.scope_builder(builder, |ui| {
            ui.style_mut().text_styles.iter_mut().for_each(|(_, f)| f.size *= 0.9);
            ui.spacing_mut().slider_width = rect.width();
            ui.set_clip_rect(rect.intersect(ui.clip_rect())); // like the Windows dialog, text that does not fit is cut
            ui.add(widget)
        })
        .inner
    }

    /// OMSI's own text for `key` when the launcher language is one OMSI has; the launcher's translation of `fallback` otherwise.
    pub fn t(&self, key: &str, fallback: &str) -> String {
        let has_omsi = matches!(i18n::lang(), Lang::Es | Lang::En | Lang::De | Lang::Pl | Lang::Fr);
        let own = !fallback.is_empty() && i18n::has(fallback);
        match self.omsi.get(key).filter(|s| !s.is_empty()) {
            // OMSI's own wording; for Czech and Portuguese (which OMSI does not have) only when the launcher has no translation
            Some(s) if has_omsi || !own => s.replace('@', "\n"),
            _ => tr(fallback).to_string(),
        }
    }

    pub fn label(&self, ui: &mut Ui, x: f32, y: f32, w: f32, text: &str) {
        self.put(ui, self.rect(x, y - 10.0, w, 20.0), egui::Label::new(text).wrap());
    }

    /// A line of text (wrapping inside `w`), `h` tall.
    pub fn note(&self, ui: &mut Ui, x: f32, y: f32, w: f32, h: f32, text: &str) {
        self.put(ui, self.rect(x, y, w, h), egui::Label::new(text).wrap());
    }

    pub fn value(&self, ui: &mut Ui, x: f32, y: f32, text: &str) {
        self.put(ui, self.rect(x, y - 9.0, 90.0, 18.0), egui::Label::new(RichText::new(text).strong()));
    }

    /// A group box: a frame with its title sitting on the top border.
    pub fn group(&self, ui: &mut Ui, x: f32, y: f32, w: f32, h: f32, title: &str) {
        let r = self.rect(x, y, w, h);
        let p = ui.painter();
        p.rect_stroke(r, 2.0, egui::Stroke::new(1.0, pal().border), egui::StrokeKind::Inside);
        if !title.is_empty() {
            let galley = p.layout_no_wrap(title.to_string(), egui::FontId::proportional(12.6), pal().text);
            let pos = r.left_top() + vec2(8.0, -9.0);
            let p = p.with_clip_rect(r.expand2(vec2(0.0, 12.0)));
            p.rect_filled(Rect::from_min_size(pos - vec2(3.0, 0.0), galley.size() + vec2(6.0, 0.0)), 0.0, pal().bg);
            p.galley(pos, galley, pal().text);
        }
    }

    // ---------------------------------------------------------------------------------------------------- checkboxes

    pub fn put_check(&self, ui: &mut Ui, x: f32, y: f32, w: f32, text: &str, on: &mut bool) -> bool {
        let h = if text.contains('\n') { 40.0 } else { 20.0 };
        self.put(ui, self.rect(x, y - 10.0, w, h), egui::Checkbox::new(on, text)).changed()
    }

    /// Key present = on (`inverted`: key present = off).
    pub fn flag(&self, ui: &mut Ui, o: &mut Options, x: f32, y: f32, w: f32, text: &str, key: &str, section: &str, inverted: bool) -> bool {
        let mut on = o.has(key) != inverted;
        let ch = self.put_check(ui, x, y, w, text, &mut on);
        if ch {
            o.set_flag(key, section, on != inverted);
        }
        ch
    }

    /// `on` / `off` value.
    pub fn on_off(&self, ui: &mut Ui, o: &mut Options, x: f32, y: f32, w: f32, text: &str, key: &str, section: &str) -> bool {
        let mut on = o.get(key, 0).is_none_or(|v| v.eq_ignore_ascii_case("on"));
        let ch = self.put_check(ui, x, y, w, text, &mut on);
        if ch {
            o.set(key, section, 0, if on { "on" } else { "off" });
        }
        ch
    }

    /// A 0/1 value line.
    pub fn int_bool(&self, ui: &mut Ui, o: &mut Options, x: f32, y: f32, w: f32, text: &str, key: &str, section: &str, idx: usize) -> bool {
        let mut on = o.get(key, idx).is_some_and(|s| s != "0");
        let ch = self.put_check(ui, x, y, w, text, &mut on);
        if ch {
            o.set(key, section, idx, if on { "1" } else { "0" });
        }
        ch
    }

    /// One checkbox that stands for several `no_...` keys (all present = on).
    pub fn flags_together(&self, ui: &mut Ui, o: &mut Options, x: f32, y: f32, w: f32, text: &str, keys: &[(&str, &str)]) -> bool {
        let mut on = keys.iter().any(|(k, _)| o.has(k));
        let ch = self.put_check(ui, x, y, w, text, &mut on);
        if ch {
            for (k, s) in keys {
                o.set_flag(k, s, on);
            }
        }
        ch
    }

    // ---------------------------------------------------------------------------------------------------- sliders and boxes

    /// An integer slider with its value written at `(vx, vy)` like OMSI does.
    #[allow(clippy::too_many_arguments)]
    pub fn slider_i(&self, ui: &mut Ui, o: &mut Options, x: f32, y: f32, w: f32, key: &str, section: &str, idx: usize, range: (i64, i64), shown: (f32, f32, &dyn Fn(i64) -> String)) -> bool {
        let mut v: i64 = o.get(key, idx).and_then(|s| s.parse().ok()).unwrap_or(range.0).clamp(range.0, range.1);
        let ch = self.put(ui, self.rect(x, y, w, 20.0), egui::Slider::new(&mut v, range.0..=range.1).show_value(false)).changed();
        if ch {
            o.set(key, section, idx, &v.to_string());
        }
        self.value(ui, shown.0, shown.1, &(shown.2)(v));
        ch
    }

    #[allow(clippy::too_many_arguments)]
    pub fn slider_f(&self, ui: &mut Ui, o: &mut Options, x: f32, y: f32, w: f32, key: &str, section: &str, range: (f64, f64), decimals: usize, shown: Option<(f32, f32, &dyn Fn(f64) -> String)>) -> bool {
        let mut v: f64 = o.get(key, 0).and_then(|s| s.parse().ok()).unwrap_or(range.0).clamp(range.0, range.1);
        let ch = self.put(ui, self.rect(x, y, w, 20.0), egui::Slider::new(&mut v, range.0..=range.1).show_value(false)).changed();
        if ch {
            o.set(key, section, 0, &format!("{v:.decimals$}"));
        }
        if let Some((vx, vy, f)) = shown {
            self.value(ui, vx, vy, &f(v));
        }
        ch
    }

    /// A number box (OMSI shows e.g. `20.000`).
    pub fn number(&self, ui: &mut Ui, o: &mut Options, x: f32, y: f32, w: f32, key: &str, section: &str, idx: usize, decimals: usize) -> bool {
        let mut v: f64 = o.get(key, idx).and_then(|s| s.parse().ok()).unwrap_or(0.0);
        let ch = self.put(ui, self.rect(x, y, w, 22.0), |ui: &mut Ui| ui.add_sized([w, 22.0], egui::DragValue::new(&mut v).speed(0.5).fixed_decimals(decimals))).changed();
        if ch {
            o.set(key, section, idx, &format!("{v:.decimals$}"));
        }
        ch
    }

    pub fn combo(&self, ui: &mut Ui, o: &mut Options, x: f32, y: f32, w: f32, id: &str, key: &str, section: &str, choices: Choices, translate: bool) -> bool {
        let cur = o.get(key, 0).unwrap_or_else(|| choices[0].0.to_string());
        let label = |l: &str| if translate { tr(l).to_string() } else { l.to_string() };
        let text = choices.iter().find(|(v, _)| v.eq_ignore_ascii_case(&cur)).map_or(cur.clone(), |c| label(c.1));
        let mut changed = false;
        self.put(ui, self.rect(x, y, w, 22.0), |ui: &mut Ui| {
            egui::ComboBox::from_id_salt(id).selected_text(text).width(w - 20.0).show_ui(ui, |ui| {
                for (v, l) in choices {
                    if ui.selectable_label(v.eq_ignore_ascii_case(&cur), label(l)).clicked() {
                        o.set(key, section, 0, v);
                        changed = true;
                    }
                }
            });
            ui.response()
        });
        changed
    }

    pub fn text_box(&self, ui: &mut Ui, o: &mut Options, x: f32, y: f32, w: f32, key: &str, section: &str) -> bool {
        let mut v = o.get(key, 0).unwrap_or_default();
        let ch = self.put(ui, self.rect(x, y, w, 22.0), egui::TextEdit::singleline(&mut v).desired_width(w)).changed();
        if ch {
            if v.trim().is_empty() {
                o.remove(key);
            } else {
                o.set(key, section, 0, v.trim());
            }
        }
        ch
    }
}

const LANGUAGES: Choices = &[("DEU", "DEU"), ("ENG", "ENG"), ("ESP", "ESP"), ("FRA", "FRA"), ("HUN", "HUN"), ("POL", "POL"), ("RUH", "RUH"), ("SRB", "SRB")];
const TICKETS: Choices = &[("0", "Ninguno"), ("1", "Fácil"), ("2", "Avanzado")];
const REFLEXIONS: Choices = &[("none", "Ninguno"), ("economy", "Económico"), ("full", "Completo")];
const WEAR: Choices = &[
    ("0", "Infinito"),
    ("1", "Mantenimiento muy malo"),
    ("2", "Mantenimiento malo"),
    ("3", "Mantenimiento normal"),
    ("4", "Buen mantenimiento"),
];

/// Size of the page area, as in OMSI's dialog.
pub const PAGE: (f32, f32) = (600.0, 505.0);

/// Draws tab `tab` (0 = General ... 5 = Tráfico IA) except the preset box of General, which the caller draws. Returns true if
/// something changed.
pub fn draw(cv: &Canvas, ui: &mut Ui, o: &mut Options, tab: usize) -> bool {
    match tab {
        0 => general(cv, ui, o),
        1 => advanced(cv, ui, o),
        2 => graphics(cv, ui, o),
        3 => graphics_advanced(cv, ui, o),
        4 => sound(cv, ui, o),
        _ => traffic(cv, ui, o),
    }
}

fn general(cv: &Canvas, ui: &mut Ui, o: &mut Options) -> bool {
    let mut c = false;
    cv.label(ui, 21.0, 19.0, 180.0, &cv.t("DG_Options_Label41", "Idioma"));
    c |= cv.combo(ui, o, 206.0, 11.0, 111.0, "language", "language", "GENERAL", LANGUAGES, false);
    cv.label(ui, 21.0, 173.0, 130.0, &cv.t("DG_Options_Label70", "Venta de billetes"));
    c |= cv.combo(ui, o, 157.0, 162.0, 112.0, "ticketselling", "ticketselling", "GENERAL", TICKETS, true);
    cv.label(ui, 21.0, 202.0, 135.0, &cv.t("DG_Options_Label42", "Vincular emisora de radio"));
    c |= cv.text_box(ui, o, 157.0, 195.0, 424.0, "radio", "GENERAL");
    c |= cv.flag(ui, o, 30.0, 245.0, 300.0, &cv.t("DG_Options_CheckBox49", "La cabeza del conductor se mueve"), "driverview_moving", "GENERAL", false);
    c |= cv.flag(ui, o, 30.0, 268.0, 330.0, &cv.t("DG_Options_CheckBox7", "Transiciones suaves de vista en la vista del conductor"), "driverview_smooth", "GENERAL", false);
    c |= cv.flag(ui, o, 30.0, 291.0, 330.0, &cv.t("DG_Options_CheckBox52", "Vista alternativa (zoom = [Shift] + botón derecho, desplazamiento = botón derecho)"), "altView", "GENERAL", false);
    c |= cv.flag(ui, o, 30.0, 358.0, 330.0, &cv.t("DG_Options_CheckBox53", "Centro automático del volante (solo control por teclado)"), "autoCenter", "GENERAL", false);
    c |= cv.flag(ui, o, 30.0, 382.0, 300.0, &cv.t("DG_Options_CheckBox57", "Embrague automático"), "no_automaticClutch", "GENERAL", true);
    c |= cv.flag(ui, o, 30.0, 405.0, 300.0, &cv.t("DG_Options_CheckBox_DynSteeringSpd", "Velocidad dinámica del volante"), "redSteerSpd", "GENERAL", false);
    c |= cv.flag(ui, o, 30.0, 453.0, 330.0, &cv.t("DG_Options_CheckBox13", "Colisiones con otros vehículos y paisaje"), "no_collision", "GENERAL", true);
    cv.label(ui, 348.0, 233.0, 200.0, &cv.t("DG_Options_Label_Wearlifespan", "Estado de mantenimiento"));
    c |= cv.combo(ui, o, 350.0, 251.0, 160.0, "wear", "wear_lifespan", "GENERAL", WEAR, true);
    cv.group(ui, 388.0, 380.0, 192.0, 97.0, &cv.t("DG_Options_GroupBox1", "Usar fecha/hora actuales"));
    c |= cv.flag(ui, o, 397.0, 412.0, 170.0, &cv.t("DG_Options_CheckBox3", "Hora"), "useActTime", "GENERAL", false);
    c |= cv.flag(ui, o, 397.0, 436.0, 170.0, &cv.t("DG_Options_CheckBox4", "Fecha"), "useActDate", "GENERAL", false);
    c |= cv.flag(ui, o, 397.0, 460.0, 175.0, &cv.t("DG_Options_CheckBox5", "Año (± desplazamiento dep. del mapa)"), "useActYear", "GENERAL", false);
    c
}

fn advanced(cv: &Canvas, ui: &mut Ui, o: &mut Options) -> bool {
    let mut c = false;
    cv.group(ui, 25.0, 24.0, 397.0, 170.0, &cv.t("DG_Options_Groupbox_GameOpt", "Opciones avanzadas del juego"));
    c |= cv.flag(ui, o, 41.0, 55.0, 370.0, &cv.t("DG_Options_CheckBox_NoSchedPopUp", "No mostrar análisis automático del horario"), "no_schedAnaPopUp", "GENERAL", false);
    c |= cv.flag(ui, o, 41.0, 84.0, 370.0, &cv.t("DG_Options_CheckBox47", "Conductor visible en tu propio autobús"), "see_own_driver", "GENERAL", false);
    c |= cv.flag(ui, o, 41.0, 113.0, 370.0, &cv.t("DG_Options_CheckBox24", "Mostrar ayuda para venta de billetes y diálogo con pasajeros"), "no_ticketinfo_visible", "GENERAL", true);
    c |= cv.flag(ui, o, 41.0, 142.0, 370.0, &cv.t("DG_Options_Autosave", "Guardado automático"), "noAutoSave", "GRAPHICS", true);
    c |= cv.flag(ui, o, 41.0, 171.0, 370.0, &cv.t("DG_Options_Showerrormessages", "Mostrar mensajes de error"), "showerrormessages", "GRAPHICS", false);
    cv.group(ui, 25.0, 223.0, 397.0, 128.0, &cv.t("DG_Options_Groupbox_Collisions", "Colisiones"));
    c |= cv.flag(ui, o, 41.0, 258.0, 370.0, &cv.t("DG_Options_CheckBox50", "Colisiones: entre el vehículo del usuario y el terreno"), "no_collision_terrain", "GENERAL", true);
    c |= cv.flag(ui, o, 41.0, 291.0, 370.0, &cv.t("DG_Options_CheckBox55", "Colisiones: entre vehículos del usuario y peatones"), "no_collision_pedastrians", "GENERAL", true);
    c |= cv.flag(ui, o, 41.0, 324.0, 370.0, &cv.t("DG_Options_CheckBox54", "Colisiones: entre vehículos IA"), "no_collision_vehToVeh", "GENERAL", true);
    cv.group(ui, 25.0, 364.0, 397.0, 104.0, &cv.t("DG_Options_Groupbox_Performance", "Rendimiento"));
    c |= cv.flags_together(
        ui,
        o,
        41.0,
        405.0,
        370.0,
        &cv.t("DG_Options_CheckBox61", "Rendimiento: cálculo multihilo"),
        &[("no_multithreading_calculate", "MULTITHREADING"), ("no_multithreading_texload", "MULTITHREADING")],
    );
    c |= cv.flag(ui, o, 41.0, 438.0, 370.0, &cv.t("DG_Options_ChckBox_LoadAllTiles", "Cargar todo el mapa al inicio"), "loadAllTiles", "MULTITHREADING", false);
    c
}

fn graphics(cv: &Canvas, ui: &mut Ui, o: &mut Options) -> bool {
    let mut c = false;
    let plain = |v: i64| v.to_string();
    cv.label(ui, 21.0, 19.0, 250.0, &cv.t("DG_Options_Label2", "Framerate objetivo"));
    c |= cv.slider_i(ui, o, 122.0, 24.0, 150.0, "maxFPS", "GRAPHICS", 0, (10, 200), (21.0, 42.0, &plain));
    cv.label(ui, 21.0, 70.0, 250.0, &cv.t("DG_Options_Label8", "Cantidad de baldosas vecinas"));
    c |= cv.slider_i(ui, o, 122.0, 75.0, 150.0, "performance_tiledistmax", "GRAPHICS", 0, (1, 20), (21.0, 93.0, &plain));
    cv.label(ui, 21.0, 120.0, 250.0, &cv.t("DG_Options_Label15", "Distancia máxima de visibilidad de objetos"));
    c |= cv.slider_f(ui, o, 122.0, 125.0, 150.0, "performance_maxObjDist", "GRAPHICS", (20.0, 5000.0), 3, Some((21.0, 143.0, &|v: f64| format!("{v:.0}"))));
    cv.label(ui, 21.0, 177.0, 260.0, &cv.t("DG_Options_Label19", "Tamaño mínimo de objeto (% tamaño de pantalla)"));
    percent(cv, ui, o, 122.0, 182.0, "performance_minObjSize", &mut c);
    cv.label(ui, 21.0, 232.0, 250.0, &cv.t("DG_Options_Label27", "… para reflexiones"));
    percent(cv, ui, o, 122.0, 237.0, "performance_minObjSizeRefl", &mut c);
    cv.label(ui, 21.0, 284.0, 160.0, &cv.t("DG_Options_Label1", "Reflexiones en tiempo real"));
    c |= cv.combo(ui, o, 181.0, 274.0, 88.0, "refl", "performance_realreflexions", "GRAPHICS", REFLEXIONS, true);

    // particle systems: the box has its own "Activo" checkbox on the border
    cv.group(ui, 21.0, 326.0, 241.0, 101.0, &cv.t("DG_Options_GroupBox4", "Sistemas de partículas"));
    let active = o.get("smokesystems", 0).is_some_and(|s| s != "0");
    ui.painter().rect_filled(cv.rect(160.0, 321.0, 90.0, 20.0), 0.0, pal().bg);
    c |= cv.int_bool(ui, o, 163.0, 331.0, 90.0, &cv.t("DG_Options_CheckBox17", "Activo"), "smokesystems", "GRAPHICS", 0);
    ui.scope(|ui| {
        ui.add_enabled_ui(active, |ui| {
            cv.label(ui, 29.0, 359.0, 120.0, &cv.t("DG_Options_Label30", "Máx. partículas por emisor"));
            c |= cv.number(ui, o, 150.0, 348.0, 100.0, "smokesystems", "GRAPHICS", 1, 0);
            c |= cv.int_bool(ui, o, 29.0, 388.0, 220.0, &cv.t("DG_Options_CheckBox18", "Partículas: solo vehículo del usuario"), "smokesystems", "GRAPHICS", 2);
            c |= cv.int_bool(ui, o, 29.0, 412.0, 220.0, &cv.t("DG_Options_CheckBox19", "Partículas: sin P.S. en reflexiones"), "smokesystems", "GRAPHICS", 3);
        });
    });

    c |= cv.flag(ui, o, 349.0, 27.0, 220.0, &cv.t("DG_Options_CheckBox23", "Efecto de brillo solar"), "sunglow", "GRAPHICS", false);
    cv.label(ui, 364.0, 94.0, 220.0, &cv.t("DG_Options_Label66", "Complejidad máxima de objetos"));
    c |= cv.slider_i(ui, o, 419.0, 105.0, 150.0, "maxcomplexity", "GRAPHICS", 0, (0, 3), (364.0, 117.0, &|v: i64| format!("{v}")));
    let obj = o.get("maxcomplexity", 0).and_then(|s| s.parse::<usize>().ok()).unwrap_or(0).min(3);
    cv.note(ui, 364.0, 140.0, 215.0, 55.0, &format!("{obj}: {}", cv.t(&format!("DG_Options_Label65_{obj}"), "")));
    cv.label(ui, 364.0, 193.0, 220.0, &cv.t("DG_Options_Label71", "Complejidad máxima del mapa"));
    c |= cv.slider_i(ui, o, 419.0, 225.0, 150.0, "maxcomplexity_map", "GRAPHICS", 0, (0, 2), (364.0, 237.0, &|v: i64| format!("{v}")));
    let map = o.get("maxcomplexity_map", 0).and_then(|s| s.parse::<usize>().ok()).unwrap_or(0).min(2);
    cv.note(ui, 364.0, 266.0, 215.0, 40.0, &format!("{map}: {}", cv.t(&format!("DG_Options_Label72_{map}"), "")));

    cv.group(ui, 293.0, 339.0, 256.0, 96.0, "");
    ui.painter().rect_filled(cv.rect(298.0, 329.0, 190.0, 20.0), 0.0, pal().bg);
    c |= cv.flag(ui, o, 301.0, 339.0, 190.0, &cv.t("DG_Options_CheckBox28", "Efectos de búfer stencil"), "no_stencilbuffer", "GRAPHICS", true);
    let stencil = !o.has("no_stencilbuffer");
    ui.scope(|ui| {
        ui.add_enabled_ui(stencil, |ui| {
            c |= cv.on_off(ui, o, 318.0, 369.0, 200.0, &cv.t("DG_Options_CheckBox6", "Sombras"), "shadow_stencil", "GRAPHICS");
            c |= cv.flag(ui, o, 318.0, 391.0, 220.0, &cv.t("DG_Options_CheckBox20", "Reflexiones de lluvia"), "no_rain_refl", "GRAPHICS", true);
            c |= cv.flag(ui, o, 333.0, 413.0, 215.0, &cv.t("DG_Options_CheckBox21", "Personas visibles en reflexiones de lluvia"), "no_humans_on_rain_refl", "GRAPHICS", true);
        });
    });
    c
}

/// A "% of screen size" slider: the file holds a fraction (0.046), OMSI shows 4.60 %.
fn percent(cv: &Canvas, ui: &mut Ui, o: &mut Options, x: f32, y: f32, key: &str, changed: &mut bool) {
    let mut v: f64 = o.get(key, 0).and_then(|s| s.parse::<f64>().ok()).unwrap_or(0.0) * 100.0;
    if cv.put(ui, cv.rect(x, y, 150.0, 20.0), egui::Slider::new(&mut v, 0.0..=50.0).show_value(false)).changed() {
        o.set(key, "GRAPHICS", 0, &format!("{:.3}", v / 100.0));
        *changed = true;
    }
    cv.value(ui, 21.0, y + 18.0, &format!("{v:.2} %"));
}

fn graphics_advanced(cv: &Canvas, ui: &mut Ui, o: &mut Options) -> bool {
    let mut c = false;
    // manual screen ratio: a key that is only present when the box is ticked
    let mut manual = o.has("screenratio");
    if cv.put_check(ui, 261.0, 19.0, 200.0, &cv.t("DG_Options_CheckBox1", "Relación de pantalla manual"), &mut manual) {
        if manual {
            o.set("screenratio", "GRAPHICS", 0, "1.778");
        } else {
            o.remove("screenratio");
        }
        c = true;
    }
    ui.scope(|ui| {
        ui.add_enabled_ui(manual, |ui| {
            c |= cv.number(ui, o, 461.0, 11.0, 120.0, "screenratio", "GRAPHICS", 0, 3);
        });
    });

    cv.group(ui, 17.0, 70.0, 183.0, 89.0, &cv.t("DG_Options_GroupBox5", "Modo económico de reflexiones forzadas"));
    cv.label(ui, 21.0, 98.0, 125.0, &cv.t("DG_Options_Label31", "Modo económico de reflexiones: activar por debajo de FPS"));
    c |= cv.number(ui, o, 150.0, 92.0, 48.0, "performance_dyn_redrefl", "GRAPHICS", 0, 3);
    cv.label(ui, 21.0, 122.0, 125.0, &cv.t("DG_Options_Label32", "Modo económico de reflexiones: desactivar por encima de FPS"));
    c |= cv.number(ui, o, 150.0, 116.0, 48.0, "performance_dyn_redrefl", "GRAPHICS", 1, 3);

    cv.group(ui, 17.0, 165.0, 183.0, 85.0, &cv.t("DG_Options_GroupBox6", "Reducción dinámica del número de losas"));
    cv.note(ui, 21.0, 176.0, 125.0, 40.0, &cv.t("DG_Options_Label33", "Reducción dinámica de losas: reducir por debajo de FPS"));
    c |= cv.number(ui, o, 150.0, 172.0, 48.0, "performance_dyn_tile_red", "GRAPHICS", 0, 3);
    cv.note(ui, 21.0, 208.0, 125.0, 40.0, &cv.t("DG_Options_Label34", "Reducción dinámica de losas: aumentar por encima de FPS"));
    c |= cv.number(ui, o, 150.0, 206.0, 48.0, "performance_dyn_tile_red", "GRAPHICS", 1, 3);

    cv.group(ui, 205.0, 74.0, 375.0, 346.0, &cv.t("DG_Options_GroupBox3", "Texturas..."));
    c |= cv.flag(ui, o, 215.0, 98.0, 355.0, &cv.t("DG_Options_CheckBox16", "Usar solo texturas de baja resolución"), "texture_uselow", "GRAPHICS", false);
    c |= cv.flag(ui, o, 215.0, 122.0, 355.0, &cv.t("DG_Options_CheckBox48", "Limitación de todas las texturas (excepto las del propio autobús) a 256 px"), "texmax256", "GRAPHICS", false);
    cv.note(ui, 215.0, 191.0, 255.0, 40.0, &cv.t("DG_Options_Label35", "Mem. máx. de texturas para texturas de alta resolución (MB)"));
    c |= cv.number(ui, o, 478.0, 196.0, 70.0, "texmemlimit", "GRAPHICS", 0, 1);
    cv.label(ui, 556.0, 207.0, 24.0, "MB");
    cv.label(ui, 215.0, 247.0, 355.0, &cv.t("DG_Options_Label28", "Tamaño de textura de reflexiones en tiempo real"));
    let mut size: i64 = o.get("performance_reflTexSize", 0).and_then(|s| s.parse().ok()).unwrap_or(8).clamp(0, 13);
    if cv.put(ui, cv.rect(290.0, 257.0, 148.0, 20.0), egui::Slider::new(&mut size, 0..=12).show_value(false)).changed() {
        o.set("performance_reflTexSize", "GRAPHICS", 0, &size.to_string());
        c = true;
    }
    cv.value(ui, 215.0, 272.0, &format!("{}", 1u32 << size));

    cv.group(ui, 17.0, 370.0, 140.0, 105.0, &cv.t("DG_Options_GroupBox2", "Usar canales de material..."));
    for (i, (label, key)) in [
        ("Night Map", "no_nightmap"),
        ("Light Map", "no_lightmap"),
        ("Light Map of Terrain", "no_lightmap_terr"),
        ("Reflexion Map", "no_reflmap"),
        ("Bump Map", "no_bumpmap"),
    ]
    .into_iter()
    .enumerate()
    {
        c |= cv.flag(ui, o, 29.0, 395.0 + i as f32 * 17.0, 125.0, label, key, "GRAPHICS", true);
    }
    c
}

fn sound(cv: &Canvas, ui: &mut Ui, o: &mut Options) -> bool {
    let mut c = false;
    cv.label(ui, 21.0, 19.0, 160.0, &cv.t("DG_Options_Label10", "Volumen general"));
    // OMSI's slider has ten steps (1 to 10): the file holds step / 10
    let mut vol: i64 = (o.get("sound_vol_master", 0).and_then(|s| s.parse::<f64>().ok()).unwrap_or(1.0) * 10.0).round().clamp(1.0, 10.0) as i64;
    if cv.put(ui, cv.rect(187.0, 8.0, 150.0, 20.0), egui::Slider::new(&mut vol, 1..=10).show_value(false)).changed() {
        o.set("sound_vol_master", "SOUND", 0, &format!("{:.6}", vol as f64 / 10.0));
        c = true;
    }
    cv.label(ui, 21.0, 59.0, 160.0, &cv.t("DG_Options_Label6", "Efecto estéreo"));
    c |= cv.slider_i(ui, o, 187.0, 64.0, 150.0, "sound_stereo", "SOUND", 0, (0, 100), (21.0, 75.0, &|v: i64| v.to_string()));
    cv.label(ui, 21.0, 154.0, 200.0, &cv.t("DG_Options_Label17", "Recuento máximo de sonidos"));
    c |= cv.slider_i(ui, o, 187.0, 169.0, 150.0, "sound_maxcount", "SOUND", 0, (5, 1000), (21.0, 178.0, &|v: i64| v.to_string()));
    c |= cv.flag(ui, o, 30.0, 220.0, 300.0, &cv.t("DG_Options_CheckBox14", "Sonidos de vehículos IA activos"), "sound_ai", "SOUND", false);
    c |= cv.flag(ui, o, 30.0, 244.0, 300.0, &cv.t("DG_Options_CheckBox15", "Sonidos del entorno activos"), "sound_scenery", "SOUND", false);
    c |= cv.on_off(ui, o, 30.0, 268.0, 300.0, &cv.t("DG_Options_CheckBox2", "Efecto Doppler"), "sound_doppler", "SOUND");
    c |= cv.flag(ui, o, 30.0, 290.0, 300.0, &cv.t("DG_Options_CheckBox_Reverb", "Efectos de reverberación"), "sound_noreverb", "SOUND", true);
    c
}

fn traffic(cv: &Canvas, ui: &mut Ui, o: &mut Options) -> bool {
    let mut c = false;
    cv.label(ui, 21.0, 19.0, 150.0, &cv.t("DG_Options_Label4", "Recuento máx. de tráfico rodado"));
    cv.label(ui, 165.0, 19.0, 190.0, &cv.t("DG_Options_Label21", "No programado (ej.: coches)"));
    c |= cv.slider_i(ui, o, 155.0, 21.0, 148.0, "AIMaxCountRandom", "AI", 0, (0, 1000), (121.0, 40.0, &|v: i64| v.to_string()));
    cv.label(ui, 388.0, 19.0, 190.0, &cv.t("DG_Options_Label22", "Programado (ej.: autobuses)"));
    c |= cv.slider_i(ui, o, 380.0, 21.0, 148.0, "AIMaxCountScheduled", "AI", 0, (0, 1000), (333.0, 40.0, &|v: i64| v.to_string()));

    cv.label(ui, 21.0, 100.0, 140.0, &cv.t("DG_Options_Label_RTFactor", "Factor de tráfico en la carretera"));
    c |= cv.slider_i(ui, o, 230.0, 90.0, 100.0, "AIUnschedFactor", "AI", 0, (1, 300), (185.0, 100.0, &|v: i64| format!("{v} %")));
    cv.label(ui, 332.0, 82.0, 200.0, &cv.t("DG_Options_Label67", "Prioridad del horario"));
    c |= cv.slider_i(ui, o, 378.0, 93.0, 148.0, "AIPriorityScheduled", "AI", 0, (1, 4), (378.0, 132.0, &|_v: i64| "1".to_string()));
    cv.value(ui, 520.0, 132.0, "4");

    cv.label(ui, 21.0, 169.0, 130.0, &cv.t("DG_Options_Label25", "Coches estacionados"));
    c |= cv.slider_i(ui, o, 230.0, 154.0, 100.0, "AIMaxCountParked", "AI", 0, (0, 100), (185.0, 169.0, &|v: i64| format!("{v} %")));
    c |= cv.flag(ui, o, 349.0, 192.0, 190.0, &cv.t("DG_Options_CheckBox51", "Usar lista de IA reducida (menos tipos diferentes)"), "useLowAIList", "AI", false);

    cv.label(ui, 21.0, 264.0, 200.0, &cv.t("DG_Options_Label13", "Recuento máx. de personas"));
    c |= cv.slider_i(ui, o, 155.0, 273.0, 148.0, "AIMaxCountRandom", "AI", 1, (0, 1000), (110.0, 289.0, &|v: i64| v.to_string()));
    cv.note(ui, 329.0, 294.0, 225.0, 50.0, &cv.t("DG_Options_Label39", "El valor de Recuento máx. de personas es el límite superior para todas las personas visibles."));

    cv.label(ui, 21.0, 337.0, 220.0, &cv.t("DG_Options_Label37", "Factor de pasajeros en estaciones"));
    c |= cv.slider_i(ui, o, 155.0, 352.0, 148.0, "AIPassFactor", "AI", 0, (0, 200), (110.0, 362.0, &|v: i64| format!("{v} %")));
    cv.note(ui, 329.0, 379.0, 225.0, 70.0, &cv.t("DG_Options_Label40", "¡Ten en cuenta que no puede haber más pasajeros que el valor de Recuento máx. de personas en todas las estaciones del área visible!"));
    let _ = Color32::WHITE;
    c
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Draws every tab in every launcher language on a real copy of the user's options, without a window: it must not panic and a
    /// plain draw must not report changes.
    #[test]
    fn every_tab_draws_in_every_language() {
        let omsi = std::path::Path::new(r"C:\Program Files (x86)\Steam\steamapps\common\OMSI 2");
        let Some(mut options) = Options::load(omsi) else { return };
        let before = options.to_bytes();
        let ctx = egui::Context::default();
        for lang in Lang::ALL {
            i18n::set_lang(lang);
            let labels = crate::olf::load(omsi, lang);
            for tab in 0..6 {
                let mut changed = false;
                let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
                    let (page, _) = ui.allocate_exact_size(vec2(PAGE.0, PAGE.1), egui::Sense::hover());
                    let cv = Canvas { origin: page.min, omsi: &labels };
                    changed |= draw(&cv, ui, &mut options, tab);
                });
                assert!(!changed, "tab {tab} in {lang:?} reported a change without any input");
            }
        }
        i18n::set_lang(Lang::Es);
        assert_eq!(options.to_bytes(), before);
    }
}
