//! Reclaim disposable UI caches once idle; retain all user data and active previews.
use std::time::{Duration, Instant};

#[derive(Default)]
pub struct IdleReclaimer {
    idle_since: Option<Instant>,
    last_release: Option<Instant>,
    released: bool,
}
impl IdleReclaimer {
    pub fn due(&mut self, active: bool, now: Instant) -> bool {
        if active { self.idle_since=None; self.released=false; return false; }
        let idle=self.idle_since.get_or_insert(now);
        if self.released || now.saturating_duration_since(*idle)<Duration::from_secs(30)
            || self.last_release.is_some_and(|last|now.saturating_duration_since(last)<Duration::from_secs(300)) {return false;}
        self.released=true;self.last_release=Some(now);true
    }
}

impl crate::app::App {
    pub fn memory_detail_active(&self, ctx:&eframe::egui::Context, expanded:bool)->bool {
        let settings_visible=self.settings_open && !ctx.input(|i|i.raw.viewports
            .get(&eframe::egui::ViewportId::from_hash_of("daozai-settings"))
            .is_some_and(|v|v.minimized==Some(true)));
        expanded || settings_visible || self.mail.reading.is_some()
    }
    pub fn release_idle_previews(&mut self, ctx:&eframe::egui::Context) {
        self.icons.release();
        self.dashboard.release_previews();
        if let Some(pro)=self.pro.as_mut(){pro.release_previews();}
        ctx.forget_all_images();
    }
}

#[cfg(windows)]
fn optimize_heap()->bool {
    use windows::Win32::{Foundation::HANDLE,System::Memory::{HeapSetInformation,HeapOptimizeResources}};
    #[repr(C)] struct Info { version:u32, flags:u32 }
    let info=Info{version:1,flags:0};
    // Microsoft documents NULL as optimizing all LFH caches in this process,
    // decommitting free pages where possible. This does not page out live data.
    unsafe {HeapSetInformation(HANDLE::default(),HeapOptimizeResources,
        Some((&info as *const Info).cast()),std::mem::size_of::<Info>()).is_ok()}
}

pub fn release_heap_async() {
    #[cfg(windows)] {
        static LANE:crate::resources::Lane=crate::resources::Lane::new("alpha-memory-reclaim",1);
        let _=LANE.spawn(|| {
            // Allow the render pass to finish releasing texture handles first.
            std::thread::sleep(Duration::from_secs(1));
            let before=crate::resources::snapshot();let started=Instant::now();
            let ok=optimize_heap();let elapsed=started.elapsed().as_millis();
            let after=crate::resources::snapshot();
            let line=format!("heap_optimized={ok} elapsed_ms={elapsed} private_before={} private_after={}",
                before.map(|s|s.private_bytes).unwrap_or(0),after.map(|s|s.private_bytes).unwrap_or(0));
            crate::resources::append_log(&crate::model::config_dir().join("diagnostics"),"memory-reclaim.log",&line);
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn idle_release_waits_rearms_and_is_rate_limited(){
        let mut state=IdleReclaimer::default();let start=Instant::now();
        assert!(!state.due(false,start));assert!(!state.due(false,start+Duration::from_secs(29)));
        assert!(state.due(false,start+Duration::from_secs(30)));
        assert!(!state.due(false,start+Duration::from_secs(600)),"no repeated work while still idle");
        assert!(!state.due(true,start+Duration::from_secs(601)));
        assert!(!state.due(false,start+Duration::from_secs(602)));
        assert!(state.due(false,start+Duration::from_secs(632)));
        state.due(true,start+Duration::from_secs(633));state.due(false,start+Duration::from_secs(634));
        assert!(!state.due(false,start+Duration::from_secs(664)),"quickly reopening must not cause heap churn");
        assert!(state.due(false,start+Duration::from_secs(933)));
    }
    #[test]
    #[cfg(windows)]
    #[ignore="Explicit current-process Windows heap optimization check"]
    fn native_heap_reclaim_is_supported(){assert!(optimize_heap());}
}
