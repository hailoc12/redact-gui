//! Activity log cho bản tổ chức (org-mode) — CHỈ compile khi bật cargo feature
//! `org-logging`. Bản release cá nhân KHÔNG bật feature này → không một dòng
//! code network nào tồn tại trong binary.
//!
//! Interface: POST JSON {tool, action, file_name, machine, timestamp} tới
//! endpoint lấy từ biến môi trường REDACT_ORG_LOG_URL (http:// nội bộ).
//! Viết bằng std::net::TcpStream thuần — không thêm dependency.

#[derive(serde::Serialize)]
pub struct ActivityEvent {
    pub tool: &'static str,
    pub action: String,
    pub file_name: String,
    pub machine: String,
    pub timestamp: String,
}

/// Ghi activity log về API tổ chức. Bỏ qua âm thầm nếu chưa cấu hình endpoint
/// (REDACT_ORG_LOG_URL) — activity log không bao giờ chặn nghiệp vụ redact.
#[cfg(feature = "org-logging")]
pub fn log_activity(action: &str, file_name: &str) {
    let Some(url) = std::env::var("REDACT_ORG_LOG_URL").ok() else {
        return; // bản tổ chức chưa cấu hình endpoint — im lặng bỏ qua
    };
    let event = ActivityEvent {
        tool: "redact-gui",
        action: action.to_string(),
        file_name: file_name.to_string(),
        machine: hostname(),
        timestamp: chrono_like_now(),
    };
    let body = serde_json::to_string(&event).unwrap_or_default();
    let _ = http_post(&url, &body);
}

/// Bản không bật feature: hàm rỗng — compiler loại bỏ toàn bộ call sites.
#[cfg(not(feature = "org-logging"))]
pub fn log_activity(_action: &str, _file_name: &str) {}

#[cfg(feature = "org-logging")]
fn hostname() -> String {
    std::env::var("HOSTNAME")
        .or_else(|_| std::env::var("COMPUTERNAME"))
        .ok()
        .or_else(|| whoami_fallback())
        .unwrap_or_else(|| "unknown".into())
}

#[cfg(feature = "org-logging")]
fn whoami_fallback() -> Option<String> {
    std::process::Command::new("hostname")
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
}

#[cfg(feature = "org-logging")]
fn chrono_like_now() -> String {
    // ISO-8601 UTC không cần dependency chrono
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let days = secs / 86400;
    let (y, m, d) = civil_from_days(days as i64);
    let rem = secs % 86400;
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        (rem % 3600) / 60,
        rem % 60
    )
}

/// Chuyển số ngày từ 1970 → (năm, tháng, ngày) — thuật toán Howard Hinnant.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// POST body tới URL dạng http://host:port/path bằng TcpStream thuần.
#[cfg(feature = "org-logging")]
fn http_post(url: &str, body: &str) -> std::io::Result<()> {
    use std::io::Write;
    let rest = url.strip_prefix("http://").unwrap_or(url);
    let (hostport, path) = match rest.split_once('/') {
        Some((h, p)) => (h.to_string(), format!("/{p}")),
        None => (rest.to_string(), "/".to_string()),
    };
    let (host, port) = match hostport.rsplit_once(':') {
        Some((h, p)) => (h.to_string(), p.parse().unwrap_or(80)),
        None => (hostport.clone(), 80),
    };
    let mut stream = std::net::TcpStream::connect((host.as_str(), port))?;
    let req = format!(
        "POST {path} HTTP/1.1\r\nHost: {hostport}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(req.as_bytes())
}

#[cfg(all(test, feature = "org-logging"))]
mod tests {
    use super::*;

    #[test]
    fn date_conversion_epoch() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(19723), (2024, 1, 1)); // 1/1/2024
    }

    #[test]
    fn http_post_reaches_local_listener() {
        use std::io::{BufRead, BufReader};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let handle = std::thread::spawn(move || {
            let (s, _) = listener.accept().unwrap();
            let mut line = String::new();
            BufReader::new(s).read_line(&mut line).unwrap();
            line
        });
        std::thread::sleep(std::time::Duration::from_millis(100));
        let _ = http_post(&format!("http://127.0.0.1:{port}/log"), r#"{"a":1}"#);
        let got = handle.join().unwrap();
        assert!(got.starts_with("POST /log HTTP/1.1"), "request line: {got}");
    }

    #[test]
    fn event_serializes_full_shape() {
        let e = ActivityEvent {
            tool: "redact-gui",
            action: "redact".into(),
            file_name: "a.txt".into(),
            machine: "m1".into(),
            timestamp: "2026-11-15T00:00:00Z".into(),
        };
        let j = serde_json::to_string(&e).unwrap();
        for k in ["tool", "action", "file_name", "machine", "timestamp"] {
            assert!(j.contains(k), "thiếu field {k}: {j}");
        }
    }
}

// ═══ Local activity log — LUÔN ghi (không phải network), hiển thị ở tab Log ═══
// File: ~/.redact-gui/activity.jsonl — mỗi dòng 1 event JSON.

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct LocalEvent {
    pub timestamp: String,
    pub action: String,
    pub file_name: String,
    pub doc_class: String,
    pub machine: String,
}

/// Fingerprint máy: hash hostname + OS (không lưu thông tin cá nhân nặng).
pub fn machine_fingerprint() -> String {
    let host = std::env::var("HOSTNAME")
        .or_else(|_| std::env::var("COMPUTERNAME"))
        .ok()
        .or_else(|| {
            std::process::Command::new("hostname")
                .output()
                .ok()
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        })
        .unwrap_or_else(|| "unknown".into());
    let os = if cfg!(target_os = "windows") {
        "win"
    } else if cfg!(target_os = "macos") {
        "mac"
    } else {
        "linux"
    };
    format!("{os}-{host}")
}

/// Đường dẫn file log local.
pub fn local_log_path() -> std::path::PathBuf {
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(std::path::PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    home.join(".redact-gui").join("activity.jsonl")
}

/// Ghi 1 event vào log local (append JSONL, tự tạo thư mục).
pub fn log_local(action: &str, file_name: &str, doc_class: &str) {
    let ev = LocalEvent {
        timestamp: crate::orglog::now_iso(),
        action: action.to_string(),
        file_name: file_name.to_string(),
        doc_class: doc_class.to_string(),
        machine: machine_fingerprint(),
    };
    let path = local_log_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(line) = serde_json::to_string(&ev) {
        use std::io::Write;
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
        {
            let _ = writeln!(f, "{line}");
        }
    }
}

/// Đọc toàn bộ log local (mới nhất cuối).
pub fn read_local_log() -> Vec<LocalEvent> {
    let mut out = Vec::new();
    if let Ok(content) = std::fs::read_to_string(local_log_path()) {
        for line in content.lines().rev() {
            if let Ok(ev) = serde_json::from_str::<LocalEvent>(line) {
                out.push(ev);
            }
        }
    }
    out
}

/// ISO-8601 UTC không dependency.
pub fn now_iso() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let days = secs / 86400;
    let (y, m, d) = civil_from_days(days as i64);
    let rem = secs % 86400;
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        (rem % 3600) / 60,
        rem % 60
    )
}

/// Trạng thái org-mode: build có feature + endpoint đã cấu hình.
pub fn org_mode_status() -> (bool, bool) {
    (
        cfg!(feature = "org-logging"),
        std::env::var("REDACT_ORG_LOG_URL").is_ok(),
    )
}
