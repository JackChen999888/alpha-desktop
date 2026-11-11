//! Explicit paste/hotkey translation. Never watches the user's clipboard.
use eframe::egui::{self, RichText};
use std::{sync::mpsc, time::Duration};
enum ResultEvent {
    Ocr(Result<String, String>),
    Translated(Result<String, String>),
}
#[derive(Default)]
pub struct QuickTranslate {
    pub editing: bool,
    capture: bool,
    focus: bool,
    source: String,
    result: String,
    status: String,
    rx: Option<mpsc::Receiver<ResultEvent>>,
    preview: Option<egui::TextureHandle>,
    language: usize,
}
impl QuickTranslate {
    pub fn launch(&mut self) {
        self.focus = true;
        self.capture = true;
    }
    fn translate(&mut self, ctx: &egui::Context) {
        if self.rx.is_some() || self.source.trim().is_empty() {
            return;
        }
        let target = ["自动中英互译", "简体中文", "英语", "日语", "韩语"][self.language];
        let prompt = format!("目标语言：{target}\n待翻译文本：\n{}", self.source);
        let (tx, rx) = mpsc::channel();
        self.rx = Some(rx);
        self.result.clear();
        self.status = "正在翻译…".into();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let _ = tx.send(ResultEvent::Translated(super::assistant::request(
                &prompt, true,
            )));
            ctx.request_repaint();
        });
    }
    fn clipboard(&mut self, ctx: &egui::Context) {
        if self.rx.is_some() {
            self.status = "正在处理，请稍后再粘贴".into();
            return;
        }
        let Ok(mut clip) = arboard::Clipboard::new() else {
            self.status = "剪贴板暂时被占用，请重试".into();
            return;
        };
        if let Ok(img) = clip.get_image() {
            if img.width > 8192 || img.height > 8192 || img.bytes.len() > 32 * 1024 * 1024 {
                self.status = "图片过大，请裁剪后再翻译（最大32 MB像素）".into();
                return;
            }
            let Some(image) = image::RgbaImage::from_raw(
                img.width as u32,
                img.height as u32,
                img.bytes.into_owned(),
            ) else {
                self.status = "图片格式无效".into();
                return;
            };
            let thumb = image::DynamicImage::ImageRgba8(image.clone())
                .thumbnail(360, 120)
                .to_rgba8();
            self.preview = Some(ctx.load_texture(
                "quick-translation-image",
                egui::ColorImage::from_rgba_unmultiplied(
                    [thumb.width() as usize, thumb.height() as usize],
                    thumb.as_raw(),
                ),
                Default::default(),
            ));
            self.source.clear();
            self.result.clear();
            self.status = "正在本机识别图片文字…".into();
            let (tx, rx) = mpsc::channel();
            self.rx = Some(rx);
            let ctx = ctx.clone();
            std::thread::spawn(move || {
                let _ = tx.send(ResultEvent::Ocr(recognize(image)));
                ctx.request_repaint();
            });
        } else if let Ok(text) = clip.get_text() {
            self.preview = None;
            self.source = text;
            if self.source.trim().is_empty() {
                self.status = "剪贴板没有文字。可直接粘贴文字或截图。".into();
            } else {
                self.translate(ctx);
            }
        } else {
            self.status = "剪贴板中没有可翻译的文字或图片".into();
        }
    }
    pub fn tick(&mut self, ctx: &egui::Context) {
        let received = self.rx.as_ref().map(|r| r.try_recv());
        match received {
            Some(Ok(event)) => {
                self.rx = None;
                match event {
                    ResultEvent::Ocr(Ok(text)) => {
                        self.source = text;
                        self.translate(ctx);
                    }
                    ResultEvent::Translated(Ok(text)) => {
                        self.result = text;
                        self.status = "翻译完成 · DeepSeek".into();
                    }
                    ResultEvent::Ocr(Err(e)) | ResultEvent::Translated(Err(e)) => self.status = e,
                }
            }
            Some(Err(mpsc::TryRecvError::Disconnected)) => {
                self.rx = None;
                self.status = "处理意外结束，请重试".into();
            }
            _ => {}
        }
        if self.capture {
            self.capture = false;
            self.clipboard(ctx);
        }
    }
    // Uses the same Card paint, fonts and skin palette as music, tasks and mail.
    pub fn island(&mut self, c: &crate::paint::Card, cmds: &mut Vec<crate::app::Cmd>) -> bool {
        use crate::{
            paint::H,
            theme::{self, size, W},
        };
        let ctx = c.ui.ctx().clone();
        if c.live
            && ctx.input(|i| {
                i.events.iter().any(|e| matches!(e, egui::Event::Paste(_)))
                    || i.modifiers.command && i.key_pressed(egui::Key::V)
            })
        {
            ctx.input_mut(|i| i.events.retain(|e| !matches!(e, egui::Event::Paste(_))));
            self.clipboard(&ctx);
        }
        let button = |x: f32, y: f32, w: f32, label: &str, id: &str, enabled: bool| {
            let hit = c.hit(x, y, w, 22., id);
            c.fill(
                x,
                y,
                w,
                22.,
                7.,
                if enabled && hit.hovered() {
                    theme::BTN_HOVER
                } else {
                    theme::BTN
                },
            );
            c.centered_text(
                x + w / 2.,
                y + 11.,
                label,
                size::SMALL,
                W::Semibold,
                if enabled { theme::WHITE } else { theme::DIM },
                H::Center,
            );
            enabled && hit.clicked()
        };
        if button(
            44.,
            34.,
            103.,
            "粘贴并翻译",
            "translate-paste",
            self.rx.is_none(),
        ) {
            self.clipboard(&ctx);
        }
        if button(
            153.,
            34.,
            51.,
            "翻译",
            "translate-run",
            self.rx.is_none() && !self.source.trim().is_empty(),
        ) {
            self.translate(&ctx);
        }
        let languages = ["中英互译", "译为中文", "译为英语", "译为日语", "译为韩语"];
        if button(
            267.,
            34.,
            107.,
            languages[self.language],
            "translate-language",
            true,
        ) {
            self.language = (self.language + 1) % languages.len();
        }
        c.fill(44., 62., 330., 66., 8., theme::CARD);
        c.text(
            54.,
            76.,
            if self.preview.is_some() {
                "原文 · 图片识别结果"
            } else {
                "原文"
            },
            size::TINY,
            W::Semibold,
            theme::GRAY,
            H::Left,
        );
        c.text(
            364.,
            76.,
            "Ctrl+V 文字 / 图片",
            size::TINY,
            W::Regular,
            theme::DIM,
            H::Right,
        );
        let bounds = c.rect(54., 82., 310., 39.);
        let mut input = egui::Ui::new(
            ctx.clone(),
            egui::Id::new(("alpha-translation-source", c.live)),
            egui::UiBuilder::new()
                .max_rect(bounds)
                .layer_id(c.painter.layer_id())
                .style(c.ui.style().clone()),
        );
        input.set_clip_rect(bounds.intersect(c.painter.clip_rect()));
        input.set_opacity(c.painter.opacity());
        if !c.live {
            input.disable();
        }
        input.style_mut().spacing.scroll.floating = false;
        input.spacing_mut().scroll.bar_width = 3. * c.s;
        input.visuals_mut().selection.bg_fill = theme::BLUE_DEEP;
        egui::ScrollArea::vertical()
            .id_salt("translation-source-scroll")
            .max_height(39. * c.s)
            .auto_shrink([false, false])
            .show(&mut input, |ui| {
                let response = ui.add(
                    egui::TextEdit::multiline(&mut self.source)
                        .frame(false)
                        .font(c.font(size::BODY, W::Regular))
                        .text_color(theme::WHITE)
                        .desired_rows(2)
                        .desired_width(f32::INFINITY)
                        .hint_text(RichText::new("输入需要翻译的文字…").color(theme::DIM)),
                );
                if c.live {
                    if self.focus {
                        response.request_focus();
                        self.focus = false;
                        cmds.push(crate::app::Cmd::Focus);
                    }
                    if response.clicked() {
                        cmds.push(crate::app::Cmd::Focus);
                    }
                    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                        response.surrender_focus();
                    }
                    self.editing = response.has_focus();
                }
            });
        c.fill(44., 135., 330., 65., 8., theme::CARD);
        c.text(
            54.,
            150.,
            "译文",
            size::TINY,
            W::Semibold,
            theme::CYAN,
            H::Left,
        );
        let copy = c.hit(307., 137., 57., 18., "translate-copy");
        c.text(
            364.,
            150.,
            "复制译文",
            size::TINY,
            W::Regular,
            if self.result.is_empty() {
                theme::DIM
            } else {
                theme::GRAY
            },
            H::Right,
        );
        if copy.clicked() && !self.result.is_empty() {
            ctx.copy_text(self.result.clone());
        }
        let bounds = c.rect(54., 158., 310., 35.);
        let mut result = egui::Ui::new(
            ctx.clone(),
            egui::Id::new(("alpha-translation-result", c.live)),
            egui::UiBuilder::new()
                .max_rect(bounds)
                .layer_id(c.painter.layer_id())
                .style(c.ui.style().clone()),
        );
        result.set_clip_rect(bounds.intersect(c.painter.clip_rect()));
        result.set_opacity(c.painter.opacity());
        if !c.live {
            result.disable();
        }
        result.style_mut().spacing.scroll.floating = false;
        result.spacing_mut().scroll.bar_width = 3. * c.s;
        egui::ScrollArea::vertical()
            .id_salt("translation-result-scroll")
            .max_height(35. * c.s)
            .auto_shrink([false, false])
            .show(&mut result, |ui| {
                ui.add(
                    egui::Label::new(
                        RichText::new(if self.result.is_empty() {
                            "译文会显示在这里"
                        } else {
                            &self.result
                        })
                        .font(c.font(size::BODY, W::Regular))
                        .color(if self.result.is_empty() {
                            theme::DIM
                        } else {
                            theme::WHITE
                        }),
                    )
                    .wrap()
                    .selectable(true),
                );
            });
        let status = if self.status.is_empty() {
            "Ctrl+Alt+T 直达 · 图片本机识别 · DeepSeek"
        } else {
            &self.status
        };
        c.text(
            44.,
            215.,
            &c.fit(status, size::TINY, W::Regular, 330.),
            size::TINY,
            W::Regular,
            theme::GRAY,
            H::Left,
        );
        c.hit(44., 204., 330., 17., "translation-status")
            .on_hover_text(status);
        if self.rx.is_some() {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
        false
    }
}
fn recognize(image: image::RgbaImage) -> Result<String, String> {
    use windows::{
        Graphics::Imaging::{BitmapAlphaMode, BitmapPixelFormat, SoftwareBitmap},
        Media::Ocr::OcrEngine,
        Storage::Streams::DataWriter,
    };
    let _ = unsafe {
        windows::Win32::System::Com::CoInitializeEx(
            None,
            windows::Win32::System::Com::COINIT_MULTITHREADED,
        )
    };
    let result = (|| {
        let engine = OcrEngine::TryCreateFromUserProfileLanguages()
            .map_err(|_| "Windows OCR 不可用，请安装系统语言的文字识别组件")?;
        let max = OcrEngine::MaxImageDimension().unwrap_or(4096);
        let image = if image.width() > max || image.height() > max {
            image::DynamicImage::ImageRgba8(image)
                .resize(max, max, image::imageops::FilterType::Triangle)
                .to_rgba8()
        } else {
            image
        };
        let (w, h) = image.dimensions();
        let mut rgba = image.into_raw();
        for p in rgba.chunks_exact_mut(4) {
            p.swap(0, 2);
            p[3] = 255;
        }
        let writer = DataWriter::new().map_err(|_| "图片缓冲初始化失败")?;
        writer.WriteBytes(&rgba).map_err(|_| "图片缓冲写入失败")?;
        let buffer = writer.DetachBuffer().map_err(|_| "图片缓冲失败")?;
        let bitmap = SoftwareBitmap::CreateWithAlpha(
            BitmapPixelFormat::Bgra8,
            w as i32,
            h as i32,
            BitmapAlphaMode::Ignore,
        )
        .map_err(|_| "图片转换失败")?;
        bitmap.CopyFromBuffer(&buffer).map_err(|_| "图片转换失败")?;
        let text = engine
            .RecognizeAsync(&bitmap)
            .and_then(|op| op.get())
            .and_then(|r| r.Text())
            .map_err(|_| "图片文字识别失败")?
            .to_string();
        if text.trim().is_empty() {
            Err("没有识别出文字，请使用清晰截图或手动输入".into())
        } else {
            Ok(text)
        }
    })();
    unsafe {
        windows::Win32::System::Com::CoUninitialize();
    }
    result
}
#[cfg(test)]
mod tests {
    #[test]
    #[ignore = "Requires Windows OCR and an explicit synthetic fixture"]
    fn windows_ocr_smoke() {
        let path = std::env::var("ALPHA_OCR_TEST_IMAGE").expect("synthetic fixture path");
        let image = image::open(path).unwrap().to_rgba8();
        let text = super::recognize(image).unwrap();
        assert!(
            text.to_lowercase().contains("alpha"),
            "OCR did not recognize synthetic fixture: {text}"
        );
    }
}
