//! redact-cli — CLI của Redact GUI, redact thông tin nhạy cảm trước khi đưa tài liệu lên AI.
//! 100% xử lý local, zero telemetry, không có bất kỳ kết nối mạng nào.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use clap::Parser;

use redact_cli::brand::Style;
use redact_cli::formats;
use redact_cli::patterns::{self, CustomRule, Pattern, BUILT_IN};
use redact_cli::redactor::{RedactStats, Redactor};
use redact_cli::unredact::Unredactor;

#[derive(Parser)]
#[command(
    name = "redact-cli",
    version,
    about = "Redact thông tin nhạy cảm (CCCD, CMND, MST, SĐT, email, tên DN) — 100% local, zero telemetry",
    after_help = "Công cụ chỉ tự động hoá che một số thông tin nhạy cảm thường gặp — KHÔNG bảo đảm\ntuyệt đối không sót. Đưa văn bản đã redact lên AI hay không là trách nhiệm người dùng.\nTUYỆT ĐỐI KHÔNG dùng cho văn bản đóng dấu MẬT.\n\nChế độ an toàn: tool không có bất kỳ tính năng network nào. Mọi xử lý trên máy bạn.\n\nVí dụ:\n  redact-cli --level 2 baocao.pdf\n  redact-cli --dry-run baocao.pdf\n  redact-cli unredact baocao.redacted.txt --dict baocao.dict.json"
)]
struct Args {
    /// Subcommand: redact (mặc định) hoặc unredact
    #[command(subcommand)]
    command: Option<Command>,
    /// File hoặc thư mục đầu vào (nhiều file cách nhau bằng khoảng trắng)
    input: Vec<PathBuf>,

    /// Thư mục đích (mặc định: ghi file *.redacted.* cạnh file gốc)
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Ghi đè lên file gốc (cẩn trọng — không có backup)
    #[arg(long)]
    in_place: bool,

    /// Chỉ quét + báo cáo số lượng, không ghi file nào
    #[arg(long)]
    dry_run: bool,

    /// Chỉ chạy một số pattern (vd: cccd,sdt). Xem --list-patterns
    #[arg(long, value_delimiter = ',', conflicts_with = "level")]
    patterns: Vec<String>,

    /// Mức redact nhanh: 1=Liên hệ (email,SĐT) · 2=Nhận diện (+CCCD,CMND,MST) · 3=Tối đa (+tên DN)
    #[arg(long, value_parser = clap::value_parser!(u8).range(1..=3))]
    level: Option<u8>,

    /// File rules JSON tùy chỉnh ([{"id","label","regex"}...])
    #[arg(long)]
    rules: Option<PathBuf>,

    /// Ghi file mapping original↔placeholder (CHỨA DỮ LIỆU NHẠY CẢM — chỉ dùng khi cần)
    #[arg(long)]
    map: Option<PathBuf>,

    /// Duyệt đệ quy khi input là thư mục
    #[arg(short, long)]
    recursive: bool,

    /// Liệt kê pattern có sẵn
    #[arg(long)]
    list_patterns: bool,

    /// Tắt màu output
    #[arg(long)]
    no_color: bool,
}

#[derive(clap::Subcommand)]
enum Command {
    /// Khôi phục dữ liệu gốc từ file đã redact + file dict
    Unredact {
        /// File đã redact (đã qua AI xử lý)
        input: Vec<PathBuf>,

        /// File dict sinh ra ở bước redact (--map / GUI)
        #[arg(long)]
        dict: PathBuf,

        /// Thư mục đích (mặc định: ghi *.restored.* cạnh file gốc)
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Ghi đè lên file gốc
        #[arg(long)]
        in_place: bool,
    },
}

fn build_pattern_list(args: &Args) -> Result<Vec<Pattern>> {
    let selected: Option<Vec<&str>> = if !args.patterns.is_empty() {
        for id in &args.patterns {
            if !BUILT_IN.iter().any(|p| p.id == id) {
                bail!("pattern không tồn tại: {id} (xem --list-patterns)");
            }
        }
        Some(args.patterns.iter().map(String::as_str).collect())
    } else {
        args.level
            .map(|level| patterns::ids_for_level(level).to_vec())
    };
    let selected_set: Option<HashSet<&str>> = selected.map(|v| v.into_iter().collect());

    let mut list: Vec<Pattern> = Vec::new();
    for p in BUILT_IN {
        if !p.enabled {
            continue;
        }
        if let Some(ids) = &selected_set {
            if !ids.contains(p.id) {
                continue;
            }
        }
        list.push(p.clone());
    }

    if let Some(rules_path) = &args.rules {
        let raw = std::fs::read_to_string(rules_path)
            .with_context(|| format!("đọc rules {}", rules_path.display()))?;
        let rules: Vec<CustomRule> =
            serde_json::from_str(&raw).context("rules JSON không hợp lệ")?;
        for r in rules {
            // validate regex sớm, lỗi báo ngay kèm id rule
            regex::Regex::new(&format!("({})", r.regex))
                .with_context(|| format!("rule '{}' có regex không hợp lệ", r.id))?;
            list.push(Pattern {
                id: Box::leak(r.id.clone().into_boxed_str()),
                label: Box::leak(r.label.clone().into_boxed_str()),
                name: Box::leak(r.name.unwrap_or(r.id).into_boxed_str()),
                description: "rule tùy chỉnh",
                regex: Box::leak(r.regex.clone().into_boxed_str()),
                enabled: true,
                normalize: patterns::Normalize::None,
            });
        }
    }
    Ok(list)
}

fn collect_files(args: &Args) -> Result<(Option<PathBuf>, Vec<PathBuf>)> {
    let mut root: Option<PathBuf> = None;
    let mut files = Vec::new();
    for input in &args.input {
        if input.is_dir() {
            if root.is_none() {
                root = Some(input.clone());
            }
            let mut found = 0usize;
            let mut walker = walkdir::WalkDir::new(input)
                .into_iter()
                .filter_map(|e| e.ok());
            for entry in walker.by_ref() {
                if !args.recursive && entry.depth() > 1 {
                    continue; // không đệ quy: bỏ qua subfolder
                }
                if entry.file_type().is_file() {
                    let p = entry.into_path();
                    if formats::is_supported(&p) {
                        files.push(p);
                        found += 1;
                    }
                }
            }
            if found == 0 {
                eprintln!(
                    "{}  {} (không có file hỗ trợ: txt/csv/json/xml/pdf/docx/xlsx/pptx)",
                    style_placeholder_no_file(),
                    input.display()
                );
            }
        } else if input.is_file() {
            files.push(input.clone());
        } else {
            bail!("không tìm thấy: {}", input.display());
        }
    }
    files.sort();
    files.dedup();
    Ok((root, files))
}

/// marker cho thông báo (tránh clone Style vào collect_files)
fn style_placeholder_no_file() -> &'static str {
    "CHÚ Ý:"
}

fn resolve_out_path(args: &Args, file: &Path, root: Option<&Path>) -> PathBuf {
    if args.in_place {
        return file.to_path_buf();
    }
    let base = formats::output_path(file);
    match &args.output {
        Some(dir) => {
            let _ = std::fs::create_dir_all(dir);
            dir.join(base.file_name().unwrap_or_default())
        }
        None => {
            // xử lý folder: mirror cấu trúc vào <root>_redacted/ cạnh folder gốc
            if let Some(r) = root {
                let rel = file.strip_prefix(r).unwrap_or(file.as_os_str().as_ref());
                let folder_name = r.file_name().and_then(|n| n.to_str()).unwrap_or("folder");
                let out_root = r.with_file_name(format!("{folder_name}_redacted"));
                // giữ cấu trúc subfolder + dùng TÊN FILE đã .redacted (base)
                let file_name = base.file_name().unwrap_or_default();
                let dst = match rel.parent() {
                    Some(parent) if !parent.as_os_str().is_empty() => {
                        out_root.join(parent).join(file_name)
                    }
                    _ => out_root.join(file_name),
                };
                if let Some(parent) = dst.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                dst
            } else {
                base
            }
        }
    }
}

fn fmt_stats(stats: &RedactStats) -> String {
    if stats.total == 0 {
        return "không tìm thấy".to_string();
    }
    let parts: Vec<String> = stats
        .per_pattern
        .iter()
        .map(|(id, n)| format!("{id} {n}"))
        .collect();
    format!("{} thay thế: {}", stats.total, parts.join(", "))
}

fn main() {
    let args = Args::parse();
    let style = Style::new(args.no_color);

    if args.list_patterns {
        println!("{}", style.heading("redact-cli — pattern có sẵn"));
        println!("{}", style.rule(52));
        for p in BUILT_IN {
            let status = if p.enabled {
                ""
            } else {
                " (mặc định tắt)"
            };
            println!("{} {}", style.heading(p.id), style.muted(status));
            println!("  {} — {}", p.name, p.description);
        }
        println!(
            "{}",
            style.muted("Thêm rule riêng: --rules rules.json (xem README)")
        );
        return;
    }

    let result = match &args.command {
        Some(Command::Unredact {
            input,
            dict,
            output,
            in_place,
        }) => run_unredact(input, dict, output, *in_place, &style),
        None => run(&args, &style),
    };
    if let Err(err) = result {
        eprintln!("{} {err:#}", style.warning("LỖI:"));
        std::process::exit(1);
    }
}

fn run_unredact(
    inputs: &[PathBuf],
    dict_path: &Path,
    output_dir: &Option<PathBuf>,
    in_place: bool,
    style: &Style,
) -> Result<()> {
    if inputs.is_empty() {
        bail!("thiếu file đầu vào cho unredact");
    }
    if in_place && output_dir.is_some() {
        bail!("--in-place và --output không dùng cùng nhau");
    }
    let dict_raw = std::fs::read_to_string(dict_path)
        .with_context(|| format!("đọc dict {}", dict_path.display()))?;
    let unredactor = Unredactor::from_dict_json(&dict_raw)?;

    println!(
        "{} {} file · dict {} entry",
        style.heading("unredact"),
        inputs.len(),
        unredactor.dict_size
    );
    println!("{}", style.rule(60));

    for file in inputs {
        let result = formats::unredact_file_to(file, &unredactor);
        match result {
            Ok((formats::Processed::Text(restored), n)) => {
                let out = restored_out_path(file, output_dir, in_place);
                std::fs::write(&out, restored).with_context(|| format!("ghi {}", out.display()))?;
                println!(
                    "  {} {} {}",
                    file.display(),
                    style.muted("→"),
                    style.muted(&out.display().to_string())
                );
                println!(
                    "    {}",
                    style.number(&format!("{n} placeholder khôi phục"))
                );
                redact_cli::orglog::log_activity("unredact", &file.display().to_string());
            }
            Ok((formats::Processed::Written(out), n)) => {
                println!(
                    "  {} {} {}",
                    file.display(),
                    style.muted("→"),
                    style.muted(&out.display().to_string())
                );
                println!(
                    "    {}",
                    style.number(&format!("{n} placeholder khôi phục"))
                );
            }
            Err(err) => {
                eprintln!("  {} {}", style.warning("BỎ QUA:"), file.display());
                eprintln!("    {err:#}");
            }
        }
    }
    Ok(())
}

fn restored_out_path(file: &Path, output_dir: &Option<PathBuf>, in_place: bool) -> PathBuf {
    if in_place {
        return file.to_path_buf();
    }
    let base = formats::unredact_output_path(file);
    match output_dir {
        Some(dir) => {
            let _ = std::fs::create_dir_all(dir);
            dir.join(base.file_name().unwrap_or_default())
        }
        None => base,
    }
}

fn run(args: &Args, style: &Style) -> Result<()> {
    if args.input.is_empty() {
        bail!("thiếu file đầu vào — truyền file/thư mục hoặc xem --help");
    }
    if args.in_place && args.output.is_some() {
        bail!("--in-place và --output không dùng cùng nhau");
    }

    let pattern_list = build_pattern_list(args)?;
    let (root, files) = collect_files(args)?;
    anyhow::ensure!(!files.is_empty(), "không tìm thấy file nào để xử lý");

    let mut redactor = Redactor::new(pattern_list)?;

    println!(
        "{} {} file · {}",
        style.heading("redact-cli"),
        files.len(),
        if args.dry_run {
            style.muted("dry-run (không ghi file)")
        } else {
            style.muted("chế độ script thuần — 100% local")
        }
    );
    println!("{}", style.rule(60));

    let mut total = 0usize;
    let mut written = 0usize;
    for file in &files {
        let before = redactor.snapshot();
        let result = if args.dry_run {
            formats::dry_run_file(file, &mut redactor).map(|_| None)
        } else {
            formats::redact_file_to(file, &mut redactor).map(Some)
        };
        let file_stats = redactor.stats_since(&before);
        total += file_stats.total;
        match result {
            Ok(Some(formats::Processed::Text(content))) => {
                let out = resolve_out_path(args, file, root.as_deref());
                std::fs::write(&out, content).with_context(|| format!("ghi {}", out.display()))?;
                written += 1;
                println!(
                    "  {} {} {}",
                    file.display(),
                    style.muted("→"),
                    style.muted(&out.display().to_string())
                );
                println!("    {}", style.number(&fmt_stats(&file_stats)));
                redact_cli::orglog::log_activity("redact", &file.display().to_string());
            }
            Ok(Some(formats::Processed::Written(tmp))) => {
                // zip formats ghi cạnh file gốc — chuyển tới đúng vị trí output
                let out = resolve_out_path(args, file, root.as_deref());
                if tmp != out && std::fs::rename(&tmp, &out).is_err() {
                    std::fs::copy(&tmp, &out).with_context(|| format!("copy {}", out.display()))?;
                    let _ = std::fs::remove_file(&tmp);
                }
                written += 1;
                println!(
                    "  {} {} {}",
                    file.display(),
                    style.muted("→"),
                    style.muted(&out.display().to_string())
                );
                println!("    {}", style.number(&fmt_stats(&file_stats)));
                redact_cli::orglog::log_activity("redact", &file.display().to_string());
            }
            Ok(None) => {
                println!("  {} {}", file.display(), style.muted("(dry-run)"));
                println!("    {}", style.number(&fmt_stats(&file_stats)));
            }
            Err(err) => {
                eprintln!("  {} {}", style.warning("BỎ QUA:"), file.display());
                eprintln!("    {err:#}");
            }
        }
    }

    // folder mode: dict mặc định nằm trong <folder>_redacted/dict.json
    let mut map_path = args.map.clone();
    if let (None, false, Some(r)) = (&args.map, args.dry_run, root.as_ref()) {
        let folder_name = r.file_name().and_then(|n| n.to_str()).unwrap_or("folder");
        map_path = Some(r.with_file_name(format!("{folder_name}_redacted/dict.json")));
    }

    println!("{}", style.rule(60));
    println!(
        "{} {} thay thế · {} file ghi ra",
        style.heading("Tổng:"),
        total,
        written
    );

    if let Some(map_path) = &map_path {
        if !redactor.mapping.is_empty() {
            let entries: Vec<serde_json::Value> = redactor
                .mapping
                .iter()
                .map(|e| {
                    serde_json::json!({
                        "pattern": e.pattern,
                        "original": e.original,
                        "placeholder": e.placeholder,
                    })
                })
                .collect();
            std::fs::write(map_path, serde_json::to_string_pretty(&entries)?)
                .with_context(|| format!("ghi mapping {}", map_path.display()))?;
            eprintln!(
                "{} mapping ghi ra {} — FILE NÀY CHỨA DỮ LIỆU NHẠY CẢM",
                style.warning("CẢNH BÁO:"),
                map_path.display()
            );
        }
    }

    Ok(())
}
