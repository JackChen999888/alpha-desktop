use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
pub struct Meter {
    pub enabled: Arc<AtomicBool>,
    pub values: Arc<Mutex<Vec<f32>>>,
    stop: Arc<AtomicBool>,
}
impl Default for Meter {
    fn default() -> Self {
        let enabled = Arc::new(AtomicBool::new(false));
        let values = Arc::new(Mutex::new(vec![0.0; 48]));
        let stop = Arc::new(AtomicBool::new(false));
        #[cfg(windows)]
        {
            let on = enabled.clone();
            let output = values.clone();
            let stop = stop.clone();
            std::thread::spawn(move || {
                use windows::Win32::{
                    Media::Audio::{Endpoints::IAudioMeterInformation, *},
                    System::Com::*,
                };
                unsafe {
                    let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
                }
                let mut meter: Option<IAudioMeterInformation> = None;
                let mut sampled = 0;
                while !stop.load(Ordering::Relaxed) {
                    std::thread::sleep(std::time::Duration::from_millis(100));
                    if !on.load(Ordering::Relaxed) {
                        meter = None;
                        continue;
                    }
                    if meter.is_none() || sampled % 100 == 0 {
                        meter = unsafe {
                            CoCreateInstance::<_, IMMDeviceEnumerator>(
                                &MMDeviceEnumerator,
                                None,
                                CLSCTX_ALL,
                            )
                            .ok()
                            .and_then(|e| e.GetDefaultAudioEndpoint(eRender, eMultimedia).ok())
                            .and_then(|d| d.Activate(CLSCTX_ALL, None).ok())
                        };
                    }
                    let value = unsafe { meter.as_ref().and_then(|m| m.GetPeakValue().ok()) }
                        .unwrap_or(0.0)
                        .clamp(0.0, 1.0);
                    sampled += 1;
                    if let Ok(mut v) = output.lock() {
                        v.remove(0);
                        v.push(value);
                    }
                }
                drop(meter);
                unsafe {
                    CoUninitialize();
                }
            });
        }
        Self {
            enabled,
            values,
            stop,
        }
    }
}
impl Drop for Meter {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}
impl Meter {
    pub fn draw(&self, ui: &mut eframe::egui::Ui) {
        use eframe::egui::*;
        self.enabled.store(true, Ordering::Relaxed);
        let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 46.0), Sense::hover());
        if let Ok(v) = self.values.lock() {
            let w = rect.width() / v.len() as f32;
            for (i, a) in v.iter().enumerate() {
                let h = (a.sqrt() * rect.height()).max(2.0);
                ui.painter().rect_filled(
                    Rect::from_center_size(
                        pos2(rect.left() + i as f32 * w + w / 2.0, rect.center().y),
                        vec2((w - 3.0).max(1.0), h),
                    ),
                    2.0,
                    Color32::from_rgb(102, 205, 220),
                );
            }
        }
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(100));
    }
}
