//! Foreground application display rules, matched by full executable path.
use eframe::egui;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Action {
    #[default]
    Default,
    Hide,
    Collapse,
    Keep,
}
impl Action {
    fn label(self) -> &'static str {
        match self {
            Self::Default => "跟随全局",
            Self::Hide => "隐藏",
            Self::Collapse => "保持收起",
            Self::Keep => "保持显示",
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Rule {
    pub path: String,
    pub name: String,
    pub normal: Action,
    pub maximized: Action,
    pub fullscreen: Action,
}
pub fn resolve(rules: &[Rule], path: &str, maximized: bool, fullscreen: bool) -> Action {
    rules
        .iter()
        .find(|r| crate::avoidance::same_app(&r.path, path))
        .map(|r| {
            if fullscreen {
                r.fullscreen
            } else if maximized {
                r.maximized
            } else {
                r.normal
            }
        })
        .unwrap_or_default()
}
#[derive(Default)]
pub struct Editor {
    entries: Vec<crate::avoidance::Candidate>,
    rx: Option<std::sync::mpsc::Receiver<Result<Vec<crate::avoidance::Candidate>, String>>>,
    error: String,
    selected: usize,
}
impl Editor {
    pub fn ui(&mut self, ui: &mut egui::Ui, rules: &mut Vec<Rule>) {
        if let Some(rx) = &self.rx {
            if let Ok(result) = rx.try_recv() {
                self.rx = None;
                match result {
                    Ok(v) => {
                        self.entries = v;
                        self.selected = 0;
                    }
                    Err(e) => self.error = e,
                }
            }
        }
        crate::extras::settings_card(ui, |ui| {
            ui.label(
                "按前台应用设置岛的行为，优先于全局全屏隐藏和自动避让。切换到其他应用后恢复。",
            );
            ui.label("保持收起：仅保留胶囊，不因悬停展开。保持显示：允许正常显示和展开。规则仅影响岛所在屏幕。");
            ui.horizontal_wrapped(|ui| {
                if ui
                    .add_enabled(self.rx.is_none(), egui::Button::new("读取正在运行的应用"))
                    .clicked()
                {
                    let (tx, rx) = std::sync::mpsc::channel();
                    self.rx = Some(rx);
                    self.error.clear();
                    let ctx = ui.ctx().clone();
                    std::thread::spawn(move || {
                        #[cfg(windows)]
                        let result = crate::system::avoidance_windows();
                        #[cfg(not(windows))]
                        let result = Err("仅支持 Windows".into());
                        let _ = tx.send(result);
                        ctx.request_repaint();
                    });
                }
                if self.rx.is_some() {
                    ui.spinner();
                }
                if !self.entries.is_empty() {
                    egui::ComboBox::from_id_salt("rule-app-picker")
                        .selected_text(&self.entries[self.selected].name)
                        .show_ui(ui, |ui| {
                            for (i, e) in self.entries.iter().enumerate() {
                                ui.selectable_value(&mut self.selected, i, &e.name)
                                    .on_hover_text(&e.path);
                            }
                        });
                    let e = &self.entries[self.selected];
                    let exists = rules
                        .iter()
                        .any(|r| crate::avoidance::same_app(&r.path, &e.path));
                    if ui
                        .add_enabled(!exists, egui::Button::new("添加规则"))
                        .clicked()
                    {
                        rules.push(Rule {
                            path: e.path.clone(),
                            name: e.name.clone(),
                            normal: Action::Default,
                            maximized: Action::Default,
                            fullscreen: Action::Default,
                        });
                    }
                }
            });
            if !self.error.is_empty() {
                ui.label(&self.error);
            }
            if rules.is_empty() {
                ui.weak("尚无自定义规则。例如：游戏全屏时隐藏，播放器全屏时收起，办公软件最大化时保持显示。");
            }
        });
        let mut remove = None;
        for (i, r) in rules.iter_mut().enumerate() {
            ui.add_space(8.);
            crate::extras::settings_card(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.strong(&r.name).on_hover_text(&r.path);
                    if ui.small_button("移除规则").clicked() {
                        remove = Some(i);
                    }
                });
                ui.horizontal_wrapped(|ui| {
                    for (mode, action) in [
                        ("普通窗口", &mut r.normal),
                        ("最大化", &mut r.maximized),
                        ("全屏", &mut r.fullscreen),
                    ] {
                        ui.label(mode);
                        egui::ComboBox::from_id_salt(("app-rule", i, mode))
                            .width(105.)
                            .selected_text(action.label())
                            .show_ui(ui, |ui| {
                                for a in [
                                    Action::Default,
                                    Action::Hide,
                                    Action::Collapse,
                                    Action::Keep,
                                ] {
                                    ui.selectable_value(action, a, a.label());
                                }
                            });
                    }
                });
            });
        }
        if let Some(i) = remove {
            rules.remove(i);
        }
    }
}
#[cfg(windows)]
pub fn foreground(rules: &[Rule], display: Option<crate::display_layout::Bounds>) -> Action {
    use windows::{
        core::PWSTR,
        Win32::{
            Foundation::{CloseHandle, RECT},
            System::Threading::*,
            UI::WindowsAndMessaging::*,
        },
    };
    if rules.is_empty() {
        return Action::Default;
    }
    unsafe {
        let window = GetForegroundWindow();
        if window.0.is_null() {
            return Action::Default;
        }
        let mut pid = 0;
        GetWindowThreadProcessId(window, Some(&mut pid));
        if pid == 0 || pid == GetCurrentProcessId() {
            return Action::Default;
        }
        let mut rect = RECT::default();
        if GetWindowRect(window, &mut rect).is_err() {
            return Action::Default;
        }
        if let Some(d) = display {
            if rect.right <= d.left
                || rect.left >= d.right
                || rect.bottom <= d.top
                || rect.top >= d.bottom
            {
                return Action::Default;
            }
        }
        let Ok(handle) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) else {
            return Action::Default;
        };
        let mut path = vec![0u16; 32768];
        let mut len = path.len() as u32;
        let result = QueryFullProcessImageNameW(
            handle,
            PROCESS_NAME_WIN32,
            PWSTR(path.as_mut_ptr()),
            &mut len,
        );
        let _ = CloseHandle(handle);
        if result.is_err() {
            return Action::Default;
        }
        resolve(
            rules,
            &String::from_utf16_lossy(&path[..len as usize]),
            IsZoomed(window).as_bool(),
            crate::system::fullscreen().is_some(),
        )
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rules_fullscreen_priority_and_exact_path() {
        let r = Rule {
            path: "C:/Games/Game.exe".into(),
            name: "Game".into(),
            normal: Action::Keep,
            maximized: Action::Collapse,
            fullscreen: Action::Hide,
        };
        let rules = vec![r];
        assert_eq!(
            resolve(&rules, r"c:\games\GAME.EXE", true, true),
            Action::Hide
        );
        assert_eq!(
            resolve(&rules, r"c:\games\game.exe", true, false),
            Action::Collapse
        );
        assert_eq!(
            resolve(&rules, r"c:\games\game.exe", false, false),
            Action::Keep
        );
        assert_eq!(
            resolve(&rules, r"d:\games\game.exe", true, true),
            Action::Default
        );
        assert_eq!(
            serde_json::from_str::<Vec<Rule>>(&serde_json::to_string(&rules).unwrap()).unwrap(),
            rules
        );
    }
}
