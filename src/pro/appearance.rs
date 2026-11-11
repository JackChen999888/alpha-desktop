//! Shared settings-style visual language for Alpha's native windows.
use eframe::egui::{self, Color32, RichText};
pub fn color(ctx: &egui::Context, dark: Color32, light: Color32) -> Color32 {
    if ctx.style().visuals.dark_mode {
        dark
    } else {
        light
    }
}
pub fn apply(ctx: &egui::Context, dark: bool) {
    let mut style = (*ctx.style()).clone();
    style.visuals = if dark {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    };
    style.visuals.override_text_color = Some(if dark {
        Color32::from_rgb(242, 243, 247)
    } else {
        Color32::from_rgb(30, 34, 41)
    });
    style.visuals.panel_fill = if dark {
        Color32::from_rgb(25, 25, 27)
    } else {
        Color32::from_rgb(246, 247, 249)
    };
    style.visuals.extreme_bg_color = if dark {
        Color32::from_rgb(29, 29, 32)
    } else {
        Color32::from_rgb(244, 246, 249)
    };
    style.visuals.widgets.noninteractive.fg_stroke.color = if dark {
        Color32::from_rgb(235, 237, 243)
    } else {
        Color32::from_rgb(30, 34, 41)
    };
    style.visuals.selection.bg_fill = if dark {
        Color32::from_rgb(45, 105, 165)
    } else {
        Color32::from_rgb(209, 226, 248)
    };
    let fill = if dark {
        Color32::from_rgb(50, 50, 54)
    } else {
        Color32::from_rgb(228, 231, 236)
    };
    let hover = if dark {
        Color32::from_rgb(65, 65, 71)
    } else {
        Color32::from_rgb(216, 222, 231)
    };
    let widgets = &mut style.visuals.widgets;
    for (w, c) in [
        (&mut widgets.inactive, fill),
        (&mut widgets.hovered, hover),
        (&mut widgets.active, hover),
    ] {
        w.corner_radius = egui::CornerRadius::same(7);
        w.bg_fill = c;
        w.weak_bg_fill = c;
        w.bg_stroke = egui::Stroke::NONE;
    }
    style.spacing.item_spacing = egui::vec2(10., 10.);
    style.spacing.button_padding = egui::vec2(12., 7.);
    style.spacing.interact_size.y = 30.;
    for (kind, size) in [
        (egui::TextStyle::Body, 14.),
        (egui::TextStyle::Button, 14.),
        (egui::TextStyle::Small, 12.),
        (egui::TextStyle::Heading, 24.),
    ] {
        style
            .text_styles
            .insert(kind, egui::FontId::proportional(size));
    }
    ctx.set_style(style);
    ctx.send_viewport_cmd(egui::ViewportCommand::SetTheme(if dark {
        egui::SystemTheme::Dark
    } else {
        egui::SystemTheme::Light
    }));
}
pub fn muted(ui: &mut egui::Ui, text: impl Into<String>) {
    let c = color(
        ui.ctx(),
        Color32::from_rgb(181, 186, 199),
        Color32::from_rgb(80, 87, 100),
    );
    ui.label(RichText::new(text).color(c));
}
pub fn empty(ui: &mut egui::Ui, title: &str, description: &str) {
    ui.add_space(22.);
    crate::extras::settings_card(ui, |ui| {
        ui.add_space(18.);
        ui.vertical_centered(|ui| {
            ui.label(RichText::new(title).size(18.).strong());
            ui.add_space(6.);
            muted(ui, description);
        });
        ui.add_space(18.);
    });
}
pub fn sidebar(ui: &mut egui::Ui, index: usize, title: &str, selected: bool) -> bool {
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 42.), egui::Sense::click());
    if selected || response.hovered() {
        ui.painter().rect_filled(
            rect,
            8.,
            color(
                ui.ctx(),
                Color32::from_rgb(43, 43, 48),
                Color32::from_rgb(214, 221, 232),
            ),
        );
    }
    let icon = match index {
        0 => egui::include_image!("../../assets/settings-icons/system/pages.svg"),
        1 => egui::include_image!("../../assets/settings-icons/system/quota.svg"),
        2 => egui::include_image!("../../assets/settings-icons/system/tasks.svg"),
        3 => egui::include_image!("../../assets/settings-icons/system/general.svg"),
        4 | 8 => egui::include_image!("../../assets/settings-icons/system/quota.svg"),
        5 => egui::include_image!("../../assets/settings-icons/system/general.svg"),
        6 => egui::include_image!("../../assets/settings-icons/system/music.svg"),
        _ => egui::include_image!("../../assets/settings-icons/system/notifications.svg"),
    };
    egui::Image::new(icon).paint_at(
        ui,
        egui::Rect::from_center_size(
            egui::pos2(rect.left() + 21., rect.center().y),
            egui::vec2(25., 25.),
        ),
    );
    ui.painter().text(
        egui::pos2(rect.left() + 42., rect.center().y),
        egui::Align2::LEFT_CENTER,
        title,
        egui::FontId::proportional(14.),
        color(
            ui.ctx(),
            Color32::from_rgb(242, 243, 247),
            Color32::from_rgb(30, 34, 41),
        ),
    );
    response.clicked()
}
