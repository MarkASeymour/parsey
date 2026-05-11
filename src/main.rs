// CLI: parse pattern, compile Engine, scan file line by line.

use std::env;
use std::fs::File;
use std::io::{self, BufRead, BufReader, Write};
use std::process::ExitCode;

use colored::Colorize;

use parsey::regex::parse::parse;
use parsey::regex::vm::Engine;

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();

    let (pattern, file_path) = match args.as_slice() {
        [_, pattern, file_path] => (pattern.as_str(), file_path.as_str()),
        _ => {
            let prog = args.first().map(String::as_str).unwrap_or("parsey");
            eprintln!("Usage: {prog} <pattern> <file>");
            return ExitCode::from(2);
        }
    };

    match run(pattern, file_path) {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::from(1),
        Err(err) => {
            eprintln!("parsey: {err}");
            ExitCode::from(2)
        }
    }
}

fn run(pattern: &str, file_path: &str) -> io::Result<bool> {
    let ast = parse(pattern.as_bytes())
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
