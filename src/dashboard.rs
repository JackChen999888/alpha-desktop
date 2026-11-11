//! In-island monitoring, quota and device dashboards. All numbers retain their source semantics.
use crate::{
    app::{App, Cmd, Env, PW},
    monitor::Reading,
    paint::{Card, H},
    theme::{self, W},
};
use eframe::egui::{self, Color32 as C, Pos2, Rect, Vec2};
const NAMES: [&str; 6] = ["核心柱", "CPU", "内存", "GPU", "下载", "上传"];
const COLORS: [[u8; 3]; 7] = [
    [70, 160, 255],
    [70, 160, 255],
    [92, 222, 210],
    [235, 115, 176],
    [70, 160, 255],
    [92, 222, 210],
    [92, 222, 210],
];
const SWATCHES: [(&str, [u8; 3]); 16] = [
    ("蓝色", [70, 160, 255]),
    ("天蓝", [91, 199, 247]),
    ("青色", [65, 210, 201]),
    ("绿色", [68, 200, 112]),
    ("薄荷", [153, 228, 177]),
    ("黄色", [250, 210, 80]),
    ("橙色", [246, 158, 73]),
    ("红色", [241, 91, 101]),
    ("粉色", [241, 130, 184]),
    ("紫色", [181, 137, 244]),
    ("靛蓝", [128, 139, 240]),
    ("棕色", [184, 141, 108]),
    ("白色", [240, 244, 249]),
    ("浅灰", [181, 191, 204]),
    ("深灰", [104, 116, 135]),
    ("黑色", [39, 46, 59]),
];
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Config {
    pub size: [u16; 2],
    pub colors: [[u8; 3]; 7],
    pub follow_theme: bool,
    pub order: [usize; 6],
    pub numbers: [bool; 6],
    pub visible: [bool; 6],
    pub device_rings: bool,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            size: [590, 250],
            colors: COLORS,
            follow_theme: true,
            order: [0, 1, 2, 3, 4, 5],
            numbers: [false, true, true, true, true, true],
            visible: [false, true, true, true, true, true],
            device_rings: true,
        }
    }
}
impl Config {
    fn palette(&mut self, ui: &mut egui::Ui, index: usize) -> egui::Response {
        let color = self.color(index);
        ui.push_id(("monitor-palette", index), |ui| {
            ui.menu_button(egui::RichText::new("■").size(26.).color(color), |ui| {
                ui.label(egui::RichText::new("选择基础色").strong());
                for row in 0..4 {
                    ui.horizontal(|ui| {
                        for col in 0..4 {
                            let k = row * 4 + col;
                            let (name, rgb) = SWATCHES[k];
                            let (rect, _) =
                                ui.allocate_exact_size(egui::vec2(30., 30.), egui::Sense::hover());
                            let response = ui
                                .interact(
                                    rect,
                                    egui::Id::new(("monitor-swatch", index, k)),
                                    egui::Sense::click(),
                                )
                                .on_hover_text(name);
                            ui.painter().rect_filled(
                                rect.shrink(2.),
                                6.,
                                C::from_rgb(rgb[0], rgb[1], rgb[2]),
                            );
                            if self.colors[index] == rgb && !self.follow_theme {
                                ui.painter().rect_stroke(
                                    rect,
                                    7.,
                                    egui::Stroke::new(2., ui.visuals().text_color()),
                                    egui::StrokeKind::Inside,
                                );
                            }
                            if response.clicked() {
                                if self.follow_theme {
                                    self.colors = COLORS;
                                }
                                self.colors[index] = rgb;
                                self.follow_theme = false;
                                ui.close_menu();
                            }
                        }
                    });
                }
                ui.separator();
                ui.horizontal(|ui| {
                    ui.label("自定义颜色");
                    if ui.color_edit_button_srgb(&mut self.colors[index]).changed() {
                        let selected = self.colors[index];
                        if self.follow_theme {
                            self.colors = COLORS;
                            self.colors[index] = selected;
                        }
                        self.follow_theme = false;
                    }
                });
                crate::extras::settings_hint(ui, "选色后自动使用自定义配色，其他项目保持原色。");
            })
            .response
            .on_hover_text("点击选择基础色或自定义颜色")
        })
        .inner
    }
    pub fn order(&self) -> [usize; 6] {
        if (0..6).all(|i| self.order.iter().filter(|v| **v == i).count() == 1) {
            self.order
        } else {
            [0, 1, 2, 3, 4, 5]
        }
    }
    pub fn color(&self, i: usize) -> C {
        let c = if self.follow_theme {
            COLORS[i]
        } else {
            self.colors[i]
        };
        C::from_rgb(c[0], c[1], c[2])
    }
    pub fn shown(&self, m: crate::monitor::Config, i: usize) -> bool {
        self.visible[i]
            && match i {
                0 => true,
                1 => m.cpu,
                2 => m.memory,
                3 => m.gpu,
                _ => m.network,
            }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Panel {
    Monitor,
    Quota,
    Devices,
}
#[derive(Default)]
pub struct State {
    pub page: usize,
    pub memory_sort: bool,
    resize: Option<([u16; 2], Pos2)>,
    process_icons: crate::icons::Cache,
}
impl State {
    pub(crate) fn release_previews(&mut self){self.process_icons.release();}
}
fn value(r: &Reading, i: usize) -> Option<f64> {
    match i {
        1 => r.cpu,
        2 => r.memory,
        3 => r.gpu,
        4 => r.network.map(|n| n.0),
        5 => r.network.map(|n| n.1),
        _ => r.cpu,
    }
}
fn text(v: Option<f64>, network: bool) -> String {
    v.map(|n| {
        if network {
            if n >= 1048576. {
                format!("{:.1}M", n / 1048576.)
            } else {
                format!("{:.0}K", n / 1024.)
            }
        } else {
            format!("{n:.0}%")
        }
    })
    .unwrap_or_else(|| "—".into())
}
pub fn compact_width(m: crate::monitor::Config, d: &Config) -> f32 {
    d.order()
        .into_iter()
        .filter(|&i| d.shown(m, i))
        .map(|i| {
            if i == 0 {
                if d.numbers[0] {
                    86.
                } else {
                    60.
                }
            } else if d.numbers[i] {
                if i >= 4 {
                    57.
                } else {
                    48.
                }
            } else {
                26.
            }
        })
        .sum()
}
pub fn compact(c: &Card, x: f32, r: &Reading, m: crate::monitor::Config, d: &Config) {
    let mut at = x - compact_width(m, d) / 2.;
    for i in d.order().into_iter().filter(|&i| d.shown(m, i)) {
        let w = if i == 0 {
            if d.numbers[0] {
                86.
            } else {
                60.
            }
        } else if d.numbers[i] {
            if i >= 4 {
                57.
            } else {
                48.
            }
        } else {
            26.
        };
        let color = d.color(i);
        if i == 0 {
            if d.numbers[0] {
                c.centered_text(
                    at + 72.,
                    15.,
                    &text(r.cpu, false),
                    9.,
                    W::Semibold,
                    color,
                    H::Center,
                );
            }
            let count = r.cores.len().min(32);
            if count == 0 {
                c.centered_text(
                    at + 30.,
                    15.,
                    "核心 —",
                    9.,
                    W::Regular,
                    theme::GRAY,
                    H::Center,
                );
            } else {
                let bw = 56. / count as f32;
                for (n, core) in r.cores.iter().take(count).enumerate() {
                    c.fill(
                        at + 2. + n as f32 * bw,
                        24. - core.load as f32 * 0.18,
                        bw.max(1.) - 0.5,
                        (core.load as f32 * 0.18).max(1.),
                        0.5,
                        if core.efficient { d.color(6) } else { color },
                    );
                }
            }
        } else if d.numbers[i] {
            c.centered_text(at + w / 2., 8., NAMES[i], 7.5, W::Regular, color, H::Center);
            c.centered_text(
                at + w / 2.,
                20.,
                &text(value(r, i), i >= 4),
                10.,
                W::Semibold,
                theme::WHITE,
                H::Center,
            );
        } else if i < 4 {
            c.ring(at + 13., 15., 8., 1.8, theme::BTN);
            if let Some(v) = value(r, i) {
                c.arc(
                    at + 13.,
                    15.,
                    8.,
                    -std::f32::consts::FRAC_PI_2,
                    -std::f32::consts::FRAC_PI_2 + std::f32::consts::TAU * v as f32 / 100.,
                    1.8,
                    color,
                );
            }
            c.centered_text(
                at + 13.,
                15.,
                ["", "C", "M", "G"][i],
                8.,
                W::Bold,
                color,
                H::Center,
            );
        } else {
            c.centered_text(
                at + 13.,
                15.,
                if i == 4 { "↓" } else { "↑" },
                16.,
                W::Bold,
                color,
                H::Center,
            );
        }
        at += w;
    }
}
fn spark(c: &Card, r: Rect, values: impl Iterator<Item = Option<f64>>, max: Option<f64>, color: C) {
    let values: Vec<_> = values.collect();
    let scale = max
        .unwrap_or_else(|| values.iter().flatten().copied().fold(1., f64::max))
        .max(1.);
    let mut segment = Vec::new();
    for (i, v) in values.iter().enumerate() {
        if let Some(v) = v {
            segment.push((
                r.left() + i as f32 / 59. * r.width(),
                r.bottom() - (*v / scale).clamp(0., 1.) as f32 * r.height(),
            ));
        } else {
            if segment.len() > 1 {
                c.path(&segment, 1.2, color);
            }
            segment.clear();
        }
    }
    if segment.len() > 1 {
        c.path(&segment, 1.2, color);
    }
}
impl App {
    pub(crate) fn dashboard_kind(&self) -> Option<Panel> {
        match self.page {
            6 => Some(Panel::Quota),
            9 => Some(Panel::Monitor),
            10 => Some(Panel::Devices),
            _ => None,
        }
    }
    pub(crate) fn expanded_width(&self) -> f32 {
        if self.dashboard_kind().is_some() {
            self.settings.dashboard.size[0].clamp(420, if self.side() { 480 } else { 600 }) as f32
        } else {
            PW
        }
    }
    pub fn monitor_details_active(&self) -> bool {
        self.open && self.dashboard_kind() == Some(Panel::Monitor)
    }
    pub(crate) fn open_dashboard(&mut self, kind: Panel) {
        let page = match kind {
            Panel::Quota => 6,
            Panel::Monitor => 9,
            Panel::Devices => 10,
        };
        self.dashboard.page = 0;
        self.show_page(page);
    }

    pub(crate) fn dashboard_ui(&mut self, c: &Card, env: &Env) {
        let kind = self.dashboard_kind().unwrap();
        let width = self.expanded_width();
        let height = self.expanded_height();
        let body_height = height - 12.;
        c.centered_text(
            width / 2.,
            21.,
            crate::app::PAGE_NAMES[self.page],
            12.,
            W::Semibold,
            theme::WHITE,
            H::Center,
        );
        let refresh = if kind == Panel::Quota {
            "刷新"
        } else {
            "自动"
        };
        c.centered_text(32., 21., refresh, 10., W::Regular, theme::GRAY, H::Center);
        if c.hit(12., 9., 40., 24., "dashboard-refresh").clicked() && kind == Panel::Quota {
            self.pending.push(Cmd::QuotaRefresh);
        }
        let settings = c
            .hit(width - 38., 7., 28., 28., "dashboard-settings")
            .on_hover_text("打开此栏目设置");
        crate::paint::gear(c, width - 24., 21., theme::GRAY);
        if settings.clicked() {
            self.settings_tab = if kind == Panel::Quota { 11 } else { 19 };
            self.open_settings();
        }
        match kind {
            Panel::Monitor => self.monitor_dashboard(c, width, body_height),
            Panel::Quota => self.quota_dashboard(c, width, body_height),
            Panel::Devices => self.device_dashboard(c, env, width, body_height),
        }
        self.dashboard_resize(c, width, height);
    }
    fn dashboard_resize(&mut self, c: &Card, w: f32, h: f32) {
        for (i, rect) in [
            c.rect(0., 35., 5., h - 42.),
            c.rect(w - 5., 35., 5., h - 42.),
        ]
        .into_iter()
        .enumerate()
        {
            let r =
                c.ui.interact(
                    rect,
                    egui::Id::new(("dashboard-resize", i)),
                    egui::Sense::drag(),
                )
                .on_hover_cursor(egui::CursorIcon::ResizeHorizontal);
            if r.drag_started() {
                if let Some(pos) = c.ui.input(|i| i.pointer.press_origin()) {
                    self.dashboard.resize = Some(([w as u16, h as u16], pos));
                }
            }
            if r.dragged() {
                if let (Some((size, start)), Some(pos)) =
                    (self.dashboard.resize, c.ui.ctx().pointer_interact_pos())
                {
                    let delta = (pos - start) / c.s;
                    self.settings.dashboard.size = [
                        if i != 2 {
                            (size[0] as f32 + delta.x * if i == 0 { -2. } else { 2. })
                                .clamp(420., if self.side() { 480. } else { 600. })
                                as u16
                        } else {
                            size[0]
                        },
                        250,
                    ];
                }
            }
            if r.drag_stopped() {
                self.settings.save();
                self.dashboard.resize = None;
            }
        }
    }
    fn monitor_dashboard(&mut self, c: &Card, w: f32, h: f32) {
        let r = &self.monitor.reading;
        let d = &self.settings.dashboard;
        let split = w * 0.60;
        let chart_w = split - 34.;
        let graph_bottom = h - 69.;
        c.centered_text(
            17.,
            48.,
            &format!("CPU {}", text(r.cpu, false)),
            13.,
            W::Bold,
            d.color(1),
            H::Left,
        );
        spark(
            c,
            Rect::from_min_size(Pos2::new(98., 40.), Vec2::new((split - 154.).max(30.), 14.)),
            self.monitor.history.iter().map(|r| r.cpu),
            Some(100.),
            d.color(1),
        );
        let count = r.cores.len();
        let gpu_x = split - 42.;
        let cores_w = chart_w - 47.;
        let graph_h = (graph_bottom - 65.).max(30.);
        if count == 0 {
            c.centered_text(
                20.,
                90.,
                "逻辑核心采样中 / 暂不可用",
                10.,
                W::Regular,
                theme::GRAY,
                H::Left,
            );
        } else {
            let bw = cores_w / count as f32;
            for (i, core) in r.cores.iter().enumerate() {
                let x = 17. + i as f32 * bw;
                let fill = graph_h * core.load as f32 / 100.;
                c.fill(x, 64., (bw - 2.).max(1.), graph_h, 2., theme::BTN);
                c.fill(
                    x,
                    64. + graph_h - fill,
                    (bw - 2.).max(1.),
                    fill.max(1.),
                    2.,
                    if core.efficient {
                        d.color(6)
                    } else {
                        d.color(0)
                    },
                );
                c.hit(x, 64., bw, graph_h, ("core", i))
                    .on_hover_text(format!(
                        "逻辑核心 {} · {:.1}%{}",
                        core.name,
                        core.load,
                        if core.efficient {
                            " · 能效类别"
                        } else {
                            ""
                        }
                    ));
            }
        }
        c.fill(gpu_x, 64., 26., graph_h, 5., theme::BTN);
        if let Some(v) = r.gpu {
            let fill = graph_h * v as f32 / 100.;
            c.fill(
                gpu_x,
                64. + graph_h - fill,
                26.,
                fill.max(1.),
                5.,
                d.color(3),
            );
        }
        c.centered_text(
            gpu_x + 13.,
            graph_bottom + 10.,
            &text(r.gpu, false),
            9.,
            W::Semibold,
            d.color(3),
            H::Center,
        );
        c.centered_text(
            gpu_x + 13.,
            49.,
            "GPU",
            9.,
            W::Semibold,
            d.color(3),
            H::Center,
        );
        let mem = r
            .memory_bytes
            .map(|(a, b)| {
                format!(
                    "内存 {:.1} / {:.1} GB",
                    a as f64 / 1073741824.,
                    b as f64 / 1073741824.
                )
            })
            .unwrap_or_else(|| "内存 —".into());
        c.centered_text(17., h - 44., &mem, 10., W::Semibold, d.color(2), H::Left);
        let x = 155.;
        c.fill(x, h - 47., (split - x - 15.).max(20.), 4., 2., theme::BTN);
        if let Some(v) = r.memory {
            c.fill(
                x,
                h - 47.,
                (split - x - 15.).max(20.) * v as f32 / 100.,
                4.,
                2.,
                d.color(2),
            );
        }
        for (i, metric) in [4, 5].into_iter().enumerate() {
            let x = 17. + i as f32 * (split - 20.) / 2.;
            c.centered_text(
                x,
                h - 25.,
                &format!(
                    "{} {}B/s",
                    if metric == 4 { "↓" } else { "↑" },
                    text(value(r, metric), true)
                ),
                9.,
                W::Semibold,
                d.color(metric),
                H::Left,
            );
            spark(
                c,
                Rect::from_min_size(
                    Pos2::new(x + 68., h - 33.),
                    Vec2::new(((split - 20.) / 2. - 78.).max(10.), 13.),
                ),
                self.monitor.history.iter().map(|r| value(r, metric)),
                None,
                d.color(metric),
            );
        }
        c.centered_text(split + 3., 48., "进程", 10., W::Bold, theme::WHITE, H::Left);
        for (i, label) in ["CPU", "内存"].into_iter().enumerate() {
            let x = w - 101. + i as f32 * 43.;
            if c.hit(x, 38., 40., 20., ("process-sort", i)).clicked() {
                self.dashboard.memory_sort = i == 1;
            }
            c.centered_text(
                x + 20.,
                48.,
                label,
                9.,
                W::Semibold,
                if self.dashboard.memory_sort == (i == 1) {
                    theme::CYAN
                } else {
                    theme::GRAY
                },
                H::Center,
            );
        }
        let mut rows: Vec<_> = r.processes.iter().collect();
        rows.sort_by(|a, b| {
            if self.dashboard.memory_sort {
                b.memory.cmp(&a.memory)
            } else {
                b.cpu.unwrap_or(-1.).total_cmp(&a.cpu.unwrap_or(-1.))
            }
        });
        let n = ((h - 99.) / 21.).floor() as usize;
        for (i, p) in rows.iter().take(n).enumerate() {
            let y = 72. + i as f32 * 21.;
            let icon = p.executable.as_ref().and_then(|path| self.dashboard.process_icons.get_program(c.ui.ctx(), path))
                .and_then(|preview| preview.image);
            if let Some(icon) = icon {
                c.painter.image(icon.id(), c.rect(split + 3., y - 8., 16., 16.),
                    Rect::from_min_max(Pos2::ZERO, Pos2::new(1., 1.)), C::WHITE);
            } else {
                // Protected processes or programs without an icon get a neutral app glyph.
                c.fill(split + 3., y - 8., 16., 16., 3., theme::BTN);
                c.fill(split + 6., y - 5., 10., 2., 0.5, theme::GRAY);
                c.fill(split + 6., y - 1., 4., 6., 0.5, theme::DIM);
                c.fill(split + 12., y - 1., 4., 6., 0.5, theme::DIM);
            }
            c.centered_text(
                split + 24.,
                y,
                &c.fit(&p.name, 9.5, W::Regular, (w - split - 125.).max(20.)),
                9.5,
                W::Regular,
                theme::WHITE,
                H::Left,
            );
            c.centered_text(
                w - 61.,
                y,
                &text(p.cpu, false),
                9.,
                W::Regular,
                theme::GRAY,
                H::Right,
            );
            c.centered_text(
                w - 17.,
                y,
                &format!("{:.0}M", p.memory as f64 / 1048576.),
                9.,
                W::Regular,
                theme::GRAY,
                H::Right,
            );
            c.hit(split, y - 9., w - split - 14., 18., ("process", p.pid))
                .on_hover_text(format!(
                    "{} · PID {}\nCPU 为全机占比；内存为工作集",
                    p.name, p.pid
                ));
        }
        if rows.is_empty() {
            c.centered_text(
                split + 3.,
                78.,
                "正在采样 / 无可读进程",
                9.,
                W::Regular,
                theme::GRAY,
                H::Left,
            );
        }
        c.centered_text(
            split + 3.,
            h - 25.,
            "展开 2 秒采样 · 最近 60 次",
            8.,
            W::Regular,
            theme::DIM,
            H::Left,
        );
    }
    fn device_dashboard(&mut self, c: &Card, env: &Env, w: f32, h: f32) {
        let mut rows = vec![("这台电脑".to_owned(), env.battery, None)];
        rows.extend(
            env.bt
                .iter()
                .map(|d| (d.name.clone(), d.battery, Some(d.kind))),
        );
        let columns = ((w - 28.) / 130.).floor().max(1.) as usize;
        let pages = rows.len().div_ceil(columns);
        self.dashboard.page = self.dashboard.page.min(pages.saturating_sub(1));
        for (i, (name, battery, kind)) in rows
            .iter()
            .skip(self.dashboard.page * columns)
            .take(columns)
            .enumerate()
        {
            let x = 14. + i as f32 * (w - 28.) / columns as f32;
            let cw = (w - 28.) / columns as f32;
            let cx = x + cw / 2.;
            device_ring(c, cx, 87., 22., *battery, *kind);
            c.centered_text(
                cx,
                123.,
                &c.fit(name, 11., W::Semibold, cw - 12.),
                11.,
                W::Semibold,
                theme::WHITE,
                H::Center,
            );
            c.centered_text(
                cx,
                144.,
                &battery
                    .map(|v| format!("{v}%"))
                    .unwrap_or_else(|| "未上报电量".into()),
                11.,
                W::Regular,
                theme::GRAY,
                H::Center,
            );
        }
        c.centered_text(
            16.,
            h - 31.,
            &c.fit(
                "Windows 蓝牙电量 · 约 60 秒更新；2.4G 接收器需厂商接口",
                8.,
                W::Regular,
                w - 110.,
            ),
            8.,
            W::Regular,
            theme::DIM,
            H::Left,
        );
        self.dashboard_pages(c, w, h, pages);
    }
    pub(crate) fn dashboard_pages(&mut self, c: &Card, w: f32, h: f32, pages: usize) {
        if pages > 1 {
            for (x, next, label) in [(w - 86., false, "‹"), (w - 29., true, "›")] {
                c.centered_text(x, h - 31., label, 16., W::Bold, theme::WHITE, H::Center);
                if c.hit(x - 12., h - 41., 24., 20., ("dashboard-pager", next))
                    .clicked()
                {
                    self.dashboard.page = if next {
                        (self.dashboard.page + 1) % pages
                    } else {
                        (self.dashboard.page + pages - 1) % pages
                    };
                }
            }
            c.centered_text(
                w - 57.,
                h - 31.,
                &format!("{}/{}", self.dashboard.page + 1, pages),
                9.,
                W::Regular,
                theme::GRAY,
                H::Center,
            );
        }
    }
    pub(crate) fn dashboard_settings(&mut self, ui: &mut egui::Ui) {
        use crate::extras::{setting_row, settings_card, settings_hint, settings_label};
        settings_card(ui, |ui| {
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(egui::RichText::new("实时监控与设备").size(15.0).strong());
                    settings_hint(ui, "在主岛查看资源曲线、进程排行与连接设备电量。");
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("打开岛内浏览面板").clicked() {
                        self.open_dashboard(Panel::Monitor);
                    }
                });
            });
            ui.separator();
            setting_row(
                ui,
                "连接设备电量",
                "在电脑电量旁显示鼠标、耳机等设备的圆环电量。",
                &mut self.settings.dashboard.device_rings,
            );
        });
        settings_label(ui, "收起状态 · 颜色与布局");
        settings_card(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label("配色预设");
                for (label, colors) in [
                    ("海洋", COLORS),
                    ("石墨", [[170, 179, 193]; 7]),
                    (
                        "暖色",
                        [
                            [245, 166, 93],
                            [245, 166, 93],
                            [235, 194, 105],
                            [231, 122, 153],
                            [246, 157, 105],
                            [223, 177, 122],
                            [235, 194, 105],
                        ],
                    ),
                ] {
                    if ui.button(label).clicked() {
                        self.settings.dashboard.colors = colors;
                        self.settings.dashboard.follow_theme = false;
                    }
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("恢复默认样式").clicked() {
                        let d = &mut self.settings.dashboard;
                        d.colors = COLORS;
                        d.follow_theme = true;
                        d.order = [0, 1, 2, 3, 4, 5];
                        d.numbers = [false, true, true, true, true, true];
                        d.size = [590, 250];
                    }
                });
            });
            setting_row(
                ui,
                "跟随主题",
                "点击色块即可自由选色，自动切换为自定义配色。拖动名称调整顺序。",
                &mut self.settings.dashboard.follow_theme,
            );
            ui.separator();
            let width = ui.available_width();
            let label_width = (width - 264.0 - ui.spacing().item_spacing.x * 3.0).max(100.0);
            let row = |ui: &mut egui::Ui, text: &str, width: f32| {
                ui.add_sized(
                    [width, 24.0],
                    egui::Label::new(egui::RichText::new(text).size(11.5))
                        .halign(egui::Align::LEFT),
                );
            };
            ui.horizontal(|ui| {
                row(ui, "组件 · 拖动排序", label_width);
                row(ui, "颜色", 64.0);
                row(ui, "显示数值", 88.0);
                row(ui, "显示组件", 88.0);
            });
            let mut order = self.settings.dashboard.order();
            let mut moved = None;
            for (pos, i) in order.into_iter().enumerate() {
                let drop = ui.dnd_drop_zone::<usize, _>(egui::Frame::NONE, |ui| {
                    ui.horizontal(|ui| {
                        ui.allocate_ui(egui::vec2(label_width, 30.0), |ui| {
                            ui.set_min_width(label_width);
                            ui.dnd_drag_source(egui::Id::new(("monitor-row", i)), pos, |ui| {
                                ui.horizontal(|ui| {
                                    let (r, _) = ui.allocate_exact_size(
                                        egui::vec2(14.0, 24.0),
                                        egui::Sense::hover(),
                                    );
                                    for x in [4.0, 9.0] {
                                        for y in [7.0, 12.0, 17.0] {
                                            ui.painter().circle_filled(
                                                r.min + egui::vec2(x, y),
                                                1.2,
                                                ui.visuals().weak_text_color(),
                                            );
                                        }
                                    }
                                    ui.label(egui::RichText::new(NAMES[i]).size(13.0));
                                });
                            });
                        });
                        ui.allocate_ui(egui::vec2(64.0, 30.0), |ui| {
                            ui.set_min_width(64.0);
                            self.settings.dashboard.palette(ui, i);
                        });
                        ui.allocate_ui(egui::vec2(88.0, 30.0), |ui| {
                            ui.set_min_width(88.0);
                            crate::extras::switch(ui, &mut self.settings.dashboard.numbers[i]);
                        });
                        ui.allocate_ui(egui::vec2(88.0, 30.0), |ui| {
                            ui.set_min_width(88.0);
                            let mut visible =
                                self.settings.dashboard.shown(self.settings.monitor, i);
                            if crate::extras::switch(ui, &mut visible) {
                                self.settings.dashboard.visible[i] = visible;
                                match i {
                                    1 => self.settings.monitor.cpu = visible,
                                    2 => self.settings.monitor.memory = visible,
                                    3 => self.settings.monitor.gpu = visible,
                                    4 | 5 => {
                                        self.settings.monitor.network =
                                            self.settings.dashboard.visible[4]
                                                || self.settings.dashboard.visible[5]
                                    }
                                    _ => {}
                                }
                            }
                        });
                    });
                });
                if let Some(from) = drop.1 {
                    moved = Some((*from, pos));
                }
            }
            if let Some((from, to)) = moved {
                let mut v = order.to_vec();
                let n = v.remove(from.min(5));
                v.insert(to.min(5), n);
                order.copy_from_slice(&v);
                self.settings.dashboard.order = order;
            }
            ui.separator();
            ui.horizontal(|ui| {
                ui.label("能效核心颜色");
                self.settings.dashboard.palette(ui, 6);
                settings_hint(ui, "仅在 Windows 报告不同效率类别时区分。");
            });
        });
        settings_label(ui, "数据说明");
        settings_card(ui, |ui| {
            settings_hint(ui, "CPU 与核心读数来自系统计数器；GPU 显示最繁忙引擎。进程排行仅在监控面板展开时采样，无权读取的系统进程不会列出。");
            settings_hint(ui, "设备电量取自 Windows 蓝牙驱动。未上报时显示未知；2.4G 接收器和厂商私有协议暂不通用支持。");
            settings_hint(ui, "拖动浏览面板左右边缘调整宽度，松开后自动保存；紧凑工具排在前面，详细面板排在后面，各组使用统一高度。");
        });
    }
}
pub(crate) fn device_ring(
    c: &Card,
    x: f32,
    y: f32,
    r: f32,
    battery: Option<u8>,
    kind: Option<crate::bt::DeviceKind>,
) {
    c.ring(x, y, r, 2., theme::BTN);
    if let Some(v) = battery {
        c.arc(
            x,
            y,
            r,
            -std::f32::consts::FRAC_PI_2,
            -std::f32::consts::FRAC_PI_2 + std::f32::consts::TAU * v as f32 / 100.,
            2.,
            if v <= 20 { theme::YELLOW } else { theme::CYAN },
        );
    }
    if let Some(kind) = kind {
        crate::extras::device_icon(c, kind, x - r * 0.6, y - r * 0.6, r * 1.2);
    } else {
        let col = theme::WHITE;
        c.stroke(
            x - r * 0.52,
            y - r * 0.40,
            r * 1.04,
            r * 0.72,
            r * 0.09,
            (r * 0.07).max(1.),
            col,
        );
        c.path(
            &[(x - r * 0.64, y + r * 0.48), (x + r * 0.64, y + r * 0.48)],
            (r * 0.09).max(1.),
            col,
        );
        c.path(
            &[(x - r * 0.52, y + r * 0.32), (x - r * 0.64, y + r * 0.48)],
            (r * 0.07).max(1.),
            col,
        );
        c.path(
            &[(x + r * 0.52, y + r * 0.32), (x + r * 0.64, y + r * 0.48)],
            (r * 0.07).max(1.),
            col,
        );
    }
}

#[cfg(test)]
mod palette_tests {
    use super::*;
    #[test]
    fn palette_click_applies_one_basic_color_and_persists() {
        let ctx = egui::Context::default();
        let mut config = Config::default();
        fn draw(ctx: &egui::Context, cfg: &mut Config, events: Vec<egui::Event>) -> Rect {
            let mut rect = Rect::NOTHING;
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(500., 500.))),
                    events,
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        rect = cfg.palette(ui, 1).rect;
                    });
                },
            );
            rect
        }
        let button = draw(&ctx, &mut config, vec![]).center();
        let event = |p, pressed| egui::Event::PointerButton {
            pos: p,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        };
        draw(
            &ctx,
            &mut config,
            vec![egui::Event::PointerMoved(button), event(button, true)],
        );
        draw(&ctx, &mut config, vec![event(button, false)]);
        draw(&ctx, &mut config, vec![]);
        let swatch = ctx
            .read_response(egui::Id::new(("monitor-swatch", 1usize, 7usize)))
            .unwrap()
            .rect
            .center();
        draw(
            &ctx,
            &mut config,
            vec![egui::Event::PointerMoved(swatch), event(swatch, true)],
        );
        draw(&ctx, &mut config, vec![event(swatch, false)]);
        assert_eq!(config.colors[1], SWATCHES[7].1);
        assert!(!config.follow_theme);
        for i in [0, 2, 3, 4, 5, 6] {
            assert_eq!(config.colors[i], COLORS[i]);
        }
        assert_eq!(
            serde_json::from_str::<Config>(&serde_json::to_string(&config).unwrap()).unwrap(),
            config
        );
    }
}
