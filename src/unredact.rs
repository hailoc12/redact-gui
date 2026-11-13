//! Engine unredact: khôi phục dữ liệu gốc từ placeholder dùng dict (mapping)
//! sinh ra ở bước redact. Dict = JSON array [{pattern, original, placeholder}]
//! (định dạng trùng output của `--map` / GUI).

use std::collections::HashMap;

use anyhow::{Context, Result};
use regex::Regex;

use crate::redactor::MapEntry;

pub struct Unredactor {
    /// placeholder → original
    map: HashMap<String, String>,
    re: Regex,
    /// tổng số entry trong dict
    pub dict_size: usize,
}

impl Unredactor {
    /// Dựng từ nội dung file dict (JSON array MapEntry).
    pub fn from_dict_json(json: &str) -> Result<Self> {
        let entries: Vec<MapEntry> =
            serde_json::from_str(json).context("dict JSON không hợp lệ")?;
        anyhow::ensure!(
            !entries.is_empty(),
            "dict rỗng — không có gì để khôi phục"
        );

        let mut map = HashMap::new();
        for e in &entries {
            if e.placeholder.is_empty() || e.original.is_empty() {
                anyhow::bail!(
                    "dict có entry thiếu placeholder/original (pattern '{}')",
                    e.pattern
                );
            }
            map.insert(e.placeholder.clone(), e.original.clone());
        }

        // union tất cả placeholder thành 1 regex, dài trước để [SDT_1] không cắt [SDT_10]
        let mut tokens: Vec<&String> = map.keys().collect();
        tokens.sort_by(|a, b| b.len().cmp(&a.len()));
        let alternation: Vec<String> = tokens.iter().map(|t| regex::escape(t)).collect();
        let re = Regex::new(&alternation.join("|"))
            .context("không dựng được matcher từ dict")?;

        Ok(Self {
            map,
            re,
            dict_size: entries.len(),
        })
    }

    /// Khôi phục text: trả về (text gốc, số placeholder đã thay).
    pub fn unredact(&self, text: &str) -> (String, usize) {
        let mut count = 0usize;
        let out = self
            .re
            .replace_all(text, |caps: &regex::Captures| -> String {
                let token = caps.get(0).unwrap().as_str();
                match self.map.get(token) {
                    Some(original) => {
                        count += 1;
                        original.clone()
                    }
                    None => token.to_string(),
                }
            })
            .into_owned();
        (out, count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::patterns::BUILT_IN;
    use crate::redactor::Redactor;

    fn redact_all() -> Vec<crate::patterns::Pattern> {
        BUILT_IN.iter().filter(|p| p.enabled).cloned().collect()
    }

    #[test]
    fn round_trip_redact_unredact() {
        let original = "CCCD 012345678901, SĐT 0901 234 567, email a@b.com của Công ty TNHH ABC";
        let mut r = Redactor::new(redact_all()).unwrap();
        let redacted = r.redact(original);
        assert_ne!(redacted, original);

        let dict = serde_json::to_string(&r.mapping).unwrap();
        let u = Unredactor::from_dict_json(&dict).unwrap();
        let (restored, n) = u.unredact(&redacted);
        assert_eq!(restored, original);
        assert_eq!(n, 4);
    }

    #[test]
    fn placeholder_prefix_no_collision() {
        // 10+ số khác nhau → [SDT_1] và [SDT_10] phải không đè nhau
        let original = "1:0901111001 2:0901111002 3:0901111003 4:0901111004 5:0901111005 6:0901111006 7:0901111007 8:0901111008 9:0901111009 10:0901111010 11:0901111011";
        let mut r = Redactor::new(redact_all()).unwrap();
        let redacted = r.redact(original);
        assert!(redacted.contains("[SDT_10]") && redacted.contains("[SDT_1]"));
        let dict = serde_json::to_string(&r.mapping).unwrap();
        let u = Unredactor::from_dict_json(&dict).unwrap();
        let (restored, _) = u.unredact(&redacted);
        assert_eq!(restored, original);
    }

    #[test]
    fn empty_dict_rejected() {
        assert!(Unredactor::from_dict_json("[]").is_err());
        assert!(Unredactor::from_dict_json("not json").is_err());
    }
}
