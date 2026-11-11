use serde::{Deserialize, Serialize};
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    pub text: String,
    pub done: bool,
    pub from: String,
}
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Day {
    pub text: String,
    pub tasks: Vec<Task>,
}
#[derive(Default, Serialize, Deserialize)]
pub struct Journal {
    pub days: std::collections::BTreeMap<String, Day>,
    pub carry: bool,
}
impl Journal {
    pub fn load() -> Self {
        super::load(&super::root().join("journal.json"))
    }
    pub fn save(&self) -> Result<(), String> {
        super::save(&super::root().join("journal.json"), self)
    }
    pub fn day(&mut self, date: &str) -> &mut Day {
        if !self.days.contains_key(date) {
            let tasks = if self.carry {
                self.days
                    .range(..date.to_string())
                    .next_back()
                    .map(|(from, day)| {
                        day.tasks
                            .iter()
                            .filter(|t| !t.done)
                            .cloned()
                            .map(|mut t| {
                                if t.from.is_empty() {
                                    t.from = from.clone();
                                }
                                t
                            })
                            .collect()
                    })
                    .unwrap_or_default()
            } else {
                Vec::new()
            };
            self.days.insert(
                date.into(),
                Day {
                    text: String::new(),
                    tasks,
                },
            );
        }
        self.days.get_mut(date).unwrap()
    }
    pub fn markdown(&self, date: &str) -> String {
        let Some(day) = self.days.get(date) else {
            return String::new();
        };
        let mut out = format!("# {date}\n\n## 每日笔记\n\n{}\n\n## 待办\n\n", day.text);
        for t in &day.tasks {
            out.push_str(&format!(
                "- [{}] {}{}\n",
                if t.done { "x" } else { " " },
                t.text.replace(['\n', '\r'], " "),
                if t.from.is_empty() {
                    String::new()
                } else {
                    format!("（结转自 {}）", t.from)
                }
            ));
        }
        out
    }
    pub fn export(&self, date: &str) -> Result<std::path::PathBuf, String> {
        chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d").map_err(|_| "日期无效")?;
        let path = super::root().join("exports").join(format!("{date}.md"));
        std::fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
        crate::model::atomic_write(&path, self.markdown(date).as_bytes())
            .map_err(|e| e.to_string())?;
        Ok(path)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn carry_once_and_keep_archive() {
        let mut j = Journal {
            carry: true,
            ..Default::default()
        };
        j.day("2026-11-08").tasks = vec![
            Task {
                id: "a".into(),
                text: "A".into(),
                ..Default::default()
            },
            Task {
                id: "b".into(),
                done: true,
                ..Default::default()
            },
        ];
        assert_eq!(j.day("2026-11-09").tasks.len(), 1);
        assert_eq!(j.day("2026-11-09").tasks.len(), 1);
        assert_eq!(j.days["2026-11-08"].tasks.len(), 2);
        assert!(j.markdown("2026-11-09").contains("结转自 2026-11-08"));
    }
}
