use super::{
    agent,
    assistant,
    journal::{Journal, Task},
};
use eframe::egui::{self, Color32, RichText};
use std::{
    sync::mpsc,
    time::Duration,
};

pub struct Pro {
    pub open: bool,
    tab: usize,
    search: String,
    favorites: bool,
    journal: Journal,
    date: String,
    new_task: String,
    pub agents: agent::State,
    agent_watcher: Option<agent::Watcher>,
    task_search: String,
    task_filter: usize,
    key: String,
    key_saved: Result<bool, String>,
    key_notice: String,
    key_test_rx: Option<mpsc::Receiver<Result<String, String>>>,
    message: String,
    pub meter: super::audio::Meter,
    mail: super::email::Mail,
    requested_settings: Option<usize>,
    image_cache: std::collections::HashMap<String, Option<egui::TextureHandle>>,
    pub(crate) quick: super::translate::QuickTranslate,
}
impl Default for Pro {
    fn default() -> Self {
        Self {
            open: false,
            tab: 0,
            search: String::new(),
            favorites: false,
            journal: Journal::load(),
            date: chrono::Local::now().format("%Y-%m-%d").to_string(),
            new_task: String::new(),
            agents: Default::default(),
            agent_watcher: None,
            task_search: String::new(),
            task_filter: 0,
            key: String::new(),
            key_saved: assistant::saved_key_status(),
            key_notice: String::new(),
            key_test_rx: None,
            message: String::new(),
            meter: Default::default(),
            mail: Default::default(),
            image_cache: Default::default(),
            requested_settings: None,
            quick: Default::default(),
        }
    }
}
impl Pro {
    pub(crate) fn release_previews(&mut self){
        self.image_cache.clear();self.image_cache.shrink_to_fit();
    }
    pub fn open_center(&mut self) {
        self.open = true;
    }
    pub(crate) fn open_translation_settings(&mut self) {
        self.requested_settings = Some(17);
        self.open_center();
    }
    pub fn email_panel(&mut self, ui: &mut egui::Ui, provider: crate::mail::Provider) {
        let kind=if provider==crate::mail::Provider::Outlook {super::email::Kind::Outlook}else{super::email::Kind::Proton};
        self.mail.select_provider(kind.clone());
        self.mail.ui(ui,kind);
    }

    pub fn tick(&mut self) {
        self.mail.tick();
        match self.key_test_rx.as_ref().map(|rx| rx.try_recv()) {
            Some(Ok(result)) => {
                self.key_test_rx = None;
                self.key_notice = result.unwrap_or_else(|e| e);
                self.key_saved = assistant::saved_key_status();
            }
            Some(Err(mpsc::TryRecvError::Disconnected)) => {
                self.key_test_rx = None;
                self.key_notice = "连接测试意外结束，请重试。".into();
            }
            _ => {}
        }

        if !cfg!(test) && self.agent_watcher.is_none(){self.agent_watcher=Some(agent::Watcher::start(super::root().join("events")));}
        if let Some(state)=self.agent_watcher.as_mut().and_then(agent::Watcher::poll){self.agents=state;}
    }
    pub fn show(&mut self, ctx: &egui::Context, app: &mut crate::app::App) {
        self.tick();
        self.quick.tick(ctx);
        ctx.request_repaint_after(if self.key_test_rx.is_some(){Duration::from_millis(250)}else{Duration::from_secs(1)});
        if self.open {
            self.open = false;
            if let Some(tab) = self.requested_settings.take() {
                app.settings_tab = tab;
            }
            app.open_settings();
        }
        if !app.settings_open || app.settings_tab != 1 {
            self.meter
                .enabled
                .store(false, std::sync::atomic::Ordering::Relaxed);
        }
    }
    pub(crate) fn settings_section(
        &mut self,
        ui: &mut egui::Ui,
        app: &mut crate::app::App,
        section: usize,
    ) {
        let tab = match section {
            12 => 0,
            13 => 2,
            14 => 1,
            17 => 5,
            18 => 3,
            _ => return,
        };
        self.tab = tab;
        crate::extras::settings_card(ui, |ui| match tab {
            0 => self.clips(ui, app),
            1 => self.tasks(ui),
            2 => self.notes(ui, app),
            3 => self.translation(ui, app),
            5 => self.settings(ui),
            _ => {}
        });
        if self.tab != tab {
            app.settings_tab = match self.tab {
                0 => 12,
                2 => 13,
                1 => 14,
                5 => 17,
                3 => 18,
                _ => section,
            };
        }
        if !self.message.is_empty() {
            ui.add_space(12.);
            ui.horizontal_wrapped(|ui| {
                super::appearance::muted(ui, &self.message);
            });
        }
    }
    pub(crate) fn waveform_panel(&mut self, ui: &mut egui::Ui, app: &crate::app::App) {
        let response = ui.collapsing("实时音频波形", |ui| {
            crate::extras::settings_card(ui, |ui| {
                self.meter.draw(ui);
                ui.label(format!("{} — {}", app.media.title, app.media.artist));
                ui.small("默认音频输出的实时峰值；播放控制、进度和歌词在悬浮岛音乐栏目。");
            });
        });
        self.meter.enabled.store(
            response.body_returned.is_some(),
            std::sync::atomic::Ordering::Relaxed,
        );
    }
    fn clips(&mut self, ui: &mut egui::Ui, app: &mut crate::app::App) {
        ui.horizontal(|ui| {
            let mut on = app.clipboard.is_enabled();
            if ui.checkbox(&mut on, "记录剪贴板").changed() {
                app.settings.clipboard_history = on;
                app.settings.save();
                app.clipboard.set_enabled(on);
            }
            ui.checkbox(&mut self.favorites, "只看固定 / 收藏");
            if ui.button("清空普通历史").clicked() {
                app.clipboard.lock().clear_recent();
            }
        });
        ui.add(egui::TextEdit::singleline(&mut self.search).hint_text("搜索文字或备注"));
        ui.label("最近30条 + 最多20条收藏；单张图片≤10MB，收藏不会自动淘汰。");
        let items = app.clipboard.lock().items.clone();
        let search = self.search.to_lowercase();
        self.image_cache
            .retain(|name, _| items.iter().any(|c| c.image.as_ref() == Some(name)));
        if items.is_empty() {
            super::appearance::empty(
                ui,
                "还没有剪贴板记录",
                if app.clipboard.is_enabled() {
                    "复制一段文字或图片后，会出现在这里。"
                } else {
                    "开启上方“记录剪贴板”，开始保存最近 30 条。"
                },
            );
        }
        egui::ScrollArea::vertical().show(ui, |ui| {
            for (i, c) in items.iter().enumerate() {
                if self.favorites && !c.pinned {
                    continue;
                }
                if !search.is_empty()
                    && !format!("{} {}", c.text, c.note)
                        .to_lowercase()
                        .contains(&search)
                {
                    continue;
                }
                ui.push_id(i, |ui| {
                    egui::Frame::group(ui.style()).show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(&c.at);
                            if ui.button("复制").clicked() {
                                self.message = if crate::clipboard::copy_clip(c) {
                                    "已复制"
                                } else {
                                    "复制失败"
                                }
                                .into();
                            }
                            if ui.selectable_label(c.pinned, "固定 / 收藏").clicked() {
                                app.clipboard.lock().toggle_pin(i);
                            }
                            if ui.button("删除记录").clicked() {
                                app.clipboard.lock().remove(i);
                            }
                        });
                        if let Some(name) = &c.image {
                            let texture =
                                self.image_cache.entry(name.clone()).or_insert_with(|| {
                                    let path = crate::clipboard::image_path(name)?;
                                    let thumb =
                                        image::open(path).ok()?.thumbnail(340, 160).to_rgba8();
                                    Some(ui.ctx().load_texture(
                                        name,
                                        egui::ColorImage::from_rgba_unmultiplied(
                                            [thumb.width() as usize, thumb.height() as usize],
                                            thumb.as_raw(),
                                        ),
                                        Default::default(),
                                    ))
                                });
                            if let Some(texture) = texture {
                                ui.add(egui::Image::new(&*texture));
                            } else {
                                ui.label("图片文件不可用");
                            }
                        } else {
                            ui.label(c.text.chars().take(1500).collect::<String>());
                        }
                        let mut note = c.note.clone();
                        if ui
                            .add(egui::TextEdit::singleline(&mut note).hint_text("添加备注"))
                            .changed()
                        {
                            app.clipboard.lock().set_note(i, note);
                        }
                    });
                });
            }
        });
    }
    fn tasks(&mut self, ui: &mut egui::Ui) {
        let now=chrono::Utc::now().timestamp_millis();
        let counts = self.agents.counts_at(now);
        ui.columns(4, |cols| {
            for (i, (label, color)) in [
                ("运行中", Color32::from_rgb(100, 190, 245)),
                ("已完成", Color32::from_rgb(110, 210, 155)),
                ("等待操作", Color32::from_rgb(235, 186, 94)),
                ("失败", Color32::from_rgb(240, 124, 130)),
            ]
            .into_iter()
            .enumerate()
            {
                let ui = &mut cols[i];
                ui.label(
                    RichText::new(counts[i].to_string())
                        .size(27.)
                        .strong()
                        .color(color),
                );
                super::appearance::muted(ui, label);
            }
        });
        ui.add_space(10.);
        let unverified=self.agents.tasks.iter().filter(|t|t.effective_phase(now)==agent::Phase::Unknown).count();
        if unverified>0 {super::appearance::muted(ui,format!("{unverified} 个会话待核实 · 超过 30 分钟未更新的活动状态不计入运行中，也不推断已完成。"));}
        if self.agents.errors>0 {super::appearance::muted(ui,format!("有 {} 个状态文件暂时无法读取，其余任务仍正常显示。",self.agents.errors));}
        ui.separator();
        for client in ["Codex", "Claude Code"] {
            let latest = self
                .agents
                .tasks
                .iter()
                .filter(|e| e.agent == client)
                .max_by_key(|e| e.at);
            ui.horizontal_wrapped(|ui| {
                ui.strong(client);
                if let Some(e) = latest {
                    ui.label("● 已收到生命周期事件");
                    ui.small(
                        chrono::DateTime::from_timestamp_millis(e.at)
                            .map(|t| {
                                t.with_timezone(&chrono::Local)
                                    .format("最近 %m-%d %H:%M:%S")
                                    .to_string()
                            })
                            .unwrap_or_default(),
                    );
                } else {
                    super::appearance::muted(ui, "○ 尚未收到事件 · 需要客户端加载并审核 Hook");
                }
            });
        }
        if ui.button("安装 / 修复状态接入").clicked() {
            self.message = match agent::install_hooks() {
                Ok(s) => s,
                Err(e) => e,
            };
        }
        super::appearance::muted(ui,"Codex：设置 → Hooks 审核新增命令后，新任务开始上报。Claude Code：新会话加载。未收到事件不等于空闲，也不会根据进程猜测。");
        if self.agents.tasks.is_empty() {
            super::appearance::empty(
                ui,
                "等待第一次真实任务",
                "接通后，各项目和会话会分别显示在这里。",
            );
            return;
        }
        ui.add(egui::TextEdit::singleline(&mut self.task_search).hint_text("搜索项目、会话或客户端"));
        ui.horizontal_wrapped(|ui|{for(i,label)in ["全部","运行中","等待操作","已完成","失败","待核实"].iter().enumerate(){ui.selectable_value(&mut self.task_filter,i,*label);}});
        let query=self.task_search.trim().to_lowercase();
        let mut projects: std::collections::BTreeMap<&str, Vec<&agent::Event>> = Default::default();
        for event in &self.agents.tasks {
            let phase=event.effective_phase(now);
            let selected=match phase{agent::Phase::Running=>1,agent::Phase::Attention=>2,agent::Phase::Completed=>3,agent::Phase::Failed=>4,agent::Phase::Unknown=>5};
            if self.task_filter!=0 && self.task_filter!=selected {continue;}
            if !format!("{} {} {}",event.project,event.session,event.agent).to_lowercase().contains(&query){continue;}
            projects
                .entry(if event.project.is_empty() {
                    "未提供项目"
                } else {
                    &event.project
                })
                .or_default()
                .push(event);
        }
        if projects.is_empty(){super::appearance::muted(ui,"没有符合筛选条件的任务");}
        for (project, tasks) in projects {
            ui.add_space(14.);
            let name = std::path::Path::new(project)
                .file_name()
                .and_then(|p| p.to_str())
                .unwrap_or(project);
            ui.label(RichText::new(format!("{}  ·  {} 个会话", name, tasks.len())).strong())
                .on_hover_text(project);
            for t in tasks {
                egui::Frame::group(ui.style())
                    .inner_margin(12)
                    .show(ui, |ui| {
                        ui.set_min_width(ui.available_width());
                        let effective=t.effective_phase(now);
                        let stale = effective==agent::Phase::Unknown && t.phase!=agent::Phase::Unknown;
                        let color = match effective {
                            agent::Phase::Running => Color32::from_rgb(100, 190, 245),
                            agent::Phase::Completed => Color32::from_rgb(110, 210, 155),
                            agent::Phase::Attention => Color32::from_rgb(235, 186, 94),
                            agent::Phase::Failed => Color32::from_rgb(240, 124, 130),
                            agent::Phase::Unknown => Color32::from_gray(180),
                        };
                        ui.horizontal(|ui| {
                            ui.strong(&t.agent);
                            ui.label(
                                RichText::new(if stale {
                                    "● 长时间未更新 · 待核实"
                                } else {
                                    t.label()
                                })
                                .color(color),
                            );
                        });
                        super::appearance::muted(ui, format!("会话 {}", t.session));
                        if let Some(at)=chrono::DateTime::from_timestamp_millis(t.at){ui.small(format!("最后更新 {}",at.with_timezone(&chrono::Local).format("%m-%d %H:%M:%S")));}
                        ui.small(&t.detail);
                    });
            }
        }
    }
    fn notes(&mut self, ui: &mut egui::Ui, app: &mut crate::app::App) {
        let mut changed = false;
        ui.horizontal(|ui| {
            ui.label("日期");
            ui.text_edit_singleline(&mut self.date);
            if ui.button("今天").clicked() {
                self.date = chrono::Local::now().format("%Y-%m-%d").to_string();
            }
            changed |= ui
                .checkbox(&mut self.journal.carry, "自动结转未完成任务")
                .changed();
        });
        if chrono::NaiveDate::parse_from_str(&self.date, "%Y-%m-%d").is_err() {
            ui.label("请输入 YYYY-MM-DD");
            return;
        }
        ui.horizontal(|ui| {
            if ui.button("导出 Markdown").clicked() {
                self.message = match self.journal.export(&self.date) {
                    Ok(p) => format!("已导出：{}", p.display()),
                    Err(e) => e,
                };
            }
            if ui.button("导入主岛未完成待办").clicked() {
                let day = self.journal.day(&self.date);
                for t in &app.todos.items {
                    if !t.done_for(&self.date) && !day.tasks.iter().any(|v| v.text == t.text) {
                        day.tasks.push(Task {
                            id: format!(
                                "{}-{}",
                                chrono::Utc::now().timestamp_millis(),
                                day.tasks.len()
                            ),
                            text: t.text.clone(),
                            ..Default::default()
                        });
                    }
                }
                changed = true;
            }
        });
        changed |= !self.journal.days.contains_key(&self.date);
        let day = self.journal.day(&self.date);
        changed |= ui
            .add(
                egui::TextEdit::multiline(&mut day.text)
                    .desired_rows(8)
                    .desired_width(f32::INFINITY)
                    .hint_text("今天的想法、记录、复盘…"),
            )
            .changed();
        ui.horizontal(|ui| {
            ui.add(egui::TextEdit::singleline(&mut self.new_task).hint_text("新增当天任务"));
            if ui.button("添加").clicked() && !self.new_task.trim().is_empty() {
                day.tasks.push(Task {
                    id: format!(
                        "{}-{}",
                        chrono::Utc::now().timestamp_millis(),
                        day.tasks.len()
                    ),
                    text: std::mem::take(&mut self.new_task),
                    ..Default::default()
                });
                changed = true;
            }
        });
        let mut remove = None;
        egui::ScrollArea::vertical()
            .max_height(190.)
            .show(ui, |ui| {
                for (i, t) in day.tasks.iter_mut().enumerate() {
                    ui.horizontal(|ui| {
                        changed |= ui.checkbox(&mut t.done, "").changed();
                        changed |= ui.text_edit_singleline(&mut t.text).changed();
                        if !t.from.is_empty() {
                            ui.small(format!("来自 {}", t.from));
                        }
                        if ui.small_button("删除").clicked() {
                            remove = Some(i);
                        }
                    });
                }
            });
        if let Some(i) = remove {
            day.tasks.remove(i);
            changed = true;
        }
        if changed {
            self.message = match self.journal.save() {
                Ok(()) => "已自动保存".into(),
                Err(e) => e,
            };
        }
    }
    fn translation(&mut self, ui: &mut egui::Ui, app: &mut crate::app::App) {
        ui.heading("悬浮岛 · 翻译栏目");
        ui.label("通过主岛底部圆点或左右箭头切换到“翻译”，原文与译文直接显示在岛内。");
        ui.label("复制文字或截图后按 Ctrl+Alt+T，即可直达该栏目并翻译；也可在栏目中输入或粘贴。");
        if ui.button("打开悬浮岛翻译栏目").clicked() {
            app.open_translation(false);
            app.settings_open = false;
        }
        ui.label("图片在本机识别文字，仅将识别结果发送到 DeepSeek。");
        if ui.button("配置 DeepSeek 密钥").clicked() {
            self.tab = 5;
        }
    }
    fn settings(&mut self, ui: &mut egui::Ui) {
        ui.heading("DeepSeek API");
        ui.label("用于快捷翻译；密钥保存在 Windows 凭据管理器。");
        match &self.key_saved {
            Ok(true) => {
                ui.colored_label(
                    Color32::from_rgb(110, 210, 155),
                    "● 已保存密钥 · 输入框留空表示继续使用已保存密钥",
                );
            }
            Ok(false) => {
                ui.label("○ 尚未保存密钥");
            }
            Err(e) => {
                ui.colored_label(Color32::from_rgb(240, 150, 120), e);
            }
        }
        ui.add(
            egui::TextEdit::singleline(&mut self.key)
                .password(true)
                .desired_width(f32::INFINITY)
                .hint_text(if self.key_saved == Ok(true) {
                    "输入新密钥以替换；留空不会清除现有密钥"
                } else {
                    "粘贴以 sk- 开头的 DeepSeek API 密钥"
                }),
        );
        let busy = self.key_test_rx.is_some();
        ui.horizontal_wrapped(|ui| {
            if ui
                .add_enabled(!busy, egui::Button::new("保存密钥"))
                .clicked()
            {
                if self.key.trim().is_empty() {
                    self.key_saved = assistant::saved_key_status();
                    self.key_notice = if self.key_saved == Ok(true) {
                        "现有密钥已安全保存，无需重复粘贴。".into()
                    } else {
                        "请先在输入框粘贴密钥。".into()
                    };
                } else {
                    match assistant::save_key(&self.key) {
                        Ok(()) => {
                            self.key.clear();
                            self.key_saved = Ok(true);
                            self.key_notice = "已保存，并已从 Windows 回读确认。".into();
                        }
                        Err(e) => self.key_notice = e,
                    }
                }
            }
            let test_label = if self.key.trim().is_empty() {
                "测试连接"
            } else {
                "保存并测试"
            };
            if ui
                .add_enabled(!busy, egui::Button::new(test_label))
                .clicked()
            {
                let saved = if self.key.trim().is_empty() {
                    Ok(())
                } else {
                    assistant::save_key(&self.key)
                };
                match saved {
                    Err(e) => self.key_notice = e,
                    Ok(()) => {
                        self.key.clear();
                        self.key_saved = assistant::saved_key_status();
                        self.key_notice = "正在验证 DeepSeek 官方接口…".into();
                        let (tx, rx) = mpsc::channel();
                        self.key_test_rx = Some(rx);
                        let ctx = ui.ctx().clone();
                        std::thread::spawn(move || {
                            let _ = tx.send(assistant::test_connection());
                            ctx.request_repaint();
                        });
                    }
                }
            }
            if ui
                .add_enabled(
                    !busy && self.key_saved == Ok(true),
                    egui::Button::new("移除密钥"),
                )
                .clicked()
            {
                self.key_notice = match assistant::credential().and_then(|c| {
                    c.delete_credential()
                        .map_err(|_| "移除失败，请重试。".into())
                }) {
                    Ok(()) => "已移除密钥。".into(),
                    Err(e) => e,
                };
                self.key_saved = assistant::saved_key_status();
            }
            if busy {
                ui.spinner();
            }
        });
        if !self.key_notice.is_empty() {
            ui.label(&self.key_notice);
        }
        ui.small("测试仅查询官方模型列表，验证认证与连接；不发送笔记、剪贴板或翻译内容。");
    }
}
impl crate::app::App {
    pub fn pro_ui(&mut self, ctx: &egui::Context) {
        if let Some(mut pro) = self.pro.take() {
            pro.show(ctx, self);
            self.pro = Some(pro);
        }
    }
}
