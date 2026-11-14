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
    after_help = "Chế độ an toàn tuyệt đối: tool này KHÔNG có bất kỳ tính năng network nào.\nMọi xử lý diễn ra trên máy bạn.\n\nVí dụ:\n  redact-cli --level 2 baocao.pdf\n  redact-cli --dry-run baocao.pdf\n  redact-cli unredact baocao.redacted.txt --dict baocao.dict.json"
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

fn collect_files(args: &Args) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    for input in &args.input {
        if input.is_dir() {
            if args.recursive {
                for entry in walkdir::WalkDir::new(input)
                    .into_iter()
                    .filter_map(|e| e.ok())
                {
                    if entry.file_type().is_file() {
                        files.push(entry.into_path());
                    }
                }
            } else {
                for entry in std::fs::read_dir(input)
                    .with_context(|| format!("đọc thư mục {}", input.display()))?
                {
                    let entry = entry?;
                    let path = entry.path();
                    if path.is_file() {
                        files.push(path);
                    }
                }
            }
        } else if input.is_file() {
            files.push(input.clone());
        } else {
            bail!("không tìm thấy: {}", input.display());
        }
    }
    files.sort();
    files.dedup();
    Ok(files)
}

fn resolve_out_path(args: &Args, file: &Path) -> PathBuf {
    if args.in_place {
        return file.to_path_buf();
    }
    let base = formats::output_path(file);
    match &args.output {
        Some(dir) => {
            let _ = std::fs::create_dir_all(dir);
            dir.join(base.file_name().unwrap_or_default())
        }
        None => base,
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
        let raw =
            std::fs::read_to_string(file).with_context(|| format!("đọc {}", file.display()))?;
        let (restored, n) = unredactor.unredact(&raw);
        let out = if in_place {
            file.clone()
        } else {
            let stem = file.file_stem().and_then(|s| s.to_str()).unwrap_or("out");
            // bỏ đuôi .redacted nếu có: baocao.redacted.txt → baocao.restored.txt
            let stem = stem.strip_suffix(".redacted").unwrap_or(stem);
            let name = match file.extension().and_then(|e| e.to_str()) {
                Some(e) => format!("{stem}.restored.{e}"),
                None => format!("{stem}.restored"),
            };
            let base = file.with_file_name(name);
            match output_dir {
                Some(dir) => {
                    let _ = std::fs::create_dir_all(dir);
                    dir.join(base.file_name().unwrap_or_default())
                }
                None => base,
            }
        };
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
    }
    Ok(())
}

fn run(args: &Args, style: &Style) -> Result<()> {
    if args.input.is_empty() {
        bail!("thiếu file đầu vào — truyền file/thư mục hoặc xem --help");
    }
    if args.in_place && args.output.is_some() {
        bail!("--in-place và --output không dùng cùng nhau");
    }

    let pattern_list = build_pattern_list(args)?;
    let files = collect_files(args)?;
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
            formats::redact_file(file, &mut redactor).map(Some)
        };
        let file_stats = redactor.stats_since(&before);
        total += file_stats.total;
        match result {
            Ok(Some(content)) => {
                let out = resolve_out_path(args, file);
                std::fs::write(&out, content).with_context(|| format!("ghi {}", out.display()))?;
                written += 1;
                println!(
                    "  {} {} {}",
                    file.display(),
                    style.muted("→"),
                    style.muted(&out.display().to_string())
                );
                println!("    {}", style.number(&fmt_stats(&file_stats)));
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

    println!("{}", style.rule(60));
    println!(
        "{} {} thay thế · {} file ghi ra",
        style.heading("Tổng:"),
        total,
        written
    );

    if let Some(map_path) = &args.map {
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
