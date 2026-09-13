use anyhow::{anyhow, Context, Result};
use chrono::Local;
use clap::{Parser, Subcommand};
use ini::Ini;
use md5::{Digest, Md5};
use rand::seq::SliceRandom;
use rand::thread_rng;
use serde_json::Value;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration as StdDuration, SystemTime};
use tracing::info;
use users::get_current_uid;

const BING_JSON_ENDPOINT: &str = "https://www.bing.com/HPImageArchive.aspx?format=js&idx=0&n=1";
const BING_IMAGE_ENDPOINT: &str = "https://www.bing.com";
const REGIONS: &[&str] = &[
    "en-US", "en-GB", "en-AU", "en-CA", "de-DE", "fr-FR", "ja-JP", "zh-CN", "pt-BR",
    "es-ES", "it-IT", "en-IN",
];

#[derive(Parser)]
#[command(name = "bing_wallpaper")]
#[command(about = "Bing wallpaper fetcher for macOS")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Run one update cycle (default)
    Run,
    /// Remove byte-duplicate images
    Prune,
    /// Print config and archive status
    Status,
    /// Show or set config values
    Config {
        #[arg(long)]
        show: bool,
        #[arg(long, value_name = "KEY=VALUE")]
        set: Vec<String>,
    },
    /// Create default config and install the LaunchAgent
    Init,
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

    fn set(&mut self, key: &str, value: &str) -> Result<()> {
        match key {
            "RESOLUTION" => self.resolution = value.into(),
            "AUTO_CLEANUP" => self.auto_cleanup = value.eq_ignore_ascii_case("true"),
            "CLEANUP_DAYS" => self.cleanup_days = value.parse().context("invalid CLEANUP_DAYS")?,
            "SAVE_PATH" => self.save_path = expand_path(value),
            "REGION_MODE" => self.region_mode = value.into(),
            "REGION" => self.region = value.into(),
            _ => return Err(anyhow!("unknown config key: {}", key)),
        }
        Ok(())
    }

    fn print(&self) {
        println!("RESOLUTION={}", self.resolution);
        println!("AUTO_CLEANUP={}", self.auto_cleanup);
        println!("CLEANUP_DAYS={}", self.cleanup_days);
        println!("SAVE_PATH={}", self.save_path.display());
        println!("REGION_MODE={}", self.region_mode);
        println!("REGION={}", self.region);
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
    let conf = Ini::load_from_file(&path)?;
    if let Some(props) = conf.section(None::<&str>) {
        if let Some(v) = props.get("RESOLUTION") {
            cfg.resolution = v.to_string();
        }
        if let Some(v) = props.get("AUTO_CLEANUP") {
            cfg.auto_cleanup = v.eq_ignore_ascii_case("true");
        }
        if let Some(v) = props.get("CLEANUP_DAYS") {
            cfg.cleanup_days = v.parse().context("invalid CLEANUP_DAYS")?;
        }
        if let Some(v) = props.get("SAVE_PATH") {
            cfg.save_path = expand_path(v);
        }
        if let Some(v) = props.get("REGION_MODE") {
            cfg.region_mode = v.to_string();
        }
        if let Some(v) = props.get("REGION") {
            cfg.region = v.to_string();
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
    let (w, h) = rdev::display_size().ok()?;
    Some((w as u32, h as u32))
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
    urlbase
        .split('_')
        .next()
        .unwrap_or(urlbase)
        .to_string()
}

fn parse_meta(json: &Value, region: &str, res_name: &str) -> Option<ImageMeta> {
    let img = json.get("images")?.get(0)?;
    let urlbase = img.get("urlbase")?.as_str()?;
    let startdate = img.get("startdate")?.as_str()?.to_string();
    let title = img.get("title")?.as_str().unwrap_or("").to_string();
    let copyright = img.get("copyright")?.as_str().unwrap_or("").to_string();
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
                info!("new image found: {} {}", region, meta.content_key);
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
        info!("already on disk: {} ({})", meta.region, filename);
        info!("title: {}", meta.title);
        info!("copyright: {}", meta.copyright);
        return Ok(());
    }

    info!("downloading {}", meta.url);
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

    info!("saved {}", path.display());
    info!("resolution: {}", resolution_name(&meta.resolution));
    info!("title: {}", meta.title);
    info!("copyright: {}", meta.copyright);
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
                        info!("cleaned up {}", path.display());
                    }
                }
            }
        }
    }
    Ok(())
}

fn prune_duplicates(save_path: &Path) -> Result<usize> {
    let mut seen = HashMap::<String, PathBuf>::new();
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
            info!("removed duplicate {}", path.display());
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
    let mut rng = thread_rng();
    Ok(entries.choose(&mut rng).unwrap().clone())
}

fn set_random_wallpaper(save_path: &Path) -> Result<()> {
    let path = pick_random_wallpaper(save_path)?;
    let path_str = path.to_string_lossy().into_owned();
    info!("setting desktop to {}", path.display());
    wallpaper::set_from_path(&path_str).map_err(|e| anyhow!("wallpaper error: {}", e))?;
    Ok(())
}

async fn run() -> Result<()> {
    let cfg = load_config()?;
    cfg.ensure_dirs()?;
    save_config(&cfg)?;

    let client = reqwest::Client::builder()
        .connect_timeout(StdDuration::from_secs(10))
        .timeout(StdDuration::from_secs(60))
        .build()?;

    let seen = load_seen()?;
    info!("starting wallpaper update");
    if let Some(meta) = find_new_image(&client, &cfg, &seen).await? {
        download_image(&client, &meta, &cfg.save_path).await?;
    } else {
        info!("no new wallpapers today");
    }

    if cfg.auto_cleanup {
        cleanup_old_files(&cfg.save_path, cfg.cleanup_days)?;
    }

    set_random_wallpaper(&cfg.save_path)?;
    info!("done");
    Ok(())
}

fn status() -> Result<()> {
    let cfg = load_config()?;
    let save_path = &cfg.save_path;
    cfg.ensure_dirs()?;

    let files: Vec<_> = fs::read_dir(save_path)?
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path().extension().and_then(|x| x.to_str()) == Some("jpg")
                && e.file_name().to_str().map(|n| n.starts_with("bing_")).unwrap_or(false)
        })
        .collect();

    let mut hashes = HashSet::new();
    for e in &files {
        if let Ok(bytes) = fs::read(e.path()) {
            let mut hasher = Md5::new();
            hasher.update(&bytes);
            let _ = hashes.insert(format!("{:x}", hasher.finalize()));
        }
    }

    let seen = load_seen().unwrap_or_default();

    println!("Config");
    cfg.print();
    println!();
    println!("Archive: {}", save_path.display());
    println!("  images: {}", files.len());
    println!("  unique: {}", hashes.len());
    println!("  seen keys: {}", seen.len());
    Ok(())
}

fn cmd_config(show: bool, sets: Vec<String>) -> Result<()> {
    let mut cfg = load_config()?;
    cfg.ensure_dirs()?;

    if !sets.is_empty() {
        for kv in &sets {
            let Some((k, v)) = kv.split_once('=') else {
                return Err(anyhow!("expected KEY=VALUE, got: {}", kv));
            };
            let k = k.trim();
            let v = v.trim();
            cfg.set(k, v)?;
        }
        save_config(&cfg)?;
    }

    if show || sets.is_empty() {
        cfg.print();
    }
    Ok(())
}

fn cmd_init() -> Result<()> {
    let cfg = Config::default();
    cfg.ensure_dirs()?;
    let config_path = config_file()?;
    if !config_path.exists() {
        save_config(&cfg)?;
        info!("created default config at {}", config_path.display());
    } else {
        info!("config already exists at {}", config_path.display());
    }

    let home = home_dir();
    let bin = home.join(".local").join("bin").join("bing_wallpaper");
    let plist_path = home
        .join("Library")
        .join("LaunchAgents")
        .join("com.masrurimz.bingwallpaper.plist");
    fs::create_dir_all(plist_path.parent().unwrap())?;

    let plist = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.masrurimz.bingwallpaper</string>
    <key>ProgramArguments</key>
    <array>
        <string>{}</string>
        <string>run</string>
    </array>
    <key>StartInterval</key>
    <integer>3600</integer>
    <key>StandardErrorPath</key>
    <string>{}</string>
    <key>StandardOutPath</key>
    <string>{}</string>
    <key>RunAtLoad</key>
    <true/>
</dict>
</plist>
"#,
        bin.display(),
        config_dir()?.join("bing_wallpaper.err").display(),
        config_dir()?.join("bing_wallpaper.out").display(),
    );
    fs::write(&plist_path, plist)?;
    info!("wrote {}", plist_path.display());

    let _ = Command::new("launchctl")
        .args([
            "bootout",
            &format!("gui/{}", get_current_uid()),
            &plist_path.to_string_lossy(),
        ])
        .output();
    let status = Command::new("launchctl")
        .args([
            "bootstrap",
            &format!("gui/{}", get_current_uid()),
            &plist_path.to_string_lossy(),
        ])
        .status()?;
    if !status.success() {
        return Err(anyhow!("launchctl bootstrap failed"));
    }
    info!("LaunchAgent loaded");
    println!("Make sure {} exists and is executable.", bin.display());
    Ok(())
}

fn cmd_prune() -> Result<()> {
    let cfg = load_config()?;
    let removed = prune_duplicates(&cfg.save_path)?;
    println!("Pruned {} duplicate(s)", removed);
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(filter).init();

    let cli = Cli::parse();
    match cli.command.unwrap_or(Commands::Run) {
        Commands::Run => run().await,
        Commands::Prune => cmd_prune(),
        Commands::Status => status(),
        Commands::Config { show, set } => cmd_config(show, set),
        Commands::Init => cmd_init(),
    }
}
