# parsey

A regex grep written by hand in Rust. No `regex` crate, no `clap`, no
`aho-corasick`. Standard library plus `colored` for ANSI output.

## Build

```
cargo build --release
```

## Use

```
parsey [-iRr] <pattern> [<file>... | -]
```

Prints each matching line prefixed with its line number, with all matches
highlighted in red. With more than one input, or with `-r`, every line is
also prefixed with the file path.

Inputs:

- One or more file paths.
- `-` reads from standard input.
- With no inputs, parsey reads standard input (or walks `.` if `-r` is set).

Flags:

- `-i` case insensitive matching (ASCII)
- `-r`, `-R` recurse into directories. Symlinks inside the tree are skipped.

Exit codes follow grep convention:

- `0`: at least one match found
- `1`: no matches found
- `2`: error (invalid pattern, missing file, usage error)

## Pattern syntax

Operators:

- `abc` literal bytes
- `.` any byte
- `*` `+` `?` standard quantifiers (greedy)
- `()` grouping
- `|` alternation

Character classes:

- `[abc]`, `[a-z]`, `[^abc]`, `[a-zA-Z0-9_]`
- A `-` at the start or end of a class is a literal `-`
- Escapes inside classes: `\]`, `\\`, `\d`, `\w`, `\s`, etc.

Anchors (whole pattern only):

- `^` the match must start at the beginning of the line
- `$` the match must end at the end of the line

Escapes:

- `\d \D` ASCII digit and not digit
- `\w \W` ASCII word `[A-Za-z0-9_]` and not word
- `\s \S` ASCII whitespace and not whitespace
- `\n \t \r` newline, tab, carriage return
- `\.` `\*` `\(` etc. escape any metacharacter

Counted repetition:

- `a{3}` exactly 3
- `a{3,}` 3 or more
- `a{2,5}` between 2 and 5

## Examples

```
parsey 'ERROR' app.log
parsey '^\[\w+\]' app.log
parsey '\d{3}-\d{4}' contacts.txt
parsey '\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}' access.log
parsey '[Hh]ello' greetings.txt
parsey '(GET|POST) /api/' access.log
parsey -i 'error' app.log
parsey -r 'TODO' src/
git diff | parsey '^\+\s*[a-z]'
```

## Limits

- No binary file detection. Files are scanned line by line as bytes; lines
  with invalid UTF-8 are skipped on output.
- No file globs or `.gitignore` awareness yet.
- Pattern complexity is capped at 64 positions. Longer patterns return an
  error. Long literal alternations such as a hundred way `(foo|bar|...)`
  list will hit this.
- `.` matches one byte, so multibyte UTF-8 characters do not match `.` as
  a single unit. Same as grep in the C locale.
- No `\b` word boundary yet.
- Anchors apply to the whole pattern, not per alternation branch. Use
  `^(foo|bar)$` rather than `^foo|bar$`.

## Performance

On a 32 MB log shaped file:

- about 1.4 GB/s on literal patterns (Boyer-Moore prefix scan plus NFA)
- about 1 GB/s on most regex patterns
- about 300 MB/s on match heavy patterns like `\d+`
- linear time on all inputs, including the classic `(a|a)*b` backtracking
  killer

Reproduce on your machine with:

```
cargo run --release --example bench
```

## License

See the `LICENSE` file.
