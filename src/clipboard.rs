//! 剪贴板记录：最近复制的文字，在置物架页的「剪贴板」里查看、再次复制、固定。
//!
//! - 记录文字与有大小限制的 PNG 图片；文件列表不记录。
//! - 最近记录与固定条目都保存在本机 `clipboard.json`。
//! - 密码管理器等标记为「不进剪贴板历史」的内容一律跳过
//!   （`ExcludeClipboardContentFromMonitorProcessing` / `CanIncludeInClipboardHistory = 0`）。
//! - Windows 上用 `AddClipboardFormatListener` 等系统通知，没有轮询；
//!   监听线程只在剪贴板真正变化时醒来。

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

/// 未固定条目的上限；固定的不占这个名额。
pub const MAX_RECENT: usize = 30;
/// 单条最长字符数，超过的不记录（大段日志、整份文件内容）。
pub const MAX_CHARS: usize = 20_000;
pub const MAX_PINNED: usize = 20;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Clip {
    pub text: String,
    /// 本地时间 `HH:MM`。
    pub at: String,
    pub pinned: bool,
    pub image: Option<String>,
    pub note: String,
}

#[derive(Debug, Default)]
pub struct History {
    pub items: Vec<Clip>,
    /// 每次变化 +1，界面据此判断是否需要重绘。
    pub version: u64,
    path: Option<std::path::PathBuf>,
}

impl History {
    /// 新内容放最前；同样的文字只留一条（移到最前、保留固定状态）。
    pub fn push(&mut self, text: &str, at: &str) -> bool {
        if text.trim().is_empty() || text.chars().count() > MAX_CHARS { return false; }
        let (pinned,note) = match self.items.iter().position(|c| c.text == text && c.image.is_none()) {
            Some(0) => return false,
            Some(i) => {let c=self.items.remove(i);(c.pinned,c.note)},
            None => (false,String::new()),
        };
        self.items.insert(0, Clip { text: text.to_string(), at: at.to_string(), pinned, image:None, note });
        let mut recent = 0;
        self.items.retain(|c| c.pinned || { recent += 1; recent <= MAX_RECENT });
        self.changed();
        true
    }

    pub fn remove(&mut self, index: usize) {
        if index < self.items.len() {
            self.items.remove(index);
            self.changed();
        }
    }

    /// 清空未固定的条目，固定的保留。
    pub fn clear_recent(&mut self) {
        self.items.retain(|c| c.pinned);
        self.changed();
    }

    pub fn toggle_pin(&mut self, index: usize) -> bool {
        let pinned_count = self.items.iter().filter(|c| c.pinned).count();
        let Some(item) = self.items.get_mut(index) else { return false; };
        if !item.pinned && pinned_count >= MAX_PINNED { return false; }
        item.pinned = !item.pinned;
        let mut recent=0;self.items.retain(|c|c.pinned||{recent+=1;recent<=MAX_RECENT});
        self.changed();
        true
    }

    fn to_json(&self) -> Vec<u8> {
        let rows: Vec<_> = self.items.iter().map(|c| serde_json::json!({"pinned": c.pinned,"text": c.text, "at": c.at,"image":c.image,"note":c.note})).collect();
        serde_json::to_vec(&rows).unwrap_or_default()
    }

    pub fn from_json(raw: &[u8]) -> Self {
        let rows: Vec<serde_json::Value> = serde_json::from_slice(raw).unwrap_or_default();
        let items = rows.iter().filter_map(|row| Some(Clip {
            text: row.get("text")?.as_str()?.to_string(),
            at: row.get("at").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            pinned: row.get("pinned").and_then(|v| v.as_bool()).unwrap_or(true),
            image:row.get("image").and_then(|v|v.as_str()).filter(|s|valid_image_name(s)).map(str::to_string),
            note:row.get("note").and_then(|v|v.as_str()).unwrap_or("").chars().take(500).collect(),
        })).filter(|c| !c.text.is_empty()).take(MAX_PINNED + MAX_RECENT).collect();
        Self { items, ..Self::default() }
    }

    fn path() -> std::path::PathBuf { crate::model::config_dir().join("clipboard.json") }

    pub fn load() -> Self {
        Self::load_from(Self::path())
    }

    fn load_from(path: std::path::PathBuf) -> Self {
        let mut history = std::fs::read(&path).map(|raw| Self::from_json(&raw)).unwrap_or_default();
        history.path = Some(path);
        history
    }

    fn changed(&mut self) {
        self.version += 1;
        if let Some(path) = &self.path {
            if let Some(parent) = path.parent() { let _ = std::fs::create_dir_all(parent); }
            let _ = crate::model::atomic_write(path, &self.to_json());
            self.cleanup_images();
        }
    }

    pub fn set_note(&mut self,index:usize,note:String){if let Some(c)=self.items.get_mut(index){c.note=note.chars().take(500).collect();self.changed();}}
    fn images_dir(&self)->std::path::PathBuf{self.path.as_ref().and_then(|p|p.parent()).map(std::path::Path::to_path_buf).unwrap_or_else(crate::model::config_dir).join("clipboard-images")}
    pub fn push_png(&mut self,png:&[u8],width:usize,height:usize)->Result<bool,String>{
        if png.len()>10*1024*1024{return Err("图片超过10MB，未记录".into());}
        let digest=ring::digest::digest(&ring::digest::SHA256,png);
        let name=format!("{}.png",digest.as_ref().iter().map(|v|format!("{v:02x}")).collect::<String>());
        let (pinned,note)=if let Some(i)=self.items.iter().position(|c|c.image.as_ref()==Some(&name)){if i==0{return Ok(false);}let c=self.items.remove(i);(c.pinned,c.note)}else{(false,String::new())};
        let path=self.images_dir().join(&name);
        std::fs::create_dir_all(path.parent().unwrap()).map_err(|e|e.to_string())?;
        crate::model::atomic_write(&path,png).map_err(|e|e.to_string())?;
        self.items.insert(0,Clip{text:format!("图片 {width} × {height}"),at:chrono::Local::now().format("%H:%M").to_string(),pinned,image:Some(name),note});
        let mut n=0;self.items.retain(|c|c.pinned||{n+=1;n<=MAX_RECENT});self.changed();Ok(true)
    }
    fn cleanup_images(&self){
        if let Ok(files)=std::fs::read_dir(self.images_dir()){
            for f in files.flatten(){let name=f.file_name().to_string_lossy().to_string();if valid_image_name(&name)&&!self.items.iter().any(|c|c.image.as_ref()==Some(&name)){let _=std::fs::remove_file(f.path());}}
        }
    }
}

fn valid_image_name(s:&str)->bool{s.strip_suffix(".png").is_some_and(|s|s.len()==64&&s.bytes().all(|b|b.is_ascii_hexdigit()))}
pub fn image_path(name:&str)->Option<std::path::PathBuf>{valid_image_name(name).then(||crate::model::config_dir().join("clipboard-images").join(name))}
#[cfg(windows)]
pub fn copy_clip(clip:&Clip)->bool {
    if let Some(name)=&clip.image {
        let Some(path)=image_path(name)else{return false};let Ok(img)=image::open(path)else{return false};let img=img.to_rgba8();
        arboard::Clipboard::new().and_then(|mut c|c.set_image(arboard::ImageData{width:img.width()as usize,height:img.height()as usize,bytes:std::borrow::Cow::Owned(img.into_raw())})).is_ok()
    }else{copy_history(&clip.text)}
}

/// 界面与监听线程共用。
#[derive(Clone)]
pub struct Shared {
    pub history: Arc<Mutex<History>>,
    pub enabled: Arc<AtomicBool>,
}

impl Shared {
    pub fn new(enabled: bool) -> Self {
        Self { history: Arc::new(Mutex::new(History::load())), enabled: Arc::new(AtomicBool::new(enabled)) }
    }
    pub fn set_enabled(&self, on: bool) { self.enabled.store(on, Ordering::Relaxed); }
    pub fn is_enabled(&self) -> bool { self.enabled.load(Ordering::Relaxed) }
    pub fn lock(&self) -> std::sync::MutexGuard<'_, History> {
        self.history.lock().unwrap_or_else(|e| e.into_inner())
    }
    fn record_update(&self, text: &str, from_history: bool) -> bool {
        !from_history && self.record(text)
    }
    pub fn record(&self, text: &str) -> bool {
        if !self.is_enabled() { return false; }
        let at = chrono::Local::now().format("%H:%M").to_string();
        self.lock().push(text, &at)
    }
}

impl Default for Shared {
    fn default() -> Self { Self::new(true) }
}

/// 列表里的一行预览：换行与连续空白压成一个空格。
pub fn preview(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

// ---------------------------------------------------------------- Windows 监听

#[cfg(windows)]
pub fn watch(shared: Shared, repaint: impl Fn() + Send + 'static) {
    std::thread::Builder::new().name("daozai-clipboard".into()).spawn(move || {
        // 启动失败（极少见）时剪贴板页只是没有新记录，不影响其他功能。
        let _ = win::run(shared, Box::new(repaint));
    }).ok();
}

#[cfg(not(windows))]
pub fn watch(_shared: Shared, _repaint: impl Fn() + Send + 'static) {}

#[cfg(windows)]
const HISTORY_COPY_FORMAT: &str = "Daozai.ClipboardHistoryCopy";

#[cfg(windows)]
pub fn copy_history(text: &str) -> bool {
    use clipboard_win::{Clipboard, raw, register_format};
    let Some(format) = register_format(HISTORY_COPY_FORMAT) else { return false };
    let Ok(_clipboard) = Clipboard::new_attempts(10) else { return false };
    raw::set_string(text).is_ok() && raw::set_without_clear(format.get(), &[1]).is_ok()
}

#[cfg(windows)]
mod win {
    use super::Shared;
    use std::cell::RefCell;
    use windows::core::{w, PCWSTR};
    use windows::Win32::Foundation::{HGLOBAL, HWND, LPARAM, LRESULT, WPARAM};
    use windows::Win32::System::DataExchange::*;
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::System::Memory::{GlobalLock, GlobalSize, GlobalUnlock};
    use windows::Win32::System::Ole::CF_UNICODETEXT;
    use windows::Win32::UI::WindowsAndMessaging::*;

    const WM_CLIPBOARDUPDATE: u32 = 0x031D;

    struct State { shared: Shared, repaint: Box<dyn Fn() + Send> }
    thread_local! { static STATE: RefCell<Option<State>> = const { RefCell::new(None) }; }

    unsafe extern "system" fn proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
        if msg == WM_CLIPBOARDUPDATE {
            STATE.with(|state| {
                if let Some(state) = state.borrow().as_ref() {
                    if state.shared.is_enabled() {
                        if let Some((text, from_history)) = read_text(hwnd) {
                            if state.shared.record_update(&text, from_history) { (state.repaint)(); }
                        }else if image_allowed(hwnd) {
                            if let Ok(mut board)=arboard::Clipboard::new(){if let Ok(img)=board.get_image(){
                                if img.width<=8192&&img.height<=8192&&img.bytes.len()<=32*1024*1024 {
                                    let mut bytes=std::io::Cursor::new(Vec::new());
                                    if let Some(rgba)=image::RgbaImage::from_raw(img.width as u32,img.height as u32,img.bytes.into_owned()){
                                        if image::DynamicImage::ImageRgba8(rgba).write_to(&mut bytes,image::ImageFormat::Png).is_ok() && state.shared.lock().push_png(bytes.get_ref(),img.width,img.height).unwrap_or(false){(state.repaint)();}
                                    }
                                }
                            }}
                        }
                    }
                }
            });
            return LRESULT(0);
        }
        DefWindowProcW(hwnd, msg, wp, lp)
    }

    /// 密码管理器、远程桌面等会给敏感内容加上这些格式，表示不要进入剪贴板历史。
    unsafe fn excluded() -> bool {
        let exclude = RegisterClipboardFormatW(w!("ExcludeClipboardContentFromMonitorProcessing"));
        let ignore = RegisterClipboardFormatW(w!("Clipboard Viewer Ignore"));
        if (exclude != 0 && IsClipboardFormatAvailable(exclude).is_ok()) || (ignore != 0 && IsClipboardFormatAvailable(ignore).is_ok()) {
            return true;
        }
        // 值为 DWORD 0 表示不允许进入历史。
        let history = RegisterClipboardFormatW(w!("CanIncludeInClipboardHistory"));
        if history == 0 || IsClipboardFormatAvailable(history).is_err() { return false; }
        let Ok(handle) = GetClipboardData(history) else { return true; };
        let memory = HGLOBAL(handle.0);
        let ptr = GlobalLock(memory) as *const u32;
        if ptr.is_null() { return true; }
        let allowed = GlobalSize(memory) >= 4 && *ptr != 0;
        let _ = GlobalUnlock(memory);
        !allowed
    }

    unsafe fn image_allowed(hwnd:HWND)->bool {if OpenClipboard(hwnd).is_err(){return false;}let allowed=!excluded();let _=CloseClipboard();allowed}

    unsafe fn read_text(hwnd: HWND) -> Option<(String, bool)> {
        if IsClipboardFormatAvailable(CF_UNICODETEXT.0 as u32).is_err() { return None; }
        // 其他程序可能正占用剪贴板，稍等重试几次。
        let mut opened = false;
        for _ in 0..5 {
            if OpenClipboard(hwnd).is_ok() { opened = true; break; }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        if !opened { return None; }
        let text = (|| {
            if excluded() { return None; }
            let handle = GetClipboardData(CF_UNICODETEXT.0 as u32).ok()?;
            let memory = HGLOBAL(handle.0);
            let ptr = GlobalLock(memory) as *const u16;
            if ptr.is_null() { return None; }
            let max = (GlobalSize(memory) / 2).min(super::MAX_CHARS * 2 + 1);
            let mut len = 0;
            while len < max && *ptr.add(len) != 0 { len += 1; }
            let text = (len <= super::MAX_CHARS * 2).then(|| String::from_utf16_lossy(std::slice::from_raw_parts(ptr, len)));
            let _ = GlobalUnlock(memory);
            let from_history = clipboard_win::register_format(super::HISTORY_COPY_FORMAT)
                .is_some_and(|format| IsClipboardFormatAvailable(format.get()).is_ok());
            text.map(|text| (text, from_history))
        })();
        let _ = CloseClipboard();
        text
    }

    pub fn run(shared: Shared, repaint: Box<dyn Fn() + Send>) -> windows::core::Result<()> {
        unsafe {
            let instance = GetModuleHandleW(None)?;
            let class = w!("DaozaiClipboardWatcher");
            let wc = WNDCLASSW { lpfnWndProc: Some(proc), hInstance: instance.into(), lpszClassName: class, ..Default::default() };
            RegisterClassW(&wc);
            let hwnd = CreateWindowExW(WINDOW_EX_STYLE(0), class, PCWSTR::null(), WINDOW_STYLE(0), 0, 0, 0, 0, HWND_MESSAGE, None, instance, None)?;
            STATE.with(|state| *state.borrow_mut() = Some(State { shared, repaint }));
            AddClipboardFormatListener(hwnd)?;
            let mut msg = MSG::default();
            while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
            let _ = RemoveClipboardFormatListener(hwnd);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn image_paths_reject_unicode_and_traversal(){let unicode=format!("{}a.png","中".repeat(21));assert!(!valid_image_name(&unicode));assert!(!valid_image_name("../outside.png"));assert!(valid_image_name(&format!("{}.png","a".repeat(64))));}
    #[test]
    fn image_round_trip_notes_and_eviction(){
        let dir=std::env::temp_dir().join(format!("island-image-test-{}",std::process::id()));std::fs::create_dir_all(&dir).unwrap();
        let mut h=History::load_from(dir.join("history.json"));let img=image::RgbaImage::from_pixel(2,2,image::Rgba([1,2,3,255]));let mut bytes=std::io::Cursor::new(Vec::new());img.write_to(&mut bytes,image::ImageFormat::Png).unwrap();
        h.push_png(bytes.get_ref(),2,2).unwrap();h.set_note(0,"测试图片".into());h.toggle_pin(0);h.push("next","10:00");h.push_png(bytes.get_ref(),2,2).unwrap();
        assert!(h.items[0].pinned);assert_eq!(h.items[0].note,"测试图片");assert_eq!(History::load_from(dir.join("history.json")).items,h.items);
        let path=h.images_dir().join(h.items[0].image.as_ref().unwrap());assert!(path.exists());h.remove(0);assert!(!path.exists());
        let _=std::fs::remove_file(dir.join("history.json"));let _=std::fs::remove_dir(dir.join("clipboard-images"));let _=std::fs::remove_dir(dir);
    }
    #[test]fn unpin_keeps_recent_limit(){let mut h=History::default();h.push("pinned","10:00");h.toggle_pin(0);for n in 0..MAX_RECENT{h.push(&format!("{n}"),"10:00");}h.toggle_pin(h.items.len()-1);assert_eq!(h.items.len(),MAX_RECENT);}

    #[test]
    fn history_copy_preserves_order_but_external_copy_promotes() {
        let shared = Shared { history: Default::default(), enabled: Arc::new(AtomicBool::new(true)) };
        shared.lock().items.clear();
        shared.record("较早的一条");
        shared.record("最近的一条");
        let before = shared.lock().items.clone();
        assert!(!shared.record_update("较早的一条", true));
        assert_eq!(shared.lock().items, before);
        assert!(shared.record_update("较早的一条", false));
        assert_eq!(shared.lock().items[0].text, "较早的一条");
    }
    #[test]
    fn history_dedupes_caps_and_keeps_pins() {
        let mut h = History::default();
        assert!(!h.push("   \n", "09:00"), "空白不记录");
        assert!(h.push("a", "09:00") && h.push("b", "09:01"));
        assert!(!h.push("b", "09:02"), "与最新一条相同不重复记录");
        assert!(h.push("a", "09:03"));
        assert_eq!(h.items.iter().map(|c| c.text.as_str()).collect::<Vec<_>>(), ["a", "b"]);
        assert_eq!(h.items[0].at, "09:03");
        h.items[1].pinned = true; // b
        for i in 0..(MAX_RECENT + 5) { h.push(&format!("n{i}"), "10:00"); }
        assert_eq!(h.items.iter().filter(|c| !c.pinned).count(), MAX_RECENT);
        assert!(h.items.iter().any(|c| c.text == "b" && c.pinned), "固定的不会被挤掉");
        h.clear_recent();
        assert_eq!(h.items.len(), 1);
        assert!(!h.push(&"x".repeat(MAX_CHARS + 1), "10:00"), "超长内容不记录");
    }

    #[test]
    fn reopening_restores_history_and_respects_manual_cleanup() {
        let path = std::env::temp_dir().join(format!("daozai-clip-{}.json", std::process::id()));
        std::fs::write(&path, br#"[{"text":"old","at":"08:00"}]"#).unwrap();
        let mut history = History::load_from(path.clone());
        assert!(history.items[0].pinned);
        history.push("保留", "09:00");
        history.push("删除", "09:01");
        history.remove(0);
        let mut reopened = History::load_from(path.clone());
        assert_eq!(reopened.items, history.items);
        reopened.clear_recent();
        let final_history = History::load_from(path.clone());
        assert_eq!(final_history.items.len(), 1);
        assert_eq!(final_history.items[0].text, "old");
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn pinned_round_trip_and_preview() {
        let mut h = History::default();
        h.push("第一行\n第二行  末尾", "08:00");
        h.push("临时", "08:01");
        h.items[1].pinned = true;
        let back = History::from_json(&h.to_json());
        assert_eq!(back.items, h.items);
        assert_eq!(preview(&back.items[1].text), "第一行 第二行 末尾");
        assert!(History::from_json(b"not json").items.is_empty());
    }
}
