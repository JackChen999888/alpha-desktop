//! Local, opt-in system readings. No subprocesses, network requests or per-frame sampling.
use eframe::egui;
use std::{
    collections::HashMap,
    sync::mpsc,
    time::{Duration, Instant},
};
fn sample_interval(details: bool) -> Duration {
    Duration::from_secs(if details { 2 } else { 5 })
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Config {
    pub cpu: bool,
    pub memory: bool,
    pub gpu: bool,
    pub network: bool,
}
impl Config {
    pub fn enabled(self) -> bool {
        self.cpu || self.memory || self.gpu || self.network
    }
}
#[derive(Clone, Debug, Default)]
pub struct Reading {
    pub cpu: Option<f64>,
    pub memory: Option<f64>,
    pub gpu: Option<f64>,
    pub network: Option<(f64, f64)>,
    pub memory_bytes: Option<(u64, u64)>,
    pub cores: Vec<crate::monitor_detail::Core>,
    pub processes: Vec<crate::monitor_detail::Process>,
}
fn percent(v: Option<f64>) -> String {
    v.filter(|v| v.is_finite())
        .map(|v| format!("{:.0}%", v.clamp(0., 100.)))
        .unwrap_or_else(|| "—".into())
}
fn rate(v: f64) -> String {
    if v >= 1048576. {
        format!("{:.1}M", v / 1048576.)
    } else {
        format!("{:.0}K", v / 1024.)
    }
}
impl Reading {
    pub fn items(&self, c: Config) -> Vec<(&'static str, String)> {
        let mut v = Vec::new();
        if c.cpu {
            v.push(("CPU", percent(self.cpu)));
        }
        if c.memory {
            v.push(("RAM", percent(self.memory)));
        }
        if c.gpu {
            v.push(("GPU", percent(self.gpu)));
        }
        if c.network {
            v.push((
                "↓ / ↑",
                self.network
                    .map(|(a, b)| format!("{} / {}", rate(a), rate(b)))
                    .unwrap_or_else(|| "—".into()),
            ));
        }
        v
    }
    pub fn width(c: Config) -> f32 {
        let n = [c.cpu, c.memory, c.gpu].into_iter().filter(|v| *v).count();
        n as f32 * 52. + if c.network { 108. } else { 0. }
    }
    pub(crate) fn draw(&self, c: &crate::paint::Card, x: f32, config: Config) {
        let mut at = x - Self::width(config) / 2.;
        for (label, value) in self.items(config) {
            let w = if label == "↓ / ↑" { 108. } else { 52. };
            c.centered_text(
                at + w / 2.,
                8.,
                label,
                8.,
                crate::theme::W::Regular,
                crate::theme::GRAY,
                crate::paint::H::Center,
            );
            c.centered_text(
                at + w / 2.,
                20.,
                &value,
                10.,
                crate::theme::W::Semibold,
                crate::theme::WHITE,
                crate::paint::H::Center,
            );
            at += w;
        }
    }
}
#[derive(Default)]
pub struct Client {
    pub reading: Reading,
    control: Option<mpsc::Sender<(Config, bool, bool)>>,
    rx: Option<mpsc::Receiver<Reading>>,
    config: Config,
    details: bool,
    cores: bool,
    pub history: std::collections::VecDeque<Reading>,
    last: Option<Instant>,
}
impl Client {
    pub fn sync(&mut self, ctx: &egui::Context, config: Config, details: bool, cores: bool) {
        if !config.enabled() && !details && !cores {
            self.control = None;
            self.rx = None;
            self.reading = Reading::default();
            self.history.clear();
            self.config = config;
            return;
        }
        if self.control.is_none() {
            let (tx, commands) = mpsc::channel();
            let (out, rx) = mpsc::sync_channel(1);
            let ctx = ctx.clone();
            if std::thread::Builder::new()
                .name("alpha-system-monitor".into())
                .spawn(move || {
                    #[cfg(windows)]
                    let mut sampler = Sampler::default();
                    let mut current = (config, details, cores);
                    loop {
                        #[cfg(windows)]
                        let reading = sampler.sample_detail(current.0, current.1, current.2);
                        #[cfg(not(windows))]
                        let reading = Reading::default();
                        match out.try_send(reading) {
                            Err(mpsc::TrySendError::Disconnected(_)) => break,
                            _ => ctx.request_repaint(),
                        };
                        match commands.recv_timeout(sample_interval(current.1)) {
                            Ok(c) => current = c,
                            Err(mpsc::RecvTimeoutError::Disconnected) => break,
                            Err(_) => {}
                        }
                    }
                })
                .is_ok()
            {
                self.control = Some(tx);
                self.rx = Some(rx);
                self.config = config;
                self.details = details;
                self.cores = cores;
            }
        } else if self.config != config || self.details != details || self.cores != cores {
            if let Some(tx) = &self.control {
                let _ = tx.send((config, details, cores));
            }
            self.config = config;
            self.details = details;
            self.cores = cores;
        }
        if let Some(rx) = &self.rx {
            while let Ok(r) = rx.try_recv() {
                if self
                    .last
                    .is_some_and(|t| t.elapsed() > Duration::from_secs(6))
                {
                    self.history.clear();
                }
                self.history.push_back(Reading {
                    cpu: r.cpu,
                    memory: r.memory,
                    gpu: r.gpu,
                    network: r.network,
                    ..Default::default()
                });
                while self.history.len() > 60 {
                    self.history.pop_front();
                }
                self.reading = r;
                self.last = Some(Instant::now());
            }
        }
        if self
            .last
            .is_some_and(|at| at.elapsed() > Duration::from_secs(8))
        {
            self.reading = Reading::default();
        }
    }
    pub fn settings(&mut self, ui: &mut egui::Ui, config: &mut Config) {
        crate::extras::settings_card(ui, |ui| {
            ui.label(
                "收起时每 5 秒采样，展开监控时每 2 秒采样；关闭全部组件后停止采集。",
            );
            for (name, value) in [
                ("CPU 使用率", &mut config.cpu),
                ("内存使用率", &mut config.memory),
                ("GPU 使用率", &mut config.gpu),
                ("下载 / 上传速度", &mut config.network),
            ] {
                ui.checkbox(value, name);
            }
        });
        ui.add_space(12.);
        crate::extras::settings_card(ui, |ui| {
            for (label, value) in self.reading.items(*config) {
                ui.horizontal(|ui| {
                    ui.strong(label);
                    ui.label(value);
                });
            }
            ui.small("GPU 为所有显卡中最繁忙引擎的负载；驱动不支持时显示 —。网速统计已连接物理网卡，K / M 为 KiB / MiB 每秒，不重复计算 VPN 虚拟网卡。");
            ui.small(
                "顶部停靠时显示监控组件；侧边停靠可在本页查看。岛上歌词模式暂时替代监控读数。",
            );
        });
    }
}
fn cpu_delta(old: (u64, u64), new: (u64, u64)) -> Option<f64> {
    let total = new.1.checked_sub(old.1)?;
    let idle = new.0.checked_sub(old.0)?;
    (total > 0).then(|| 100. * (1. - idle.min(total) as f64 / total as f64))
}
fn gpu_load(values: Vec<(String, f64)>) -> Option<f64> {
    let mut engines: HashMap<String, f64> = HashMap::new();
    for (name, value) in values {
        if value.is_finite() && value >= 0. {
            if let Some((_, engine)) = name.split_once("_luid_") {
                *engines.entry(engine.into()).or_default() += value;
            }
        }
    }
    engines
        .values()
        .copied()
        .reduce(f64::max)
        .map(|v| v.clamp(0., 100.))
}
#[cfg(windows)]
#[derive(Default)]
struct Gpu {
    query: isize,
    counter: isize,
}
#[cfg(windows)]
impl Drop for Gpu {
    fn drop(&mut self) {
        if self.query != 0 {
            unsafe {
                windows::Win32::System::Performance::PdhCloseQuery(self.query);
            }
        }
    }
}
#[cfg(windows)]
impl Gpu {
    fn open() -> Option<Self> {
        use windows::{core::w, Win32::System::Performance::*};
        let mut g = Self::default();
        unsafe {
            if PdhOpenQueryW(None, 0, &mut g.query) != 0 {
                return None;
            }
            if PdhAddEnglishCounterW(
                g.query,
                w!("\\GPU Engine(*)\\Utilization Percentage"),
                0,
                &mut g.counter,
            ) != 0
            {
                return None;
            }
            PdhCollectQueryData(g.query);
        }
        Some(g)
    }
    fn read(&self) -> Option<f64> {
        use windows::Win32::System::Performance::*;
        unsafe {
            if PdhCollectQueryData(self.query) != 0 {
                return None;
            }
            let (mut size, mut count) = (0, 0);
            PdhGetFormattedCounterArrayW(self.counter, PDH_FMT_DOUBLE, &mut size, &mut count, None);
            if size == 0 || size > 8 * 1024 * 1024 {
                return None;
            }
            let mut storage = vec![0u64; (size as usize + 7) / 8];
            let ptr = storage.as_mut_ptr().cast::<PDH_FMT_COUNTERVALUE_ITEM_W>();
            if PdhGetFormattedCounterArrayW(
                self.counter,
                PDH_FMT_DOUBLE,
                &mut size,
                &mut count,
                Some(ptr),
            ) != 0
            {
                return None;
            }
            if count as usize
                > storage.len() * 8 / std::mem::size_of::<PDH_FMT_COUNTERVALUE_ITEM_W>()
            {
                return None;
            }
            let values = std::slice::from_raw_parts(ptr, count as usize)
                .iter()
                .filter(|v| v.FmtValue.CStatus <= 1)
                .filter_map(|v| {
                    Some((v.szName.to_string().ok()?, v.FmtValue.Anonymous.doubleValue))
                })
                .collect();
            gpu_load(values)
        }
    }
}
#[cfg(windows)]
#[derive(Default)]
struct Sampler {
    cpu: Option<(u64, u64)>,
    network: Option<(Instant, HashMap<u64, (u64, u64)>)>,
    gpu: Option<Gpu>,
    gpu_attempt: Option<Instant>,
    detail: Option<crate::monitor_detail::Sampler>,
}
#[cfg(windows)]
impl Sampler {
    fn sample_detail(&mut self, mut config: Config, details: bool, cores: bool) -> Reading {
        if details {
            config = Config {
                cpu: true,
                memory: true,
                gpu: true,
                network: true,
            };
        } else if cores {
            config.cpu = true;
        }
        let mut r = self.sample(config);
        if details || cores {
            (r.cores, r.processes) = self
                .detail
                .get_or_insert_with(Default::default)
                .sample(details);
        } else {
            self.detail = None;
        }
        r
    }
    fn sample(&mut self, c: Config) -> Reading {
        use windows::Win32::{
            Foundation::FILETIME,
            NetworkManagement::{IpHelper::*, Ndis::IfOperStatusUp},
            System::{
                SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX},
                Threading::GetSystemTimes,
            },
        };
        let mut r = Reading::default();
        unsafe {
            if c.cpu {
                let (mut idle, mut kernel, mut user) = (
                    FILETIME::default(),
                    FILETIME::default(),
                    FILETIME::default(),
                );
                if GetSystemTimes(Some(&mut idle), Some(&mut kernel), Some(&mut user)).is_ok() {
                    let ticks =
                        |v: FILETIME| ((v.dwHighDateTime as u64) << 32) | v.dwLowDateTime as u64;
                    let now = (ticks(idle), ticks(kernel) + ticks(user));
                    r.cpu = self.cpu.and_then(|p| cpu_delta(p, now));
                    self.cpu = Some(now);
                }
            } else {
                self.cpu = None;
            }
            if c.memory {
                let mut m = MEMORYSTATUSEX {
                    dwLength: std::mem::size_of::<MEMORYSTATUSEX>() as u32,
                    ..Default::default()
                };
                if GlobalMemoryStatusEx(&mut m).is_ok() && m.ullTotalPhys > 0 {
                    r.memory_bytes = Some((m.ullTotalPhys - m.ullAvailPhys, m.ullTotalPhys));
                    r.memory = Some(
                        100. * (m.ullTotalPhys - m.ullAvailPhys) as f64 / m.ullTotalPhys as f64,
                    );
                }
            }
            if c.network {
                let mut table = std::ptr::null_mut();
                if GetIfTable2(&mut table).is_ok() && !table.is_null() {
                    let mut totals = HashMap::new();
                    for row in std::slice::from_raw_parts(
                        (*table).Table.as_ptr(),
                        (*table).NumEntries as usize,
                    ) {
                        if row.OperStatus == IfOperStatusUp
                            && row.InterfaceAndOperStatusFlags._bitfield & 1 != 0
                        {
                            totals.insert(row.InterfaceLuid.Value, (row.InOctets, row.OutOctets));
                        }
                    }
                    FreeMibTable(table.cast());
                    let now = Instant::now();
                    if let Some((at, previous)) = &self.network {
                        let secs = now.duration_since(*at).as_secs_f64();
                        if secs > 0. {
                            let mut bytes = (0., 0.);
                            for (id, (a, b)) in &totals {
                                if let Some((old_a, old_b)) = previous.get(id) {
                                    if let (Some(a), Some(b)) =
                                        (a.checked_sub(*old_a), b.checked_sub(*old_b))
                                    {
                                        bytes.0 += a as f64;
                                        bytes.1 += b as f64;
                                    }
                                }
                            }
                            r.network = Some((bytes.0 / secs, bytes.1 / secs));
                        }
                    }
                    self.network = Some((now, totals));
                } else {
                    self.network = None;
                }
            } else {
                self.network = None;
            }
        }
        if c.gpu {
            if self.gpu.is_none()
                && self
                    .gpu_attempt
                    .is_none_or(|t| t.elapsed() > Duration::from_secs(60))
            {
                self.gpu_attempt = Some(Instant::now());
                self.gpu = Gpu::open();
            }
            r.gpu = self.gpu.as_ref().and_then(Gpu::read);
        } else {
            self.gpu = None;
            self.gpu_attempt = None;
        }
        r
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn monitor_cpu_and_gpu_aggregation() {
        assert_eq!(cpu_delta((100, 1000), (150, 1200)), Some(75.));
        assert_eq!(cpu_delta((100, 1000), (90, 900)), None);
        assert_eq!(
            gpu_load(vec![
                ("pid_1_luid_A_eng_0".into(), 20.),
                ("pid_2_luid_A_eng_0".into(), 30.),
                ("pid_2_luid_A_eng_1".into(), 40.)
            ]),
            Some(50.)
        );
        assert_eq!(gpu_load(vec![]), None);
    }
    #[test]
    fn monitor_unknown_is_not_zero() {
        let c = Config {
            cpu: true,
            memory: true,
            gpu: true,
            network: true,
        };
        assert!(Reading::default().items(c).iter().all(|(_, v)| v == "—"));
        assert_eq!(Reading::width(c), 264.);
        assert!(!Config::default().enabled());
    }
    #[test]
    #[cfg(windows)]
    #[ignore = "Local live system sampler"]
    fn monitor_live() {
        let mut s = Sampler::default();
        let c = Config {
            cpu: true,
            memory: true,
            gpu: true,
            network: true,
        };
        s.sample(c);
        std::thread::sleep(Duration::from_secs(2));
        let r = s.sample(c);
        println!("{r:?}");
        assert!(r.cpu.is_some());
        assert!(r.memory.is_some());
        assert!(r.network.is_some());
    }
}
