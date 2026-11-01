//! Windows of their own, built on egui's immediate viewports.

use eframe::egui::{self, Context, Pos2, Ui, Vec2};

/// Shows `add` in a window called `title`, whose first size is `size` and which then follows what its contents need. It opens in the
/// middle of the launcher the first time and stays where the player puts it as long as it is shown. `key` tells it apart from the
/// other windows. Returns false once the player has closed it with the system's close button.
pub fn window(ctx: &Context, key: &str, title: &str, size: Vec2, add: impl FnOnce(&mut Ui)) -> bool {
    #[allow(deprecated)] // a key that stays the same from one frame to the next is what is needed here
    let pos_id = egui::Id::new(("free-window-pos", key));
    #[allow(deprecated)]
    let size_id = egui::Id::new(("free-window-size", key));
    let now = std::time::Instant::now();
    // where it opens: kept while it is shown without a break, worked out again after it has been away
    let stored: Option<(Pos2, std::time::Instant)> = ctx.data(|d| d.get_temp(pos_id));
    let pos = match stored {
        Some((p, seen)) if now.duration_since(seen) < std::time::Duration::from_millis(1500) => p,
        _ => {
            let parent = ctx.input(|i| i.viewport().outer_rect);
            parent.map_or(Pos2::new(120.0, 80.0), |r| Pos2::new((r.center().x - size.x / 2.0).max(0.0), (r.center().y - size.y / 2.0).max(0.0)))
        }
    };
    ctx.data_mut(|d| d.insert_temp(pos_id, (pos, now)));
    let current: Vec2 = ctx.data(|d| d.get_temp(size_id)).unwrap_or(size);
    let builder = egui::ViewportBuilder::default()
        .with_title(title)
        .with_inner_size(current)
        .with_resizable(false)
        .with_minimize_button(false)
        .with_maximize_button(false)
        .with_always_on_top() // the launcher itself may be on top, and then it would hide its own windows
        .with_position(pos);
    let mut add = Some(add);
    let mut open = true;
    let mut wanted: Option<Vec2> = None;
    ctx.show_viewport_immediate(egui::ViewportId::from_hash_of(("free-window", key)), builder, |ui, _class| {
        if ui.ctx().input(|i| i.viewport().close_requested()) {
            open = false;
        }
        let fill = ui.visuals().window_fill;
        egui::CentralPanel::default().frame(egui::Frame::new().fill(fill).inner_margin(egui::Margin::same(10))).show(ui, |ui| {
            let used = ui
                .scope(|ui| {
                    if let Some(f) = add.take() {
                        f(ui);
                    }
                })
                .response
                .rect;
            wanted = Some(Vec2::new(used.width() + 20.0, used.height() + 20.0));
        });
    });
    // the window grows or shrinks to what its contents took
    if let Some(w) = wanted {
        let w = Vec2::new(w.x.max(160.0).ceil(), w.y.max(80.0).ceil());
        if (w.x - current.x).abs() > 3.0 || (w.y - current.y).abs() > 3.0 {
            ctx.data_mut(|d| d.insert_temp(size_id, w));
            ctx.request_repaint();
        }
    }
    open
}
