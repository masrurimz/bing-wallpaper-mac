use anyhow::{anyhow, Result};
use chrono::Local;
use clap::Parser;
use md5::{Digest, Md5};
use serde_json::Value;
use std::collections::BTreeSet;
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration as StdDuration, SystemTime};

const BING_JSON_ENDPOINT: &str = "https://www.bing.com/HPImageArchive.aspx?format=js&idx=0&n=1";
const BING_IMAGE_ENDPOINT: &str = "https://www.bing.com";
const REGIONS: &[&str] = &[
    "en-US", "en-GB", "en-AU", "en-CA", "de-DE", "fr-FR", "ja-JP", "zh-CN", "pt-BR",
    "es-ES", "it-IT", "en-IN",
];

#[derive(Parser)]
#[command(name = "bing_wallpaper")]
#[command(about = "Bing wallpaper fetcher — shuffles and sets the desktop")]
struct Cli {
    /// Kept for LaunchAgent compatibility; ignored.
    #[arg(long, hide = true)]
    force: bool,
    /// Remove byte-duplicate images from the archive.
    #[arg(long)]
    prune: bool,
}

#[derive(Debug, Clone)]
struct Config {
    resolution: String,
    auto_cleanup: bool,
    cleanup_days: i64,
    save_path: PathBuf,
    region_mode: String,
    region: String,
}

impl Default for Config {
    fn default() -> Self {
        let home = home_dir();
        Config {
            resolution: "UHD".into(),
            auto_cleanup: true,
            cleanup_days: 14,
            save_path: home.join(".wallpapers"),
            region_mode: "cycle".into(),
            region: "en-US".into(),
        }
    }
}

impl Config {
    fn ensure_dirs(&self) -> Result<()> {
        fs::create_dir_all(&self.save_path)?;
        fs::create_dir_all(config_dir()?)?;
        Ok(())
    }
}

#[derive(Debug, Clone)]
struct ImageMeta {
    region: String,
    startdate: String,
    content_key: String,
    title: String,
    copyright: String,
    resolution: String,
    url: String,
}

fn home_dir() -> PathBuf {
    dirs::home_dir()
        .or_else(|| std::env::var_os("HOME").map(PathBuf::from))
        .expect("could not determine home directory")
}

fn config_dir() -> Result<PathBuf> {
    Ok(home_dir().join(".config").join("bing-wallpaper"))
}

fn config_file() -> Result<PathBuf> {
    Ok(config_dir()?.join("config"))
}

fn seen_file() -> Result<PathBuf> {
    Ok(config_dir()?.join("seen_urlbases"))
}

fn load_config() -> Result<Config> {
    let mut cfg = Config::default();
    let path = config_file()?;
    if !path.exists() {
        return Ok(cfg);
    }
    let f = File::open(&path)?;
    let r = BufReader::new(f);
    for line in r.lines() {
        let line = line?;
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if let Some((k, v)) = trimmed.split_once('=') {
            let k = k.trim();
            let v = v.trim().trim_matches('"').trim_matches('\'').to_string();
            match k {
                "RESOLUTION" => cfg.resolution = v,
                "AUTO_CLEANUP" => cfg.auto_cleanup = v.eq_ignore_ascii_case("true"),
                "CLEANUP_DAYS" => cfg.cleanup_days = v.parse().unwrap_or(14),
                "SAVE_PATH" => cfg.save_path = expand_path(&v),
                "REGION_MODE" => cfg.region_mode = v,
                "REGION" => cfg.region = v,
                _ => {}
            }
        }
    }
    Ok(cfg)
}

fn save_config(cfg: &Config) -> Result<()> {
    let path = config_file()?;
    fs::create_dir_all(path.parent().unwrap())?;
    let mut f = File::create(&path)?;
    writeln!(f, "RESOLUTION={}", cfg.resolution)?;
    writeln!(f, "AUTO_CLEANUP={}", cfg.auto_cleanup)?;
    writeln!(f, "CLEANUP_DAYS={}", cfg.cleanup_days)?;
    writeln!(f, "SAVE_PATH={}", cfg.save_path.display())?;
    writeln!(f, "REGION_MODE={}", cfg.region_mode)?;
    writeln!(f, "REGION={}", cfg.region)?;
    Ok(())
}

fn expand_path(s: &str) -> PathBuf {
    if s.starts_with("~/") {
        home_dir().join(&s[2..])
    } else if s.starts_with("$HOME/") {
        home_dir().join(&s[6..])
    } else {
        PathBuf::from(s)
    }
}

fn load_seen() -> Result<BTreeSet<String>> {
    let path = seen_file()?;
    if !path.exists() {
        return Ok(BTreeSet::new());
    }
    let f = File::open(&path)?;
    let r = BufReader::new(f);
    Ok(r
        .lines()
        .filter_map(|l| l.ok())
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect())
}

fn append_seen(content_key: &str) -> Result<()> {
    let path = seen_file()?;
    fs::create_dir_all(path.parent().unwrap())?;
    let mut f = OpenOptions::new().create(true).append(true).open(&path)?;
    writeln!(f, "{}", content_key)?;
    Ok(())
}

fn resolution_value(res: &str) -> &str {
    match res {
        "UHD" => "UHD",
        "FHD" => "1920x1080",
        "HD" => "1366x768",
        _ => "UHD",
    }
}

fn resolution_name(res: &str) -> &str {
    match res {
        "UHD" => "4K (3840x2160)",
        "FHD" => "Full HD (1920x1080)",
        "HD" => "HD (1366x768)",
        _ => "4K (3840x2160)",
    }
}

fn get_screen_resolution() -> Option<(u32, u32)> {
    let out = Command::new("system_profiler")
        .args(["SPDisplaysDataType", "-xml"])
        .output()
        .ok()?;
    let text = String::from_utf8(out.stdout).ok()?;
    for token in text.split(|c: char| !c.is_ascii_digit() && c != 'x') {
        if let Some((w, h)) = token.split_once('x') {
            if let (Ok(w), Ok(h)) = (w.parse::<u32>(), h.parse::<u32>()) {
                return Some((w, h));
            }
        }
    }
    None
}

fn pick_resolution(cfg: &Config) -> String {
    if cfg.resolution == "auto" {
        if let Some((w, _)) = get_screen_resolution() {
            if w >= 3840 {
                "UHD".into()
            } else if w >= 1920 {
                "FHD".into()
            } else {
                "HD".into()
            }
        } else {
            "UHD".into()
        }
    } else {
        cfg.resolution.clone()
    }
}

async fn fetch_json(client: &reqwest::Client, region: &str) -> Result<Value> {
    let url = format!("{}&mkt={}", BING_JSON_ENDPOINT, region);
    let json: Value = client
        .get(&url)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    Ok(json)
}

fn content_key(urlbase: &str) -> String {
    // /th?id=OHR.MisurinaPeak_PT-BR1204765846 -> /th?id=OHR.MisurinaPeak
    urlbase
        .split('_')
        .next()
        .unwrap_or(urlbase)
        .to_string()
}

fn parse_meta(json: &Value, region: &str, res_name: &str) -> Option<ImageMeta> {
    let img = json.get("images")?.get(0)?;
    let startdate = img.get("startdate")?.as_str()?.to_string();
    let title = img.get("title")?.as_str().unwrap_or("").to_string();
    let copyright = img.get("copyright")?.as_str().unwrap_or("").to_string();
    let urlbase = img.get("urlbase")?.as_str()?;
    let res_val = resolution_value(res_name);
    let url = format!("{}{}_{}.jpg", BING_IMAGE_ENDPOINT, urlbase, res_val);
    Some(ImageMeta {
        region: region.to_string(),
        startdate,
        content_key: content_key(urlbase),
        title,
        copyright,
        resolution: res_name.to_string(),
        url,
    })
}

async fn find_new_image(
    client: &reqwest::Client,
    cfg: &Config,
    seen: &BTreeSet<String>,
) -> Result<Option<ImageMeta>> {
    let today = Local::now().format("%Y%m%d").to_string();
    let res_name = pick_resolution(cfg);

    if cfg.region_mode == "single" {
        let json = fetch_json(client, &cfg.region).await?;
        if let Some(meta) = parse_meta(&json, &cfg.region, &res_name) {
            if meta.startdate == today && !seen.contains(&meta.content_key) {
                return Ok(Some(meta));
            }
        }
        return Ok(None);
    }

    for region in REGIONS {
        let json = fetch_json(client, region).await?;
        if let Some(meta) = parse_meta(&json, region, &res_name) {
            if meta.startdate == today && !seen.contains(&meta.content_key) {
                return Ok(Some(meta));
            }
        }
    }
    Ok(None)
}

fn has_existing_image(save_path: &Path, startdate: &str, region: &str, res: &str) -> bool {
    let new_name = format!("bing_{}_{}_{}.jpg", startdate, region, res);
    if save_path.join(&new_name).exists() {
        return true;
    }

    // Old bash script naming: bing_{startdate}_{hhmm}_{region}_{res}.jpg
    let prefix = format!("bing_{}_", startdate);
    let suffix = format!("_{}_{}.jpg", region, res);
    if let Ok(entries) = fs::read_dir(save_path) {
        for entry in entries {
            if let Ok(e) = entry {
                if let Some(name) = e.file_name().to_str() {
                    if name.starts_with(&prefix) && name.ends_with(&suffix) {
                        return true;
                    }
                }
            }
        }
    }
    false
}

async fn download_image(client: &reqwest::Client, meta: &ImageMeta, save_path: &Path) -> Result<()> {
    let res_val = resolution_value(&meta.resolution);
    let filename = format!("bing_{}_{}_{}.jpg", meta.startdate, meta.region, res_val);
    let path = save_path.join(&filename);
    let txt_path = save_path.join(format!("bing_{}_{}_{}.txt", meta.startdate, meta.region, res_val));

    if has_existing_image(save_path, &meta.startdate, &meta.region, res_val) {
        append_seen(&meta.content_key)?;
        println!(
            "Already on disk: {} {} ({})",
            meta.region, meta.startdate, filename
        );
        println!("Title: {}", meta.title);
        println!("Copyright: {}", meta.copyright);
        return Ok(());
    }

    let bytes = client
        .get(&meta.url)
        .send()
        .await?
        .error_for_status()?
        .bytes()
        .await?;
    fs::write(&path, &bytes)?;

    let mut f = File::create(&txt_path)?;
    writeln!(f, "{}", meta.title)?;
    writeln!(f, "{}", meta.copyright)?;
    append_seen(&meta.content_key)?;

    println!("Downloaded {}", path.display());
    println!("Resolution: {}", resolution_name(&meta.resolution));
    println!("Title: {}", meta.title);
    println!("Copyright: {}", meta.copyright);
    Ok(())
}

fn cleanup_old_files(save_path: &Path, days: i64) -> Result<()> {
    let threshold = days as u64 * 86_400;
    for entry in fs::read_dir(save_path)? {
        let entry = entry?;
        let path = entry.path();
        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            if !name.starts_with("bing_") {
                continue;
            }
            let meta = entry.metadata()?;
            if let Ok(modified) = meta.modified() {
                if let Ok(age) = SystemTime::now().duration_since(modified) {
                    if age.as_secs() > threshold {
                        fs::remove_file(&path)?;
                        println!("Cleaned up {}", path.display());
                    }
                }
            }
        }
    }
    Ok(())
}

fn prune_duplicates(save_path: &Path) -> Result<usize> {
    let mut seen = std::collections::HashMap::<String, PathBuf>::new();
    let mut removed = 0usize;
    for entry in fs::read_dir(save_path)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("jpg") {
            continue;
        }
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if !name.starts_with("bing_") {
            continue;
        }
        let mut hasher = Md5::new();
        hasher.update(&fs::read(&path)?);
        let hash = format!("{:x}", hasher.finalize());

        if seen.contains_key(&hash) {
            fs::remove_file(&path)?;
            let mut sidecar = path.clone();
            sidecar.set_extension("txt");
            if sidecar.exists() {
                fs::remove_file(&sidecar)?;
            }
            removed += 1;
            println!("Removed duplicate {}", path.display());
        } else {
            seen.insert(hash, path);
        }
    }
    Ok(removed)
}

fn pick_random_wallpaper(save_path: &Path) -> Result<PathBuf> {
    let entries: Vec<_> = fs::read_dir(save_path)?
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path().extension().and_then(|x| x.to_str()) == Some("jpg")
                && e.file_name().to_str().map(|n| n.starts_with("bing_")).unwrap_or(false)
        })
        .map(|e| e.path())
        .collect();
    if entries.is_empty() {
        return Err(anyhow!("no wallpapers in {}", save_path.display()));
    }
    let idx = (SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)?
        .as_secs() as usize)
        % entries.len();
    Ok(entries[idx].clone())
}

async fn set_random_wallpaper(save_path: &Path) -> Result<()> {
    let path = pick_random_wallpaper(save_path)?;
    let path_str = path.canonicalize()?.to_string_lossy().into_owned();
    let script = format!(
        r#"tell application "System Events"
    set picFile to POSIX file "{}"
    repeat with i from 1 to count of desktops
        tell desktop i to set picture to picFile
    end repeat
end tell"#,
        path_str
    );
    let out = tokio::task::spawn_blocking(move || {
        let out = std::process::Command::new("osascript")
            .arg("-e")
            .arg(&script)
            .output()?;
        if !out.status.success() {
            return Err(anyhow!(
                "osascript failed: {}",
                String::from_utf8_lossy(&out.stderr)
            ));
        }
        Ok(())
    })
    .await?;
    out?;
    println!("Set desktop to {}", path.display());
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let cfg = load_config()?;
    cfg.ensure_dirs()?;
    save_config(&cfg)?;

    if cli.prune {
        let removed = prune_duplicates(&cfg.save_path)?;
        println!("Pruned {} duplicate(s)", removed);
        return Ok(());
    }

    if cli.force {
        println!("Note: --force is ignored; the Rust service deduplicates and rotates the desktop.");
    }

    let client = reqwest::Client::builder()
        .connect_timeout(StdDuration::from_secs(10))
        .timeout(StdDuration::from_secs(60))
        .build()?;

    let seen = load_seen()?;
    println!("Starting Bing wallpaper update...");
    if let Some(meta) = find_new_image(&client, &cfg, &seen).await? {
        download_image(&client, &meta, &cfg.save_path).await?;
    } else {
        println!("No new wallpapers today.");
    }

    if cfg.auto_cleanup {
        cleanup_old_files(&cfg.save_path, cfg.cleanup_days)?;
    }

    set_random_wallpaper(&cfg.save_path).await?;
    Ok(())
}
