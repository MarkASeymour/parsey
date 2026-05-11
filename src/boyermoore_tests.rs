use crate::boyermoore::Searcher;

fn find(pattern: &str, text: &str) -> Vec<usize> {
    Searcher::new(pattern.as_bytes().to_vec())
        .unwrap()
        .find_all(text.as_bytes())
}

#[test]
fn empty_pattern_rejected() {
    assert!(Searcher::new(Vec::<u8>::new()).is_none());
}

#[test]
fn pattern_longer_than_text() {
    assert!(find("abc", "ab").is_empty());
}

#[test]
fn single_match() {
    assert_eq!(find("foo", "the foo bar"), vec![4]);
}

#[test]
fn overlapping_matches() {
    assert_eq!(find("ab", "ababab"), vec![0, 2, 4]);
}

#[test]
fn no_match() {
    assert!(find("zzz", "abcabcabc").is_empty());
}

#[test]
fn match_at_start_and_end() {
    assert_eq!(find("ab", "abxxab"), vec![0, 4]);
}

#[test]
fn single_char_pattern() {
    assert_eq!(find("a", "banana"), vec![1, 3, 5]);
}

#[test]
fn pattern_equals_text() {
    assert_eq!(find("hello", "hello"), vec![0]);
}

#[test]
fn utf8_byte_offsets() {
    assert_eq!(find("é", "café"), vec![3]);
}

#[test]
fn repeated_letters() {
    assert_eq!(find("aa", "aaaa"), vec![0, 1, 2]);
}
