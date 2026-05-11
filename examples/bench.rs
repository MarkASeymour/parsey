use std::time::Instant;

use parsey::regex::parse::parse;
use parsey::regex::vm::Engine;

const SIZE_MB: usize = 32;
const RUNS: usize = 5;

fn main() {
    let log_input = generate_log_lines(SIZE_MB * 1024 * 1024);
    let path_input = generate_pathological(SIZE_MB * 1024 * 1024);

    println!(
        "input: {} MB log-like, {} MB pathological, {} runs each\n",
        log_input.len() / 1024 / 1024,
        path_input.len() / 1024 / 1024,
        RUNS,
    );
    println!(
        "{:<28} {:<24} {:>10}  {:>10}  {:>10}",
        "scenario", "pattern", "matches", "ns/byte", "MB/s"
    );
    println!("{}", "-".repeat(90));

    bench("literal-prefix", "needle", &log_input);
    bench("literal-no-prefix", ".eedle", &log_input);
    bench("literal-1byte-prefix", "n.edle", &log_input);
    bench("digits", r"\d+", &log_input);
    bench("phone-shape", r"\d{3}-\d{4}", &log_input);
    bench("ipv4-shape", r"\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}", &log_input);
    bench("alternation", "error|warn|info", &log_input);
    bench("start-anchored", "^needle", &log_input);
    bench("end-anchored", "needle$", &log_input);
    bench("class-quantified", r"[a-z]{4,}", &log_input);
    bench("pathological", "(a|a)*b", &path_input);
}

fn bench(name: &str, pattern: &str, input: &[u8]) {
    let ast = match parse(pattern.as_bytes()) {
        Ok(a) => a,
        Err(e) => {
            println!("{:<28} {:<24} parse error: {}", name, pattern, e);
            return;
        }
    };
    let engine = match Engine::from_ast(&ast) {
        Ok(e) => e,
        Err(e) => {
            println!("{:<28} {:<24} compile error: {}", name, pattern, e);
            return;
        }
    };

    let mut out: Vec<(usize, usize)> = Vec::new();
    for _ in 0..3 {
        engine.find_all(input, &mut out);
    }
    let match_count = out.len();

    let start = Instant::now();
    for _ in 0..RUNS {
        engine.find_all(input, &mut out);
    }
    let elapsed = start.elapsed();

    let total_bytes = input.len() * RUNS;
    let ns_per_byte = elapsed.as_nanos() as f64 / total_bytes as f64;
    let mb_per_sec = (total_bytes as f64 / 1_000_000.0) / elapsed.as_secs_f64();

    println!(
        "{:<28} {:<24} {:>10}  {:>10.3}  {:>10.0}",
        name, pattern, match_count, ns_per_byte, mb_per_sec
    );
}

fn generate_log_lines(target_size: usize) -> Vec<u8> {
    let lines: &[&[u8]] = &[
        b"[INFO] 2026-05-10T08:42:01 user_id=4815 status=ok latency=12ms\n",
        b"[WARN] 2026-05-10T08:42:01 retry 3 of 5 backoff=250ms\n",
        b"[ERROR] 2026-05-10T08:42:02 connection refused at 10.0.0.1:5432\n",
        b"[INFO] 2026-05-10T08:42:02 served 200 OK from upstream-7\n",
        b"the quick brown fox jumps over the lazy dog and continues onward\n",
        b"GET /api/v1/users/4815/profile HTTP/1.1 200 1234ms 17KB cached\n",
        b"call (415) 555-7890 between 9am and 5pm pacific time daily\n",
        b"shipped 42 needle items to bin 17 awaiting downstream pickup\n",
    ];
    let mut buf = Vec::with_capacity(target_size + 256);
    let mut i = 0;
    while buf.len() < target_size {
        buf.extend_from_slice(lines[i % lines.len()]);
        i += 1;
    }
    buf.truncate(target_size);
    buf
}

fn generate_pathological(target_size: usize) -> Vec<u8> {
    let mut buf = vec![b'a'; target_size - 2];
    buf.push(b'b');
    buf.push(b'\n');
    buf
}
