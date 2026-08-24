mod capture;
mod cores;
mod roms;

use std::collections::{BTreeMap, BTreeSet};
use std::io::{Read, Write};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use clap::Parser;
use owo_colors::OwoColorize;

use cores::System;
use roms::RomFile;

#[derive(Parser)]
#[command(author, version, about)]
struct Args {
    /// System to run (e.g. snes)
    system: String,

    /// Folder containing ROM files (plain or zipped)
    roms: PathBuf,

    /// Output database root
    #[arg(long, default_value = "screenshotdb")]
    out: PathBuf,

    /// Worker threads (default: available parallelism)
    #[arg(long)]
    threads: Option<usize>,

    /// Also write a demo.gif per game (every 4th frame)
    #[arg(long)]
    gif: bool,

    /// INTERNAL ONLY
    #[arg(long, hide = true)]
    worker: bool,
}

struct RomResult {
    file_name: String,
    stem: String,
    game_id: Option<String>,
    screenshots: usize,
    skipped: usize,
    frames_run: u32,
    error: Option<String>,
}

const KEY_GAME_ID: &str = "game_id";
const KEY_SCREENSHOTS: &str = "screenshots";
const KEY_SKIPPED: &str = "skipped";
const KEY_FRAMES_RUN: &str = "frames_run";
const KEY_ERROR: &str = "error";

fn system_dir(out: &Path, system: &System) -> PathBuf {
    out.join(system.slug())
}

fn main() -> ExitCode {
    std::panic::set_hook(Box::new(|_| {}));

    let args = Args::parse();
    let outcome = if args.worker {
        self::run_worker(&args).map(|()| true)
    } else {
        self::run(&args)
    };

    match outcome {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(e) => {
            eprintln!("{} {e:#}", "error:".red().bold());
            ExitCode::FAILURE
        }
    }
}

fn run(args: &Args) -> Result<bool> {
    let Some(system) = cores::select(&args.system) else {
        bail!(
            "unknown system '{}'. available: {}",
            args.system,
            cores::available().join(", ")
        );
    };

    let config = &system.config;

    let roms = roms::scan(&args.roms, system.rom_extensions())?;
    if roms.is_empty() {
        bail!(
            "no {} ROMs (or zips) found in {}",
            system.slug(),
            args.roms.display()
        );
    }

    let system_dir = self::system_dir(&args.out, system);
    std::fs::create_dir_all(&system_dir)
        .with_context(|| format!("creating {}", system_dir.display()))?;

    let threads = args
        .threads
        .unwrap_or_else(|| std::thread::available_parallelism().map_or(1, |n| n.get()))
        .max(1);

    println!(
        "{} {} {}",
        "▶".cyan().bold(),
        system.slug().bold(),
        format!(
            "({} ROMs, {} threads, shot every {} frames, {} frames max)",
            roms.len(),
            threads,
            config.every_frames,
            config.max_frames
        )
        .dimmed()
    );

    let queue = Mutex::new(roms);
    let results = Mutex::new(Vec::new());
    let db = Mutex::new(Db::load(&system_dir));

    std::thread::scope(|scope| {
        for _ in 0..threads {
            scope.spawn(|| {
                loop {
                    let rom = queue.lock().unwrap().pop();
                    let Some(rom) = rom else {
                        return;
                    };

                    let result = self::run_rom(&rom, system, args);
                    self::print_result(&result);

                    if let Some(game_id) = &result.game_id {
                        let mut db = db.lock().unwrap();
                        db.titlemap.insert(game_id.clone(), result.stem.clone());
                        db.gamelist.insert(result.file_name.clone());

                        if let Err(e) = db.write(&system_dir) {
                            eprintln!("{} {e:#}", "error:".red().bold());
                        }
                    }

                    results.lock().unwrap().push(result);
                }
            });
        }
    });

    let results = results.into_inner().unwrap();
    self::print_summary(&results);

    Ok(results.iter().any(|r| r.screenshots > 0))
}

fn run_rom(rom: &RomFile, system: &System, args: &Args) -> RomResult {
    let mut result = RomResult {
        file_name: rom.file_name.clone(),
        stem: rom.stem.clone(),
        game_id: None,
        screenshots: 0,
        skipped: 0,
        frames_run: 0,
        error: None,
    };

    let exe = match std::env::current_exe() {
        Ok(exe) => exe,
        Err(e) => {
            result.error = Some(format!("resolving own executable: {e}"));
            return result;
        }
    };

    let mut command = Command::new(exe);
    command
        .arg(system.slug())
        .arg(&rom.path)
        .arg("--worker")
        .arg("--out")
        .arg(&args.out);
    if args.gif {
        command.arg("--gif");
    }
    let child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn();
    let mut child = match child {
        Ok(child) => child,
        Err(e) => {
            result.error = Some(format!("spawning worker: {e}"));
            return result;
        }
    };

    let deadline = Instant::now() + system.timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) if Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                break None;
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(50)),
            Err(e) => {
                result.error = Some(format!("waiting for worker: {e}"));
                let _ = child.kill();
                let _ = child.wait();
                return result;
            }
        }
    };

    let mut stdout = String::new();
    if let Some(mut pipe) = child.stdout.take() {
        let _ = pipe.read_to_string(&mut stdout);
    }
    let mut stderr = String::new();
    if let Some(mut pipe) = child.stderr.take() {
        let _ = pipe.read_to_string(&mut stderr);
    }

    for line in stdout.lines() {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        match key {
            KEY_GAME_ID => result.game_id = Some(value.to_string()),
            KEY_SCREENSHOTS => result.screenshots = value.parse().unwrap_or(0),
            KEY_SKIPPED => result.skipped = value.parse().unwrap_or(0),
            KEY_FRAMES_RUN => result.frames_run = value.parse().unwrap_or(0),
            KEY_ERROR => result.error = Some(value.to_string()),
            _ => {}
        }
    }

    match status {
        None => {
            result.error = Some(format!("timed out after {}s", system.timeout.as_secs()));
        }
        Some(status) if !status.success() && result.error.is_none() => {
            let detail = stderr
                .lines()
                .find(|l| !l.trim().is_empty())
                .map(|l| format!(": {}", l.trim()))
                .unwrap_or_default();
            result.error = Some(format!("crashed ({status}){detail}"));
        }
        Some(_) => {}
    }

    result
}

fn run_worker(args: &Args) -> Result<()> {
    let Some(system) = cores::select(&args.system) else {
        bail!("unknown system '{}'", args.system);
    };

    let stdout = std::io::stdout();
    let mut out = stdout.lock();

    let bytes = match roms::load(&args.roms, system.rom_extensions()) {
        Ok(bytes) => bytes,
        Err(e) => return self::emit_error(&mut out, &format!("{e:#}")),
    };

    let mut core = match catch_unwind(AssertUnwindSafe(|| system.build(bytes))) {
        Ok(Ok(core)) => core,
        Ok(Err(e)) => return self::emit_error(&mut out, &format!("{e:#}")),
        Err(_) => return self::emit_error(&mut out, "panicked during boot"),
    };

    let game_id = core.game_id();
    writeln!(out, "{KEY_GAME_ID}={game_id}")?;
    out.flush()?;

    let out_dir = self::system_dir(&args.out, system).join(&game_id);
    let capture = match capture::run(core.as_mut(), &system.config, &out_dir, args.gif) {
        Ok(capture) => capture,
        Err(e) => return self::emit_error(&mut out, &format!("{e:#}")),
    };

    writeln!(out, "{KEY_SCREENSHOTS}={}", capture.screenshots)?;
    writeln!(out, "{KEY_SKIPPED}={}", capture.skipped)?;
    writeln!(out, "{KEY_FRAMES_RUN}={}", capture.frames_run)?;
    if let Some(message) = capture.panic {
        let message = format!("panicked @ frame {}: {message}", capture.frames_run);
        return self::emit_error(&mut out, &message);
    }

    Ok(())
}

fn emit_error(out: &mut impl Write, message: &str) -> Result<()> {
    writeln!(out, "{KEY_ERROR}={}", self::one_line(message))?;
    Ok(())
}

fn one_line(message: &str) -> String {
    message.replace('\n', "; ")
}

#[derive(Default)]
struct Db {
    titlemap: BTreeMap<String, String>,
    gamelist: BTreeSet<String>,
}

impl Db {
    fn load(system_dir: &Path) -> Self {
        let mut db = Self::default();

        if let Ok(existing) = std::fs::read_to_string(system_dir.join("titlemap.txt")) {
            for line in existing.lines() {
                if let Some((id, title)) = line.split_once('=') {
                    db.titlemap.insert(id.to_string(), title.to_string());
                }
            }
        }
        if let Ok(existing) = std::fs::read_to_string(system_dir.join("gamelist.txt")) {
            db.gamelist.extend(existing.lines().map(str::to_string));
        }

        db
    }

    fn write(&self, system_dir: &Path) -> Result<()> {
        let path = system_dir.join("titlemap.txt");
        let mut content = String::new();
        for (id, title) in &self.titlemap {
            content.push_str(&format!("{id}={title}\n"));
        }
        std::fs::write(&path, content).with_context(|| format!("writing {}", path.display()))?;

        let path = system_dir.join("gamelist.txt");
        let mut content = String::new();
        for file_name in &self.gamelist {
            content.push_str(file_name);
            content.push('\n');
        }
        std::fs::write(&path, content).with_context(|| format!("writing {}", path.display()))
    }
}

fn print_result(result: &RomResult) {
    let stem = result.stem.bold();
    let stats = format!(
        "{} shots · {} frames",
        result.screenshots, result.frames_run
    );

    match &result.error {
        None => println!("  {}  {stem}  {}", "PASS".green().bold(), stats.dimmed()),
        Some(error) => println!(
            "  {}  {stem}  {}",
            "FAIL".red().bold(),
            format!("{error} · {stats}").dimmed()
        ),
    }
}

fn print_summary(results: &[RomResult]) {
    let clean = results.iter().filter(|r| r.error.is_none()).count();
    let crashed = results
        .iter()
        .filter(|r| r.game_id.is_some() && r.error.is_some())
        .count();
    let unloadable = results.iter().filter(|r| r.game_id.is_none()).count();
    let screenshots: usize = results.iter().map(|r| r.screenshots).sum();
    let skipped: usize = results.iter().map(|r| r.skipped).sum();

    println!(
        "  {}  {}   {}   {}   {}",
        format!("{clean}/{} ROMs ok", results.len()).bold(),
        format!("{crashed} crashed").red(),
        format!("{unloadable} unloadable").yellow(),
        format!("{screenshots} screenshots").green(),
        format!("{skipped} dupes/blanks skipped").dimmed(),
    );
}
