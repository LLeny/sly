mod capture;
mod cores;
mod roms;

use std::collections::{BTreeMap, BTreeSet};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Mutex;

use anyhow::{Context, Result, bail};
use clap::Parser;
use owo_colors::OwoColorize;

use capture::CaptureConfig;
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

fn main() -> ExitCode {
    std::panic::set_hook(Box::new(|_| {}));

    let args = Args::parse();
    match self::run(&args) {
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

    let roms = roms::scan(&args.roms, system.extensions)?;
    if roms.is_empty() {
        bail!(
            "no {} ROMs (or zips) found in {}",
            system.name,
            args.roms.display()
        );
    }

    let system_dir = args.out.join(system.name);
    std::fs::create_dir_all(&system_dir)
        .with_context(|| format!("creating {}", system_dir.display()))?;

    let threads = args
        .threads
        .unwrap_or_else(|| std::thread::available_parallelism().map_or(1, |n| n.get()))
        .max(1);

    println!(
        "{} {} {}",
        "▶".cyan().bold(),
        system.name.bold(),
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

    std::thread::scope(|scope| {
        for _ in 0..threads {
            scope.spawn(|| {
                loop {
                    let rom = queue.lock().unwrap().pop();
                    let Some(rom) = rom else {
                        return;
                    };

                    let result = self::run_rom(&rom, system, config, &system_dir);
                    self::print_result(&result);
                    results.lock().unwrap().push(result);
                }
            });
        }
    });

    let results = results.into_inner().unwrap();
    self::write_titlemap(&system_dir, &results)?;
    self::write_gamelist(&system_dir, &results)?;
    self::print_summary(&results);

    Ok(results.iter().any(|r| r.screenshots > 0))
}

fn run_rom(rom: &RomFile, system: &System, config: &CaptureConfig, system_dir: &Path) -> RomResult {
    let mut result = RomResult {
        file_name: rom.file_name.clone(),
        stem: rom.stem.clone(),
        game_id: None,
        screenshots: 0,
        skipped: 0,
        frames_run: 0,
        error: None,
    };

    let bytes = match roms::load(&rom.path, system.extensions) {
        Ok(bytes) => bytes,
        Err(e) => {
            result.error = Some(format!("{e:#}"));
            return result;
        }
    };

    let mut core = match catch_unwind(AssertUnwindSafe(|| system.build(bytes))) {
        Ok(Ok(core)) => core,
        Ok(Err(e)) => {
            result.error = Some(format!("{e:#}"));
            return result;
        }
        Err(_) => {
            result.error = Some("panicked during boot".to_string());
            return result;
        }
    };

    let game_id = core.game_id();
    let out_dir = system_dir.join(&game_id);
    result.game_id = Some(game_id);

    match capture::run(core.as_mut(), config, &out_dir) {
        Ok(capture) => {
            result.screenshots = capture.screenshots;
            result.skipped = capture.skipped;
            result.frames_run = capture.frames_run;
            result.error = capture
                .panic
                .map(|message| format!("panicked @ frame {}: {message}", capture.frames_run));
        }
        Err(e) => result.error = Some(format!("{e:#}")),
    }

    result
}

fn write_titlemap(system_dir: &Path, results: &[RomResult]) -> Result<()> {
    let path = system_dir.join("titlemap.txt");

    let mut titlemap = BTreeMap::new();
    if let Ok(existing) = std::fs::read_to_string(&path) {
        for line in existing.lines() {
            if let Some((id, title)) = line.split_once('=') {
                titlemap.insert(id.to_string(), title.to_string());
            }
        }
    }

    for result in results {
        if let Some(game_id) = &result.game_id {
            titlemap.insert(game_id.clone(), result.stem.clone());
        }
    }

    let mut content = String::new();
    for (id, title) in &titlemap {
        content.push_str(&format!("{id}={title}\n"));
    }

    std::fs::write(&path, content).with_context(|| format!("writing {}", path.display()))
}

fn write_gamelist(system_dir: &Path, results: &[RomResult]) -> Result<()> {
    let path = system_dir.join("gamelist.txt");

    let mut gamelist = BTreeSet::new();
    if let Ok(existing) = std::fs::read_to_string(&path) {
        gamelist.extend(existing.lines().map(str::to_string));
    }

    gamelist.extend(
        results
            .iter()
            .filter(|r| r.game_id.is_some())
            .map(|r| r.file_name.clone()),
    );

    let mut content = String::new();
    for file_name in &gamelist {
        content.push_str(file_name);
        content.push('\n');
    }

    std::fs::write(&path, content).with_context(|| format!("writing {}", path.display()))
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
