//! Additional native counters, sampled only while the monitor panel is visible.
use std::{collections::HashMap, time::Instant};
#[derive(Clone, Debug, Default)]
pub struct Core {
    pub name: String,
    pub load: f64,
    pub efficient: bool,
}
#[derive(Clone, Debug, Default)]
pub struct Process {
    pub pid: u32,
    pub name: String,
    pub cpu: Option<f64>,
    pub memory: u64,
    pub executable: Option<std::path::PathBuf>,
}
#[cfg(windows)]
pub struct Counter {
    query: isize,
    counter: isize,
}
#[cfg(windows)]
impl Drop for Counter {
    fn drop(&mut self) {
        unsafe {
            windows::Win32::System::Performance::PdhCloseQuery(self.query);
        }
    }
}
#[cfg(windows)]
impl Counter {
    pub fn open(path: &str) -> Option<Self> {
        use windows::{core::PCWSTR, Win32::System::Performance::*};
        let mut c = Self {
            query: 0,
            counter: 0,
        };
        let path: Vec<_> = path.encode_utf16().chain(Some(0)).collect();
        unsafe {
            if PdhOpenQueryW(None, 0, &mut c.query) != 0 {
                return None;
            }
            if PdhAddEnglishCounterW(c.query, PCWSTR(path.as_ptr()), 0, &mut c.counter) != 0 {
                return None;
            }
            PdhCollectQueryData(c.query);
        }
        Some(c)
    }
    pub fn read(&self) -> Vec<(String, f64)> {
        use windows::Win32::System::Performance::*;
        unsafe {
            if PdhCollectQueryData(self.query) != 0 {
                return vec![];
            }
            let (mut size, mut count) = (0, 0);
            PdhGetFormattedCounterArrayW(self.counter, PDH_FMT_DOUBLE, &mut size, &mut count, None);
            if size == 0 || size > 16 * 1024 * 1024 {
                return vec![];
            }
            let mut buf = vec![0u64; (size as usize + 7) / 8];
            let p = buf.as_mut_ptr().cast::<PDH_FMT_COUNTERVALUE_ITEM_W>();
            if PdhGetFormattedCounterArrayW(
                self.counter,
                PDH_FMT_DOUBLE,
                &mut size,
                &mut count,
                Some(p),
            ) != 0
                || count as usize
                    > buf.len() * 8 / std::mem::size_of::<PDH_FMT_COUNTERVALUE_ITEM_W>()
            {
                return vec![];
            }
            std::slice::from_raw_parts(p, count as usize)
                .iter()
                .filter(|v| v.FmtValue.CStatus <= 1)
                .filter_map(|v| {
                    let value = v.FmtValue.Anonymous.doubleValue;
                    if value.is_finite() {
                        Some((v.szName.to_string().ok()?, value))
                    } else {
                        None
                    }
                })
                .collect()
        }
    }
}
#[cfg(windows)]
#[derive(Default)]
pub struct Sampler {
    counter: Option<Counter>,
    attempt: Option<Instant>,
    classes: HashMap<String, u8>,
    previous: HashMap<u32, (u64, u64)>,
    at: Option<Instant>,
    paths: HashMap<u32, (u64, Option<std::path::PathBuf>)>,
}
#[cfg(windows)]
impl Sampler {
    pub fn sample(&mut self, processes: bool) -> (Vec<Core>, Vec<Process>) {
        if self.counter.is_none() && self.attempt.is_none_or(|t| t.elapsed().as_secs() > 60) {
            self.attempt = Some(Instant::now());
            self.counter = Counter::open(r"\Processor Information(*)\% Processor Time");
            self.classes = classes();
        }
        let max_class = self.classes.values().copied().max().unwrap_or(0);
        let mut cores: Vec<_> = self
            .counter
            .as_ref()
            .map(|c| c.read())
            .unwrap_or_default()
            .into_iter()
            .filter(|(name, _)| {
                name.split(',').count() == 2 && name.split(',').all(|s| s.parse::<u16>().is_ok())
            })
            .map(|(name, load)| Core {
                efficient: self.classes.get(&name).is_some_and(|c| *c < max_class),
                name,
                load: load.clamp(0., 100.),
            })
            .collect();
        cores.sort_by_key(|c| {
            let mut n = c.name.split(',').map(|v| v.parse::<u16>().unwrap_or(0));
            (n.next(), n.next())
        });
        let rows = if processes {
            self.processes(
                cores.len().max(
                    std::thread::available_parallelism()
                        .map(|n| n.get())
                        .unwrap_or(1),
                ),
            )
        } else {
            self.previous.clear();
            self.paths.clear();
            self.at = None;
            vec![]
        };
        (cores, rows)
    }
    fn processes(&mut self, cpus: usize) -> Vec<Process> {
        use windows::Win32::{
            Foundation::*,
            System::{Diagnostics::ToolHelp::*, ProcessStatus::*, Threading::*},
        };
        let now = Instant::now();
        let dt = self.at.map(|at| now.duration_since(at).as_secs_f64());
        let mut previous = HashMap::new();
        let mut rows = Vec::new();
        unsafe {
            let Ok(snapshot) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) else {
                return rows;
            };
            let mut e = PROCESSENTRY32W {
                dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
                ..Default::default()
            };
            let mut valid = Process32FirstW(snapshot, &mut e).is_ok();
            while valid {
                if e.th32ProcessID != 0 {
                    if let Ok(p) =
                        OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, e.th32ProcessID)
                    {
                        let (mut create, mut exit, mut kernel, mut user) = (
                            FILETIME::default(),
                            FILETIME::default(),
                            FILETIME::default(),
                            FILETIME::default(),
                        );
                        let mut m = PROCESS_MEMORY_COUNTERS {
                            cb: std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32,
                            ..Default::default()
                        };
                        if GetProcessTimes(p, &mut create, &mut exit, &mut kernel, &mut user)
                            .is_ok()
                            && K32GetProcessMemoryInfo(
                                p,
                                &mut m,
                                std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32,
                            )
                            .as_bool()
                        {
                            let ticks = |t: FILETIME| {
                                ((t.dwHighDateTime as u64) << 32) | t.dwLowDateTime as u64
                            };
                            let birth = ticks(create);
                            let ticks = ticks(kernel) + ticks(user);
                            let cpu = dt.filter(|s| *s > 0.).and_then(|s| {
                                self.previous
                                    .get(&e.th32ProcessID)
                                    .filter(|(b, _)| *b == birth)
                                    .and_then(|(_, v)| ticks.checked_sub(*v))
                                    .map(|v| {
                                        (v as f64 / 1e7 / s / cpus as f64 * 100.).clamp(0., 100.)
                                    })
                            });
                            previous.insert(e.th32ProcessID, (birth, ticks));
                            let end = e
                                .szExeFile
                                .iter()
                                .position(|c| *c == 0)
                                .unwrap_or(e.szExeFile.len());
                            let executable = self.paths.get(&e.th32ProcessID)
                                .filter(|(created, _)| *created == birth).map(|(_, path)| path.clone())
                                .unwrap_or_else(|| {
                                    let mut buffer = vec![0u16; 32768];
                                    let mut size = buffer.len() as u32;
                                    let path = QueryFullProcessImageNameW(p, PROCESS_NAME_WIN32,
                                        windows::core::PWSTR(buffer.as_mut_ptr()), &mut size).ok()
                                        .map(|_| std::path::PathBuf::from(String::from_utf16_lossy(&buffer[..size as usize])));
                                    self.paths.insert(e.th32ProcessID, (birth, path.clone()));
                                    path
                                });
                            rows.push(Process {
                                pid: e.th32ProcessID,
                                name: String::from_utf16_lossy(&e.szExeFile[..end]),
                                cpu,
                                memory: m.WorkingSetSize as u64,
                                executable,
                            });
                        }
                        let _ = CloseHandle(p);
                    }
                }
                valid = Process32NextW(snapshot, &mut e).is_ok();
            }
            let _ = CloseHandle(snapshot);
        }
        self.previous = previous;
        self.paths.retain(|pid, _| self.previous.contains_key(pid));
        self.at = Some(now);
        rows
    }
}
#[cfg(windows)]
fn classes() -> HashMap<String, u8> {
    use windows::Win32::{Foundation::HANDLE, System::SystemInformation::*};
    unsafe {
        let mut size = 0;
        let _ = GetSystemCpuSetInformation(None, 0, &mut size, HANDLE::default(), 0);
        if size == 0 || size > 1024 * 1024 {
            return HashMap::new();
        }
        let mut buf = vec![0u64; (size as usize + 7) / 8];
        if !GetSystemCpuSetInformation(
            Some(buf.as_mut_ptr().cast()),
            size,
            &mut size,
            HANDLE::default(),
            0,
        )
        .as_bool()
        {
            return HashMap::new();
        }
        let mut result = HashMap::new();
        let mut offset = 0;
        while offset + std::mem::size_of::<SYSTEM_CPU_SET_INFORMATION>() <= size as usize {
            let info = &*buf
                .as_ptr()
                .cast::<u8>()
                .add(offset)
                .cast::<SYSTEM_CPU_SET_INFORMATION>();
            if info.Size == 0 || offset + info.Size as usize > size as usize {
                break;
            }
            if info.Type == CpuSetInformation {
                let c = info.Anonymous.CpuSet;
                result.insert(
                    format!("{},{}", c.Group, c.LogicalProcessorIndex),
                    c.EfficiencyClass,
                );
            }
            offset += info.Size as usize;
        }
        result
    }
}

#[cfg(all(test, windows))]
mod tests {
    #[test]
    #[ignore = "Reads native Windows counters and live process statistics"]
    fn dashboard_native_live() {
        let mut s = super::Sampler::default();
        s.sample(true);
        std::thread::sleep(std::time::Duration::from_secs(2));
        let (c, p) = s.sample(true);
        assert!(!c.is_empty(), "No logical CPU counters");
        assert!(!p.is_empty(), "No process working sets");
        assert!(p.iter().any(|p| p.cpu.is_some()));
        assert!(p.iter().any(|p| p.pid==std::process::id() && p.executable.as_ref().is_some_and(|path| path.is_absolute())));
        println!(
            "logical_cpus={} efficient={} readable_processes={}",
            c.len(),
            c.iter().filter(|c| c.efficient).count(),
            p.len()
        );
    }
}
