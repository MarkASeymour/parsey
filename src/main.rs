// CLI: parse pattern, compile Engine, scan files / stdin / directory trees.

use std::env;
use std::fs::{self, File};
use std::io::{self, BufRead, BufReader, StdoutLock, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use colored::Colorize;

use parsey::regex::parse::{parse_with, ParseOptions};
use parsey::regex::vm::Engine;

struct CliArgs {
    pattern: String,
    inputs: Vec<String>,
    opts: ParseOptions,
    recursive: bool,
}

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();
    let prog = args
        .first()
        .map(String::as_str)
        .unwrap_or("parsey")
        .to_string();

    let cli = match parse_cli(args) {
        Ok(cli) => cli,
        Err(msg) => {
            if !msg.is_empty() {
                eprintln!("parsey: {msg}");
            }
            eprintln!("Usage: {prog} [-iRr] <pattern> [<file>... | -]");
            return ExitCode::from(2);
        }
    };

    match run(cli) {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::from(1),
        Err(err) => {
            eprintln!("parsey: {err}");
            ExitCode::from(2)
        }
    }
}

fn parse_cli(args: Vec<String>) -> Result<CliArgs, String> {
    let mut opts = ParseOptions::default();
    let mut recursive = false;
    let mut positional: Vec<String> = Vec::new();
    let mut flags_done = false;

    for arg in args.into_iter().skip(1) {
        let bytes = arg.as_bytes();
        if !flags_done && arg == "--" {
            flags_done = true;
        } else if !flags_done
            && bytes.len() >= 2
            && bytes[0] == b'-'
            && bytes[1] != b'-'
            && arg != "-"
        {
            for &c in &bytes[1..] {
                match c {
                    b'i' => opts.case_insensitive = true,
                    b'r' | b'R' => recursive = true,
                    _ => return Err(format!("unknown flag '-{}'", c as char)),
                }
            }
        } else {
            positional.push(arg);
        }
    }

    if positional.is_empty() {
        return Err(String::new());
    }
    let pattern = positional.remove(0);
    if positional.is_empty() {
        positional.push(if recursive { "." } else { "-" }.to_string());
    }
    Ok(CliArgs {
        pattern,
        inputs: positional,
        opts,
        recursive,
    })
}

fn run(cli: CliArgs) -> io::Result<bool> {
    let ast = parse_with(cli.pattern.as_bytes(), cli.opts)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e.to_string()))?;
    let engine = Engine::from_ast(&ast)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e.to_string()))?;

    let show_filename = cli.recursive || cli.inputs.len() > 1;
    let stdout = io::stdout();
    let mut out = stdout.lock();
    let mut scanner = Scanner {
        nullable: engine.nullable(),
        engine,
        buf: Vec::with_capacity(8 * 1024),
        hits: Vec::new(),
        show_filename,
        matched: false,
    };

    for input in &cli.inputs {
        if input == "-" {
            let stdin = io::stdin();
            let mut locked = stdin.lock();
            scanner.scan_reader("(standard input)", &mut locked, &mut out)?;
        } else {
            let path = Path::new(input);
            let meta = match fs::metadata(path) {
                Ok(m) => m,
                Err(err) => {
                    eprintln!("parsey: {}: {}", path.display(), err);
                    continue;
                }
            };
            if meta.is_dir() {
                if cli.recursive {
                    walk_dir(path, &mut scanner, &mut out)?;
                } else {
                    eprintln!("parsey: {}: Is a directory", path.display());
                }
            } else {
                scanner.scan_path(path, &mut out);
            }
        }
    }
    Ok(scanner.matched)
}

struct Scanner {
    engine: Engine,
    nullable: bool,
    buf: Vec<u8>,
    hits: Vec<(usize, usize)>,
    show_filename: bool,
    matched: bool,
}

impl Scanner {
    fn scan_path(&mut self, path: &Path, out: &mut StdoutLock<'_>) {
        let file = match File::open(path) {
            Ok(f) => f,
            Err(err) => {
                eprintln!("parsey: {}: {}", path.display(), err);
                return;
            }
        };
        let mut reader = BufReader::new(file);
        let label = path.display().to_string();
        if let Err(err) = self.scan_reader(&label, &mut reader, out) {
            eprintln!("parsey: {}: {}", path.display(), err);
        }
    }

    fn scan_reader(
        &mut self,
        label: &str,
        reader: &mut dyn BufRead,
        out: &mut StdoutLock<'_>,
    ) -> io::Result<()> {
        let label_opt = if self.show_filename { Some(label) } else { None };
        let mut line_num: usize = 0;
        loop {
            self.buf.clear();
            let n = reader.read_until(b'\n', &mut self.buf)?;
            if n == 0 {
                break;
            }
            line_num += 1;

            let mut end = self.buf.len();
            if end > 0 && self.buf[end - 1] == b'\n' {
                end -= 1;
                if end > 0 && self.buf[end - 1] == b'\r' {
                    end -= 1;
                }
            }
            let line_bytes = &self.buf[..end];

            self.engine.find_all(line_bytes, &mut self.hits);
            if self.hits.is_empty() && !self.nullable {
                continue;
            }

            let line_str = match std::str::from_utf8(line_bytes) {
                Ok(s) => s,
                Err(_) => continue,
            };

            if let Some(formatted) = format_line(label_opt, line_str, line_num, &self.hits) {
                writeln!(out, "{formatted}")?;
                self.matched = true;
            }
        }
        Ok(())
    }
}

fn format_line(
    label: Option<&str>,
    line: &str,
    line_num: usize,
    hits: &[(usize, usize)],
) -> Option<String> {
    let mut result = match label {
        Some(name) => format!(
            "{}:{}: ",
            name.cyan(),
            line_num.to_string().bright_purple()
        ),
        None => format!("{}: ", line_num.to_string().bright_purple()),
    };
    let mut cursor = 0;
    for &(start, end) in hits {
        result.push_str(line.get(cursor..start)?);
        result.push_str(&line.get(start..end)?.red().to_string());
        cursor = end;
    }
    result.push_str(line.get(cursor..)?);
    Some(result)
}

fn walk_dir(root: &Path, scanner: &mut Scanner, out: &mut StdoutLock<'_>) -> io::Result<()> {
    let mut stack: Vec<PathBuf> = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries = match fs::read_dir(&dir) {
            Ok(e) => e,
            Err(err) => {
                eprintln!("parsey: {}: {}", dir.display(), err);
                continue;
            }
        };
        let mut children: Vec<(PathBuf, fs::FileType)> = Vec::new();
        for entry in entries {
            let entry = match entry {
                Ok(e) => e,
                Err(err) => {
                    eprintln!("parsey: {}: {}", dir.display(), err);
                    continue;
                }
            };
            let ft = match entry.file_type() {
                Ok(t) => t,
                Err(err) => {
                    eprintln!("parsey: {}: {}", entry.path().display(), err);
                    continue;
                }
            };
            children.push((entry.path(), ft));
        }
        children.sort_by(|a, b| a.0.cmp(&b.0));

        let mut subdirs: Vec<PathBuf> = Vec::new();
        for (path, ft) in &children {
            if ft.is_symlink() {
                continue;
            } else if ft.is_file() {
                scanner.scan_path(path, out);
            } else if ft.is_dir() {
                subdirs.push(path.clone());
            }
        }
        for path in subdirs.into_iter().rev() {
            stack.push(path);
        }
    }
    Ok(())
}
