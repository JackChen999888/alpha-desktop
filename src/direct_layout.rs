//! Layout editing on the island itself. Changes commit once at pointer release.
use crate::{
    bar_layout as bar,
    paint::{Card, H},
    theme::{self, W},
};
use eframe::egui::{self, Pos2, Rect, Vec2};
#[derive(Default)]
pub(crate) struct State {
    pub editing: bool,
    pub toolbar: Option<Rect>,
    drag: Option<usize>,
    resize: Option<(u32, f32, f32)>,
    draft: Option<u32>,
}
pub(crate) fn reordered(order: &[usize], from: usize, to: usize) -> Vec<usize> {
    let mut order = order.to_vec();
    if let (Some(a), Some(b)) = (
        order.iter().position(|v| *v == from),
        order.iter().position(|v| *v == to),
    ) {
        order.remove(a);
        order.insert(b, from);
    }
    order
}
fn size_from_drag(start: u32, delta: f32, width: f32) -> u32 {
    crate::model::island_size_value(
        (start as f32 * (1. + delta * 2. / width.max(1.)))
            .round()
            .max(0.) as u32,
    )
}
impl crate::app::App {
    pub(crate) fn direct_layout_controls(&mut self, ui: &mut egui::Ui, panel: Rect, progress: f32) {
        self.direct_layout.toolbar = None;
        if !self.direct_layout.editing {
            return;
        }
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.direct_layout = State::default();
            return;
        }
        if progress > 0.03 {
            return;
        }
        let layout = self.resident.current();
        let fit = ((crate::app::WIN_W - 32.) / layout.width).min(1.);
        let mut order = if bar::valid_order(&self.settings.resident_order) {
            self.settings.resident_order.clone()
        } else {
            let mut slots: Vec<_> = bar::DEFAULT_ORDER.iter().copied().collect();
            slots.sort_by(|a, b| {
                layout.x[*a]
                    .unwrap_or(10000.)
                    .total_cmp(&layout.x[*b].unwrap_or(10000.))
            });
            slots
        };
        if !order.contains(&bar::MONITOR) {
            order.push(bar::MONITOR);
        }
        let mut target = None;
        for slot in order.iter().copied() {
            let group: Vec<_> = if slot == bar::QUOTA {
                layout.quota_slots().collect()
            } else {
                vec![slot]
            };
            let left = group
                .iter()
                .filter_map(|&i| layout.x[i].map(|x| x - layout.widths[i] / 2.))
                .reduce(f32::min);
            let right = group
                .iter()
                .filter_map(|&i| layout.x[i].map(|x| x + layout.widths[i] / 2.))
                .reduce(f32::max);
            let (Some(left), Some(right)) = (left, right) else {
                continue;
            };
            let rect = Rect::from_min_max(
                Pos2::new(panel.center().x + left * fit, panel.top() + 1.),
                Pos2::new(panel.center().x + right * fit, panel.bottom() - 1.),
            );
            let response = ui
                .interact(
                    rect,
                    egui::Id::new(("direct-component", slot)),
                    egui::Sense::drag(),
                )
                .on_hover_cursor(egui::CursorIcon::Grab);
            if response.drag_started() {
                self.direct_layout.drag = Some(slot);
            }
            if self.direct_layout.drag.is_some()
                && ui
                    .ctx()
                    .pointer_interact_pos()
                    .is_some_and(|p| rect.contains(p))
            {
                target = Some(slot);
            }
            if response.hovered() || target == Some(slot) {
                ui.painter().rect_stroke(
                    rect,
                    4.,
                    egui::Stroke::new(1., theme::BLUE),
                    egui::StrokeKind::Inside,
                );
            }
        }
        if ui.input(|i| i.pointer.any_released()) {
            if let (Some(from), Some(to)) = (self.direct_layout.drag.take(), target) {
                self.settings.resident_order = reordered(&order, from, to);
                self.settings.save();
            }
        }
        for side in [-1., 1.] {
            let x = if side < 0. {
                panel.left() + 3.
            } else {
                panel.right() - 3.
            };
            let rect = Rect::from_center_size(
                Pos2::new(x, panel.center().y),
                Vec2::new(6., panel.height()),
            );
            let r = ui
                .interact(
                    rect,
                    egui::Id::new(("direct-resize", side as i32)),
                    egui::Sense::drag(),
                )
                .on_hover_cursor(egui::CursorIcon::ResizeHorizontal);
            ui.painter().line_segment(
                [
                    Pos2::new(x, panel.top() + 6.),
                    Pos2::new(x, panel.bottom() - 6.),
                ],
                egui::Stroke::new(2., theme::BLUE),
            );
            if r.drag_started() {
                let start_x = ui
                    .input(|i| i.pointer.press_origin())
                    .map(|p| p.x)
                    .unwrap_or(x);
                self.direct_layout.resize =
                    Some((self.settings.island_size, panel.width(), start_x));
            }
            if r.dragged() {
                if let Some((start, width, start_x)) = self.direct_layout.resize {
                    let delta = ui
                        .ctx()
                        .pointer_interact_pos()
                        .map(|p| p.x - start_x)
                        .unwrap_or(0.);
                    self.direct_layout.draft = Some(size_from_drag(start, delta * side, width));
                }
            }
            if r.drag_stopped() {
                if let Some(value) = self.direct_layout.draft.take() {
                    self.settings.island_size = value;
                    self.settings.save();
                }
                self.direct_layout.resize = None;
            }
        }
        let rect = Rect::from_min_size(
            Pos2::new(panel.center().x - 165., panel.bottom() + 6.),
            Vec2::new(330., 32.),
        );
        self.direct_layout.toolbar = Some(rect);
        let card = Card::new(ui, rect.min, ui.max_rect());
        card.fill(0., 0., 330., 32., 10., theme::CARD);
        let text = self
            .direct_layout
            .draft
            .map(|v| format!("大小 {v}% · 松开应用"))
            .unwrap_or_else(|| "拖动组件排序 · 拖动两侧缩放".into());
        card.centered_text(12., 16., &text, 11., W::Regular, theme::WHITE, H::Left);
        card.centered_text(298., 16., "完成", 11., W::Semibold, theme::BLUE, H::Center);
        if card.hit(270., 2., 58., 28., "direct-layout-done").clicked() {
            self.direct_layout = State::default();
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn direct_layout_reorder_preserves_hidden_and_monitor() {
        let order = [
            bar::CLOCK,
            bar::BATTERY,
            bar::QUOTA,
            bar::MUSIC,
            bar::FOCUS,
            bar::PET,
            bar::MONITOR,
        ];
        let moved = reordered(&order, bar::MONITOR, bar::CLOCK);
        assert_eq!(moved[0], bar::MONITOR);
        assert!(bar::valid_order(&moved));
        assert_eq!(reordered(&order, 99, bar::CLOCK), order);
    }
    #[test]
    fn direct_layout_size_clamps_and_steps() {
        assert_eq!(size_from_drag(100, 1000., 200.), 140);
        assert_eq!(size_from_drag(100, -1000., 200.), 80);
        assert_eq!(size_from_drag(100, 10., 200.), 110);
    }
}
