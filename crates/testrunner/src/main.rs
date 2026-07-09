mod cores;
mod harness;

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{Context, Result, bail};
use clap::Parser;
use owo_colors::OwoColorize;

use cores::Target;
use harness::{OpReport, Outcome};

#[derive(Parser)]
#[command(author, version, about)]
struct Args {
    target: String,

    #[arg(long)]
    tests: Option<PathBuf>,

    #[arg(long, value_delimiter = ',')]
    opcodes: Option<Vec<String>>,

    #[arg(long)]
    limit: Option<usize>,
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
    let targets = cores::select(&args.target);
    if targets.is_empty() {
        bail!(
            "unknown target '{}'. available: {}",
            args.target,
            cores::available().join(", ")
        );
    }
    if args.tests.is_some() && targets.len() > 1 {
        bail!("--tests requires selecting a single core, not the whole system");
    }

    let opcodes = args
        .opcodes
        .as_ref()
        .map(|list| self::parse_opcodes(list))
        .transpose()?;

    let mut all_clean = true;
    for target in targets {
        all_clean &= self::run_target(target, args, opcodes.as_deref())?;
    }
    Ok(all_clean)
}

fn run_target(target: &Target, args: &Args, opcodes: Option<&[u8]>) -> Result<bool> {
    let tests_dir = args
        .tests
        .clone()
        .unwrap_or_else(|| PathBuf::from(target.tests));
    if !tests_dir.is_dir() {
        bail!(
            "test directory not found: {} (is the submodule checked out?)",
            tests_dir.display()
        );
    }

    let opcodes = self::discover_opcodes(&tests_dir, target.suffix, opcodes)
        .with_context(|| format!("scanning {}", tests_dir.display()))?;
    if opcodes.is_empty() {
        bail!("no matching opcode test files in {}", tests_dir.display());
    }

    println!(
        "{} {} {}",
        "▶".cyan().bold(),
        target.name.bold(),
        format!("({}, {} opcodes)", tests_dir.display(), opcodes.len()).dimmed()
    );

    let mut core = target.build();
    let mut tally = Tally::default();

    for opcode in opcodes {
        let path = tests_dir.join(format!("{opcode:02x}{}.json", target.suffix));
        let report = harness::run_file(core.as_mut(), &path, args.limit)?;
        tally.add(&report);
        self::print_op(opcode, &report);
    }

    tally.print_summary();
    Ok(tally.failed == 0 && tally.panicked == 0)
}

#[derive(Default)]
struct Tally {
    clean: usize,
    failed: usize,
    panicked: usize,
    empty: usize,
    passed_tests: usize,
}

impl Tally {
    fn add(&mut self, report: &OpReport) {
        self.passed_tests += report.passed;
        match report.outcome {
            Outcome::Clean => self.clean += 1,
            Outcome::Empty => self.empty += 1,
            Outcome::Failed { .. } => self.failed += 1,
            Outcome::Panicked { .. } => self.panicked += 1,
        }
    }

    fn print_summary(&self) {
        let opcodes = self.clean + self.failed + self.panicked + self.empty;
        println!(
            "  {}  {}   {}   {}   {}",
            format!("{}/{} opcodes ok", self.clean, opcodes).bold(),
            format!("{} failed", self.failed).red(),
            format!("{} panicked", self.panicked).yellow(),
            format!("{} empty", self.empty).dimmed(),
            format!("{} tests passed", self.passed_tests).green(),
        );
        println!();
    }
}

fn print_op(opcode: u8, report: &OpReport) {
    let op = format!("{opcode:02X}");
    let op = op.bold();
    match &report.outcome {
        Outcome::Clean => {
            let note = format!("{} ok", report.passed);
            println!("  {op}  {}  {}", "PASS".green().bold(), note.dimmed());
        }
        Outcome::Empty => {
            println!("  {op}  {}  {}", "----".dimmed(), "no tests".dimmed());
        }
        Outcome::Panicked { test } => {
            let note = format!("panicked · after {} ok · {test}", report.passed);
            println!("  {op}  {}  {}", "FAIL".red().bold(), note.dimmed());
        }
        Outcome::Failed { test, fields } => {
            let note = format!("after {} ok · {test}", report.passed);
            println!("  {op}  {}  {}", "FAIL".red().bold(), note.dimmed());

            let bad: Vec<_> = fields.iter().filter(|f| !f.matched()).collect();
            let name_w = bad.iter().map(|f| f.name.len()).max().unwrap_or(0);

            for field in bad {
                let got = field.render(field.got);

                println!(
                    "        {:<name_w$}  {} {} {}",
                    field.name,
                    format!("{got:<6}").red(),
                    "vs".dimmed(),
                    field.render(field.want).green(),
                );
            }
        }
    }
}

fn discover_opcodes(dir: &Path, suffix: &str, requested: Option<&[u8]>) -> Result<Vec<u8>> {
    if let Some(requested) = requested {
        let mut opcodes: Vec<u8> = requested
            .iter()
            .copied()
            .filter(|op| dir.join(format!("{op:02x}{suffix}.json")).is_file())
            .collect();
        opcodes.sort_unstable();
        opcodes.dedup();
        return Ok(opcodes);
    }

    let want = format!("{suffix}.json");
    let mut opcodes = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
            continue;
        };

        if let Some(stem) = name.strip_suffix(&want)
            && let Ok(op) = u8::from_str_radix(stem, 16)
        {
            opcodes.push(op);
        }
    }

    opcodes.sort_unstable();

    Ok(opcodes)
}

fn parse_opcodes(list: &[String]) -> Result<Vec<u8>> {
    list.iter()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| u8::from_str_radix(s, 16).with_context(|| format!("invalid opcode: {s}")))
        .collect()
}
