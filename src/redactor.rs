//! Engine redact: khớp pattern, gán placeholder nhất quán, đếm số thay thế.

use std::collections::HashMap;

use anyhow::{Context, Result};
use regex::Regex;

use crate::patterns::{Normalize, Pattern};

/// Một entry trong mapping (chỉ ghi ra file khi --map).
#[derive(Clone, Debug)]
pub struct MapEntry {
    pub pattern: String,
    pub original: String,
    pub placeholder: String,
}

#[derive(Default, Clone)]
pub struct RedactStats {
    /// pattern id → số lần match
    pub per_pattern: Vec<(String, usize)>,
    pub total: usize,
}

pub struct Redactor {
    regex: Regex,
    patterns: Vec<Pattern>,
    /// key chuẩn hoá → placeholder
    assigned: HashMap<String, String>,
    /// pattern label → số placeholder đã cấp
    counters: HashMap<String, usize>,
    counts: HashMap<String, usize>,
    /// mapping theo thứ tự xuất hiện
    pub mapping: Vec<MapEntry>,
}

impl Redactor {
    /// Dựng redactor từ danh sách pattern (đã lọc theo --patterns / rules).
    pub fn new(patterns: Vec<Pattern>) -> Result<Self> {
        anyhow::ensure!(!patterns.is_empty(), "không có pattern nào được bật");
        let parts: Vec<String> = patterns.iter().map(|p| format!("({})", p.regex)).collect();
        let combined = parts.join("|");
        let regex = Regex::new(&combined).with_context(|| "regex pattern không hợp lệ")?;
        Ok(Self {
            regex,
            patterns,
            assigned: HashMap::new(),
            counters: HashMap::new(),
            counts: HashMap::new(),
            mapping: Vec::new(),
        })
    }

    /// Chuẩn hoá giá trị match thành key tra mapping.
    fn normalize(normalize: Normalize, raw: &str) -> String {
        match normalize {
            Normalize::None => raw.to_string(),
            Normalize::Digits => raw.chars().filter(|c| c.is_ascii_digit()).collect(),
            Normalize::Phone => {
                let digits: String = raw
                    .chars()
                    .filter(|c| c.is_ascii_digit() || *c == '+')
                    .collect();
                let digits = digits.trim_start_matches('+');
                // 84xxxxxxxxx → 0xxxxxxxxx (cùng 1 số, 2 cách viết)
                if digits.len() == 11 && digits.starts_with("84") {
                    format!("0{}", &digits[2..])
                } else {
                    digits.to_string()
                }
            }
        }
    }

    /// Cấp placeholder cho giá trị (cùng giá trị → cùng placeholder).
    fn placeholder_for(
        &mut self,
        id: &str,
        label: &str,
        normalize: Normalize,
        raw: &str,
    ) -> String {
        let key = format!("{}:{}", id, Self::normalize(normalize, raw));
        if let Some(ph) = self.assigned.get(&key) {
            return ph.clone();
        }
        let n = self.counters.entry(label.to_string()).or_insert(0);
        *n += 1;
        let ph = format!("[{}_{}]", label, n);
        self.assigned.insert(key, ph.clone());
        self.mapping.push(MapEntry {
            pattern: id.to_string(),
            original: raw.to_string(),
            placeholder: ph.clone(),
        });
        ph
    }

    /// Tìm pattern nào match trong captures: (index, id, label, normalize, raw)
    fn matched_pattern(
        &self,
        caps: &regex::Captures,
    ) -> Option<(String, String, Normalize, String)> {
        for (i, pattern) in self.patterns.iter().enumerate() {
            if let Some(m) = caps.get(i + 1) {
                return Some((
                    pattern.id.to_string(),
                    pattern.label.to_string(),
                    pattern.normalize,
                    m.as_str().to_string(),
                ));
            }
        }
        None
    }

    /// Redact toàn bộ text. Cập nhật stats + mapping.
    pub fn redact(&mut self, text: &str) -> String {
        // mượn regex ra để closure được mượn self mutable
        let regex = std::mem::replace(&mut self.regex, Regex::new("").unwrap());
        let result = regex
            .replace_all(text, |caps: &regex::Captures| -> String {
                match self.matched_pattern(caps) {
                    Some((id, label, normalize, raw)) => {
                        *self.counts.entry(id.clone()).or_insert(0) += 1;
                        self.placeholder_for(&id, &label, normalize, &raw)
                    }
                    None => caps.get(0).unwrap().as_str().to_string(),
                }
            })
            .into_owned();
        self.regex = regex;
        result
    }

    /// Chỉ đếm, không thay thế (dry-run).
    pub fn count_only(&mut self, text: &str) {
        let regex = std::mem::replace(&mut self.regex, Regex::new("").unwrap());
        let matched: Vec<(String, String, Normalize, String)> = regex
            .captures_iter(text)
            .filter_map(|caps| self.matched_pattern(&caps))
            .collect();
        self.regex = regex;
        for (id, label, normalize, raw) in matched {
            *self.counts.entry(id.clone()).or_insert(0) += 1;
            self.placeholder_for(&id, &label, normalize, &raw);
        }
    }

    /// Stats từ lần chạy hiện tại.
    pub fn stats(&self) -> RedactStats {
        RedactStats::from_counts(&self.counts)
    }

    /// Snapshot bộ đếm hiện tại (mốc `before` cho stats per-file).
    pub fn snapshot(&self) -> HashMap<String, usize> {
        self.counts.clone()
    }

    /// Stats chênh lệch từ một mốc snapshot.
    pub fn stats_since(&self, before: &HashMap<String, usize>) -> RedactStats {
        let counts: HashMap<String, usize> = self
            .counts
            .iter()
            .map(|(k, v)| {
                (
                    k.clone(),
                    v.saturating_sub(before.get(k).copied().unwrap_or(0)),
                )
            })
            .filter(|(_, v)| *v > 0)
            .collect();
        RedactStats::from_counts(&counts)
    }
}

impl RedactStats {
    fn from_counts(counts: &HashMap<String, usize>) -> RedactStats {
        let mut per_pattern: Vec<(String, usize)> =
            counts.iter().map(|(k, v)| (k.clone(), *v)).collect();
        per_pattern.sort();
        let total = per_pattern.iter().map(|(_, v)| v).sum();
        RedactStats { per_pattern, total }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::patterns::BUILT_IN;

    fn all() -> Vec<Pattern> {
        BUILT_IN.iter().filter(|p| p.enabled).cloned().collect()
    }

    #[test]
    fn same_value_same_placeholder() {
        let mut r = Redactor::new(all()).unwrap();
        let out = r.redact("CCCD 012345678901 và lặp lại 012345678901");
        assert_eq!(out, "CCCD [CCCD_1] và lặp lại [CCCD_1]");
    }

    #[test]
    fn phone_formats_unify() {
        let mut r = Redactor::new(all()).unwrap();
        let out = r.redact("Gọi 0901 234 567 hoặc +84901234567");
        assert_eq!(out, "Gọi [SDT_1] hoặc [SDT_1]");
    }

    #[test]
    fn cccd_wins_over_sdt() {
        let mut r = Redactor::new(all()).unwrap();
        let out = r.redact("012345678901");
        assert_eq!(out, "[CCCD_1]");
    }

    #[test]
    fn stats_count_matches() {
        let mut r = Redactor::new(all()).unwrap();
        r.redact("a@b.com x a@b.com y 0901234567");
        let s = r.stats();
        assert_eq!(s.total, 3);
        assert_eq!(
            s.per_pattern.iter().find(|(k, _)| k == "email").unwrap().1,
            2
        );
        assert_eq!(s.per_pattern.iter().find(|(k, _)| k == "sdt").unwrap().1, 1);
    }

    #[test]
    fn count_only_does_not_replace_but_counts() {
        let mut r = Redactor::new(all()).unwrap();
        r.count_only("0901234567 0901234567");
        assert_eq!(r.stats().total, 2);
    }
}
