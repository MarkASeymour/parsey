// CLI: parse pattern, compile Engine, scan file line by line.

use std::env;
use std::fs::File;
use std::io::{self, BufRead, BufReader, Write};
use std::process::ExitCode;

use colored::Colorize;

use parsey::regex::parse::{parse_with, ParseOptions};
use parsey::regex::vm::Engine;

struct CliArgs<'a> {
    pattern: &'a str,
    file_path: &'a str,
    opts: ParseOptions,
}

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();
    let prog = args.first().map(String::as_str).unwrap_or("parsey");

    let cli = match parse_cli(&args) {
        Ok(cli) => cli,
        Err(msg) => {
            if !msg.is_empty() {
                eprintln!("parsey: {msg}");
            }
            eprintln!("Usage: {prog} [-i] <pattern> <file>");
            return ExitCode::from(2);
        }
    };

    match run(cli.pattern, cli.file_path, cli.opts) {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::from(1),
        Err(err) => {
            eprintln!("parsey: {err}");
            ExitCode::from(2)
        }
    }
}

fn parse_cli(args: &[String]) -> Result<CliArgs<'_>, String> {
    let mut opts = ParseOptions::default();
    let mut positional: Vec<&str> = Vec::new();
    let mut flags_done = false;

    for arg in args.iter().skip(1) {
        let bytes = arg.as_bytes();
        if !flags_done && arg == "--" {
            flags_done = true;
        } else if !flags_done && bytes.len() >= 2 && bytes[0] == b'-' && bytes[1] != b'-' && arg != "-" {
            for &c in &bytes[1..] {
                match c {
                    b'i' => opts.case_insensitive = true,
                    _ => return Err(format!("unknown flag '-{}'", c as char)),
                }
            }
        } else {
            positional.push(arg.as_str());
        }
    }

    match positional.as_slice() {
        [pattern, file_path] => Ok(CliArgs {
            pattern,
            file_path,
            opts,
        }),
        _ => Err(String::new()),
    }
}

fn run(pattern: &str, file_path: &str, opts: ParseOptions) -> io::Result<bool> {
    let ast = parse_with(pattern.as_bytes(), opts)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e.to_string()))?;
    let engine = Engine::from_ast(&ast)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e.to_string()))?;
    let nullable = engine.nullable();

    let mut reader = BufReader::new(File::open(file_path)?);
    let stdout = io::stdout();
    let mut out = stdout.lock();

    let mut buf: Vec<u8> = Vec::with_capacity(8 * 1024);
    let mut hits: Vec<(usize, usize)> = Vec::new();
    let mut line_num: usize = 0;
    let mut matched = false;

    loop {
        buf.clear();
        let n = reader.read_until(b'\n', &mut buf)?;
        if n == 0 {
            break;
        }
        line_num += 1;

        let mut end = buf.len();
        if end > 0 && buf[end - 1] == b'\n' {
            end -= 1;
            if end > 0 && buf[end - 1] == b'\r' {
                end -= 1;
            }
        }
        let line_bytes = &buf[..end];

        engine.find_all(line_bytes, &mut hits);
        if hits.is_empty() && !nullable {
            continue;
        }

        let line_str = match std::str::from_utf8(line_bytes) {
            Ok(s) => s,
            Err(_) => continue,
        };

        if let Some(formatted) = format_line(line_str, line_num, &hits) {
            writeln!(out, "{formatted}")?;
            matched = true;
        }
    }

    Ok(matched)
}

fn format_line(line: &str, line_num: usize, hits: &[(usize, usize)]) -> Option<String> {
    let mut result = format!("{}: ", line_num.to_string().bright_purple());
    let mut cursor = 0;
    for &(start, end) in hits {
        result.push_str(line.get(cursor..start)?);
        result.push_str(&line.get(start..end)?.red().to_string());
        cursor = end;
    }
    result.push_str(line.get(cursor..)?);
    Some(result)
}
