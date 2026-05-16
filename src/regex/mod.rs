#![allow(dead_code)]

pub mod ast;
pub mod compile;
pub mod parse;
pub mod vm;

#[cfg(test)]
mod tests {
    use crate::regex::ast::{Ast, ClassSet};
    use crate::regex::compile::{compile, CompileError, Label};
    use crate::regex::parse::{parse, parse_with, ParseError, ParseErrorKind, ParseOptions};
    use crate::regex::vm::{build_byte_table, find_anchored, literal_prefix, Engine};

    fn lit(b: u8) -> Ast {
        Ast::Literal(b)
    }

    #[test]
    fn empty_pattern() {
        assert_eq!(parse(b""), Ok(Ast::Empty));
    }

    #[test]
    fn single_literal() {
        assert_eq!(parse(b"a"), Ok(lit(b'a')));
    }

    #[test]
    fn concat_two_literals() {
        assert_eq!(
            parse(b"ab"),
            Ok(Ast::Concat(vec![lit(b'a'), lit(b'b')]))
        );
    }

    #[test]
    fn alternation_two_literals() {
        assert_eq!(
            parse(b"a|b"),
            Ok(Ast::Alternate(vec![lit(b'a'), lit(b'b')]))
        );
    }

    #[test]
    fn alternation_precedence_below_concat() {
        assert_eq!(
            parse(b"ab|cd"),
            Ok(Ast::Alternate(vec![
                Ast::Concat(vec![lit(b'a'), lit(b'b')]),
                Ast::Concat(vec![lit(b'c'), lit(b'd')]),
            ]))
        );
    }

    #[test]
    fn star_binds_to_atom_only() {
        assert_eq!(
            parse(b"a*b"),
            Ok(Ast::Concat(vec![
                Ast::Star(Box::new(lit(b'a'))),
                lit(b'b'),
            ]))
        );
    }

    #[test]
    fn star_plus_question() {
        assert_eq!(parse(b"a*"), Ok(Ast::Star(Box::new(lit(b'a')))));
        assert_eq!(parse(b"a+"), Ok(Ast::Plus(Box::new(lit(b'a')))));
        assert_eq!(parse(b"a?"), Ok(Ast::Question(Box::new(lit(b'a')))));
    }

    #[test]
    fn group_is_transparent() {
        assert_eq!(parse(b"(a)"), Ok(lit(b'a')));
    }

    #[test]
    fn group_around_alternation_then_concat() {
        assert_eq!(
            parse(b"(a|b)c"),
            Ok(Ast::Concat(vec![
                Ast::Alternate(vec![lit(b'a'), lit(b'b')]),
                lit(b'c'),
            ]))
        );
    }

    #[test]
    fn quantifier_on_group() {
        assert_eq!(
            parse(b"(ab)+"),
            Ok(Ast::Plus(Box::new(Ast::Concat(vec![
                lit(b'a'),
                lit(b'b'),
            ]))))
        );
    }

    #[test]
    fn any_byte() {
        assert_eq!(parse(b"."), Ok(Ast::AnyByte));
    }

    #[test]
    fn dot_in_concat() {
        assert_eq!(
            parse(b"a.c"),
            Ok(Ast::Concat(vec![lit(b'a'), Ast::AnyByte, lit(b'c')]))
        );
    }

    #[test]
    fn escape_meta_chars() {
        assert_eq!(parse(b"\\."), Ok(lit(b'.')));
        assert_eq!(parse(b"\\*"), Ok(lit(b'*')));
        assert_eq!(parse(b"\\+"), Ok(lit(b'+')));
        assert_eq!(parse(b"\\?"), Ok(lit(b'?')));
        assert_eq!(parse(b"\\("), Ok(lit(b'(')));
        assert_eq!(parse(b"\\)"), Ok(lit(b')')));
        assert_eq!(parse(b"\\|"), Ok(lit(b'|')));
        assert_eq!(parse(b"\\\\"), Ok(lit(b'\\')));
    }

    #[test]
    fn empty_alternation_branch_right() {
        assert_eq!(
            parse(b"a|"),
            Ok(Ast::Alternate(vec![lit(b'a'), Ast::Empty]))
        );
    }

    #[test]
    fn empty_alternation_branch_left() {
        assert_eq!(
            parse(b"|a"),
            Ok(Ast::Alternate(vec![Ast::Empty, lit(b'a')]))
        );
    }

    #[test]
    fn empty_group() {
        assert_eq!(parse(b"()"), Ok(Ast::Empty));
    }

    #[test]
    fn unmatched_open_paren() {
        assert_eq!(
            parse(b"(a"),
            Err(ParseError {
                kind: ParseErrorKind::UnmatchedOpenParen,
                position: 0,
            })
        );
    }

    #[test]
    fn unmatched_close_paren() {
        assert_eq!(
            parse(b"a)"),
            Err(ParseError {
                kind: ParseErrorKind::UnmatchedCloseParen,
                position: 1,
            })
        );
    }

    #[test]
    fn nothing_to_repeat_at_start() {
        assert_eq!(
            parse(b"*"),
            Err(ParseError {
                kind: ParseErrorKind::NothingToRepeat,
                position: 0,
            })
        );
    }

    #[test]
    fn double_quantifier_errors() {
        assert_eq!(
            parse(b"a**"),
            Err(ParseError {
                kind: ParseErrorKind::NothingToRepeat,
                position: 2,
            })
        );
    }

    #[test]
    fn quantifier_after_open_paren() {
        assert_eq!(
            parse(b"(*"),
            Err(ParseError {
                kind: ParseErrorKind::NothingToRepeat,
                position: 1,
            })
        );
    }

    #[test]
    fn trailing_backslash() {
        assert_eq!(
            parse(b"\\"),
            Err(ParseError {
                kind: ParseErrorKind::TrailingBackslash,
                position: 0,
            })
        );
    }

    #[test]
    fn invalid_escape() {
        assert_eq!(
            parse(b"\\a"),
            Err(ParseError {
                kind: ParseErrorKind::InvalidEscape(b'a'),
                position: 0,
            })
        );
    }

    #[test]
    fn nested_groups_and_alternation() {
        assert_eq!(
            parse(b"((a|b)c)+"),
            Ok(Ast::Plus(Box::new(Ast::Concat(vec![
                Ast::Alternate(vec![lit(b'a'), lit(b'b')]),
                lit(b'c'),
            ]))))
        );
    }

    #[test]
    fn utf8_bytes_in_pattern_are_separate_literals() {
        assert_eq!(
            parse("é".as_bytes()),
            Ok(Ast::Concat(vec![lit(0xC3), lit(0xA9)]))
        );
    }

    fn compile_pat(pat: &[u8]) -> crate::regex::compile::Program {
        compile(&parse(pat).unwrap()).unwrap()
    }

    #[test]
    fn compile_empty_pattern() {
        let prog = compile(&Ast::Empty).unwrap();
        assert!(prog.nullable);
        assert_eq!(prog.first, 0);
        assert_eq!(prog.last, 0);
        assert!(prog.labels.is_empty());
        assert!(prog.follow.is_empty());
    }

    #[test]
    fn compile_single_literal() {
        let prog = compile_pat(b"a");
        assert!(!prog.nullable);
        assert_eq!(prog.first, 0b1);
        assert_eq!(prog.last, 0b1);
        assert_eq!(prog.labels, vec![Label::Literal(b'a')]);
        assert_eq!(prog.follow, vec![0]);
    }

    #[test]
    fn compile_any_byte() {
        let prog = compile_pat(b".");
        assert_eq!(prog.labels, vec![Label::AnyByte]);
        assert_eq!(prog.first, 0b1);
        assert_eq!(prog.last, 0b1);
        assert!(!prog.nullable);
    }

    #[test]
    fn compile_concat_ab() {
        let prog = compile_pat(b"ab");
        assert!(!prog.nullable);
        assert_eq!(prog.first, 0b01);
        assert_eq!(prog.last, 0b10);
        assert_eq!(prog.follow, vec![0b10, 0]);
    }

    #[test]
    fn compile_alternation_ab() {
        let prog = compile_pat(b"a|b");
        assert!(!prog.nullable);
        assert_eq!(prog.first, 0b11);
        assert_eq!(prog.last, 0b11);
        assert_eq!(prog.follow, vec![0, 0]);
    }

    #[test]
    fn compile_star_self_loop() {
        let prog = compile_pat(b"a*");
        assert!(prog.nullable);
        assert_eq!(prog.first, 0b1);
        assert_eq!(prog.last, 0b1);
        assert_eq!(prog.follow, vec![0b1]);
    }

    #[test]
    fn compile_plus_self_loop() {
        let prog = compile_pat(b"a+");
        assert!(!prog.nullable);
        assert_eq!(prog.first, 0b1);
        assert_eq!(prog.last, 0b1);
        assert_eq!(prog.follow, vec![0b1]);
    }

    #[test]
    fn compile_question_nullable_no_loop() {
        let prog = compile_pat(b"a?");
        assert!(prog.nullable);
        assert_eq!(prog.first, 0b1);
        assert_eq!(prog.last, 0b1);
        assert_eq!(prog.follow, vec![0]);
    }

    #[test]
    fn compile_a_star_b() {
        let prog = compile_pat(b"a*b");
        assert!(!prog.nullable);
        assert_eq!(prog.first, 0b11);
        assert_eq!(prog.last, 0b10);
        assert_eq!(prog.follow, vec![0b11, 0]);
    }

    #[test]
    fn compile_a_star_b_star() {
        let prog = compile_pat(b"a*b*");
        assert!(prog.nullable);
        assert_eq!(prog.first, 0b11);
        assert_eq!(prog.last, 0b11);
        assert_eq!(prog.follow, vec![0b11, 0b10]);
    }

    #[test]
    fn compile_ab_group_star() {
        let prog = compile_pat(b"(ab)*");
        assert!(prog.nullable);
        assert_eq!(prog.first, 0b01);
        assert_eq!(prog.last, 0b10);
        assert_eq!(prog.follow, vec![0b10, 0b01]);
    }

    #[test]
    fn compile_alternation_in_concat() {
        let prog = compile_pat(b"(a|b)c");
        assert!(!prog.nullable);
        assert_eq!(prog.first, 0b011);
        assert_eq!(prog.last, 0b100);
        assert_eq!(prog.follow, vec![0b100, 0b100, 0]);
    }

    #[test]
    fn compile_three_nullable_concat() {
        let prog = compile_pat(b"a*b*c");
        assert!(!prog.nullable);
        assert_eq!(prog.first, 0b111);
        assert_eq!(prog.last, 0b100);
        assert_eq!(prog.follow, vec![0b111, 0b110, 0]);
    }

    #[test]
    fn compile_dot_star_x() {
        let prog = compile_pat(b".*x");
        assert_eq!(prog.labels, vec![Label::AnyByte, Label::Literal(b'x')]);
        assert!(!prog.nullable);
        assert_eq!(prog.first, 0b11);
        assert_eq!(prog.last, 0b10);
        assert_eq!(prog.follow, vec![0b11, 0]);
    }

    #[test]
    fn compile_too_many_positions_errors() {
        let bytes = vec![b'a'; 65];
        let ast = parse(&bytes).unwrap();
        assert_eq!(compile(&ast), Err(CompileError::TooManyPositions));
    }

    #[test]
    fn compile_exactly_max_positions_succeeds() {
        let bytes = vec![b'a'; 64];
        let ast = parse(&bytes).unwrap();
        let prog = compile(&ast).unwrap();
        assert_eq!(prog.labels.len(), 64);
        assert_eq!(prog.follow[63], 0);
        assert_eq!(prog.follow[0], 1u64 << 1);
    }

    fn anchored(pat: &[u8], input: &[u8]) -> Option<(usize, usize)> {
        let prog = compile(&parse(pat).unwrap()).unwrap();
        let table = build_byte_table(&prog);
        find_anchored(&prog, &table, input)
    }

    #[test]
    fn vm_byte_table_literal() {
        let prog = compile(&parse(b"a").unwrap()).unwrap();
        let table = build_byte_table(&prog);
        assert_eq!(table.b[b'a' as usize], 0b1);
        assert_eq!(table.b[b'b' as usize], 0);
    }

    #[test]
    fn vm_byte_table_any_byte() {
        let prog = compile(&parse(b".").unwrap()).unwrap();
        let table = build_byte_table(&prog);
        for c in 0..=255u8 {
            assert_eq!(table.b[c as usize], 0b1, "byte {} should match AnyByte", c);
        }
    }

    #[test]
    fn vm_byte_table_mixed() {
        let prog = compile(&parse(b"a.b").unwrap()).unwrap();
        let table = build_byte_table(&prog);
        assert_eq!(table.b[b'a' as usize], 0b011);
        assert_eq!(table.b[b'b' as usize], 0b110);
        assert_eq!(table.b[b'x' as usize], 0b010);
    }

    #[test]
    fn vm_anchored_literal_matches() {
        assert_eq!(anchored(b"abc", b"abc"), Some((0, 3)));
    }

    #[test]
    fn vm_anchored_literal_matches_prefix() {
        assert_eq!(anchored(b"abc", b"abcdef"), Some((0, 3)));
    }

    #[test]
    fn vm_anchored_literal_no_match() {
        assert_eq!(anchored(b"abc", b"xyz"), None);
    }

    #[test]
    fn vm_anchored_literal_partial_input() {
        assert_eq!(anchored(b"abc", b"ab"), None);
    }

    #[test]
    fn vm_anchored_match_must_start_at_zero() {
        assert_eq!(anchored(b"abc", b"xabc"), None);
    }

    #[test]
    fn vm_anchored_empty_pattern_always_matches() {
        assert_eq!(anchored(b"", b""), Some((0, 0)));
        assert_eq!(anchored(b"", b"abc"), Some((0, 0)));
    }

    #[test]
    fn vm_anchored_star_is_leftmost_longest() {
        assert_eq!(anchored(b"a*", b"aaa"), Some((0, 3)));
        assert_eq!(anchored(b"a*", b"aaabc"), Some((0, 3)));
        assert_eq!(anchored(b"a*", b"bbb"), Some((0, 0)));
        assert_eq!(anchored(b"a*", b""), Some((0, 0)));
    }

    #[test]
    fn vm_anchored_plus() {
        assert_eq!(anchored(b"a+", b"aaa"), Some((0, 3)));
        assert_eq!(anchored(b"a+", b"abc"), Some((0, 1)));
        assert_eq!(anchored(b"a+", b"bbb"), None);
        assert_eq!(anchored(b"a+", b""), None);
    }

    #[test]
    fn vm_anchored_question() {
        assert_eq!(anchored(b"a?", b"abc"), Some((0, 1)));
        assert_eq!(anchored(b"a?", b"bbc"), Some((0, 0)));
        assert_eq!(anchored(b"a?", b""), Some((0, 0)));
    }

    #[test]
    fn vm_anchored_a_star_b() {
        assert_eq!(anchored(b"a*b", b"aaab"), Some((0, 4)));
        assert_eq!(anchored(b"a*b", b"b"), Some((0, 1)));
        assert_eq!(anchored(b"a*b", b"aaac"), None);
        assert_eq!(anchored(b"a*b", b""), None);
    }

    #[test]
    fn vm_anchored_dot_matches_any_byte() {
        assert_eq!(anchored(b".", b"a"), Some((0, 1)));
        assert_eq!(anchored(b".", b"\x00"), Some((0, 1)));
        assert_eq!(anchored(b".", b"\xff"), Some((0, 1)));
        assert_eq!(anchored(b".", b""), None);
    }

    #[test]
    fn vm_anchored_dot_in_concat() {
        assert_eq!(anchored(b"a.c", b"abc"), Some((0, 3)));
        assert_eq!(anchored(b"a.c", b"axc"), Some((0, 3)));
        assert_eq!(anchored(b"a.c", b"ac"), None);
    }

    #[test]
    fn vm_anchored_dot_star_consumes_rest() {
        assert_eq!(anchored(b".*", b"anything"), Some((0, 8)));
        assert_eq!(anchored(b".*", b""), Some((0, 0)));
    }

    #[test]
    fn vm_anchored_dot_star_x_leftmost_longest() {
        assert_eq!(anchored(b".*x", b"foox"), Some((0, 4)));
        assert_eq!(anchored(b".*x", b"axbxcx"), Some((0, 6)));
    }

    #[test]
    fn vm_anchored_alternation_either_branch() {
        assert_eq!(anchored(b"a|b", b"abc"), Some((0, 1)));
        assert_eq!(anchored(b"a|b", b"bcd"), Some((0, 1)));
        assert_eq!(anchored(b"a|b", b"xyz"), None);
    }

    #[test]
    fn vm_anchored_alternation_leftmost_longest_picks_longer_branch() {
        assert_eq!(anchored(b"a|ab", b"ab"), Some((0, 2)));
        assert_eq!(anchored(b"ab|a", b"ab"), Some((0, 2)));
    }

    #[test]
    fn vm_anchored_group_star() {
        assert_eq!(anchored(b"(ab)*", b"abab"), Some((0, 4)));
        assert_eq!(anchored(b"(ab)*", b"ababx"), Some((0, 4)));
        assert_eq!(anchored(b"(ab)*", b"x"), Some((0, 0)));
    }

    #[test]
    fn vm_anchored_group_alternation() {
        assert_eq!(anchored(b"(a|b)c", b"ac"), Some((0, 2)));
        assert_eq!(anchored(b"(a|b)c", b"bc"), Some((0, 2)));
        assert_eq!(anchored(b"(a|b)c", b"cc"), None);
    }

    #[test]
    fn vm_anchored_utf8_byte_sequence() {
        assert_eq!(anchored("é".as_bytes(), "éclair".as_bytes()), Some((0, 2)));
        assert_eq!(anchored("café".as_bytes(), "café au lait".as_bytes()), Some((0, 5)));
    }

    #[test]
    fn vm_anchored_pathological_no_blowup() {
        let pat = b"(a|a)*b";
        let input = b"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaab";
        assert_eq!(anchored(pat, input), Some((0, input.len())));
    }

    fn engine(pat: &[u8]) -> Engine {
        Engine::from_ast(&parse(pat).unwrap()).unwrap()
    }

    fn find(pat: &[u8], input: &[u8]) -> Option<(usize, usize)> {
        engine(pat).find(input)
    }

    fn find_all(pat: &[u8], input: &[u8]) -> Vec<(usize, usize)> {
        let mut out = Vec::new();
        engine(pat).find_all(input, &mut out);
        out
    }

    #[test]
    fn engine_find_literal_anywhere() {
        assert_eq!(find(b"foo", b"foo"), Some((0, 3)));
        assert_eq!(find(b"foo", b"barfoo"), Some((3, 6)));
        assert_eq!(find(b"foo", b"barfoobaz"), Some((3, 6)));
    }

    #[test]
    fn engine_find_no_match() {
        assert_eq!(find(b"foo", b"bar"), None);
        assert_eq!(find(b"foo", b""), None);
    }

    #[test]
    fn engine_find_at_end_of_input() {
        assert_eq!(find(b"foo", b"xyzfoo"), Some((3, 6)));
    }

    #[test]
    fn engine_find_leftmost() {
        assert_eq!(find(b"a", b"xxaxxa"), Some((2, 3)));
    }

    #[test]
    fn engine_find_leftmost_longest_alternation() {
        assert_eq!(find(b"a|ab", b"xxab"), Some((2, 4)));
        assert_eq!(find(b"ab|a", b"xxab"), Some((2, 4)));
    }

    #[test]
    fn engine_find_dot_star_x() {
        assert_eq!(find(b".*x", b"axbxcx"), Some((0, 6)));
        assert_eq!(find(b".*x", b"yyy axbxcx"), Some((0, 10)));
    }

    #[test]
    fn engine_find_plus_extends() {
        assert_eq!(find(b"a+", b"xxaaab"), Some((2, 5)));
        assert_eq!(find(b"ab+", b"aabb"), Some((1, 4)));
    }

    #[test]
    fn engine_find_group_plus() {
        assert_eq!(find(b"(ab)+", b"xyababab"), Some((2, 8)));
    }

    #[test]
    fn engine_find_nullable_pattern_returns_zero_when_no_real_match() {
        assert_eq!(find(b"a*", b"bbb"), Some((0, 0)));
    }

    #[test]
    fn engine_find_nullable_pattern_prefers_nonempty_match() {
        assert_eq!(find(b"a*", b"bbab"), Some((2, 3)));
        assert_eq!(find(b"a*", b"bbaab"), Some((2, 4)));
    }

    #[test]
    fn engine_find_dot_star_consumes_to_end() {
        assert_eq!(find(b".*", b"abcdef"), Some((0, 6)));
    }

    #[test]
    fn engine_find_utf8_substring() {
        assert_eq!(find("é".as_bytes(), "café".as_bytes()), Some((3, 5)));
    }

    #[test]
    fn engine_find_all_non_overlapping_literals() {
        assert_eq!(find_all(b"ab", b"ababab"), vec![(0, 2), (2, 4), (4, 6)]);
    }

    #[test]
    fn engine_find_all_skips_no_match_regions() {
        assert_eq!(
            find_all(b"foo", b"foo bar foo baz foo"),
            vec![(0, 3), (8, 11), (16, 19)]
        );
    }

    #[test]
    fn engine_find_all_no_matches_returns_empty() {
        assert_eq!(find_all(b"foo", b"bar baz"), Vec::<(usize, usize)>::new());
    }

    #[test]
    fn engine_find_all_nullable_returns_empty_when_no_nonempty() {
        assert_eq!(find_all(b"a*", b"bbb"), Vec::<(usize, usize)>::new());
    }

    #[test]
    fn engine_find_all_consumes_greedily_then_continues() {
        assert_eq!(find_all(b"a+", b"aaa bb aaaa"), vec![(0, 3), (7, 11)]);
    }

    #[test]
    fn engine_find_all_alternation_leftmost_longest() {
        assert_eq!(find_all(b"a|ab", b"abab"), vec![(0, 2), (2, 4)]);
    }

    #[test]
    fn engine_find_all_dot_plus() {
        assert_eq!(find_all(b".+", b"foo"), vec![(0, 3)]);
    }

    #[test]
    fn engine_pathological_alternation_linear_time() {
        let pat = b"(a|a)*b";
        let mut input = vec![b'a'; 200];
        input.push(b'b');
        let mut out = Vec::new();
        engine(pat).find_all(&input, &mut out);
        assert_eq!(out, vec![(0, 201)]);
    }

    #[test]
    fn engine_nullable_reports_via_method() {
        assert!(engine(b"a*").nullable());
        assert!(!engine(b"a+").nullable());
        assert!(engine(b"").nullable());
        assert!(engine(b"a?").nullable());
    }

    fn prefix_of(pat: &[u8]) -> Vec<u8> {
        literal_prefix(&parse(pat).unwrap())
    }

    #[test]
    fn literal_prefix_pure_literal() {
        assert_eq!(prefix_of(b"error"), b"error");
    }

    #[test]
    fn literal_prefix_stops_at_star() {
        assert_eq!(prefix_of(b"err*or"), b"er");
    }

    #[test]
    fn literal_prefix_stops_at_question() {
        assert_eq!(prefix_of(b"err?or"), b"er");
    }

    #[test]
    fn literal_prefix_stops_at_dot() {
        assert_eq!(prefix_of(b"er.or"), b"er");
    }

    #[test]
    fn literal_prefix_stops_at_alternation() {
        assert_eq!(prefix_of(b"er(a|b)"), b"er");
    }

    #[test]
    fn literal_prefix_includes_one_iteration_of_plus() {
        assert_eq!(prefix_of(b"error+"), b"error");
    }

    #[test]
    fn literal_prefix_includes_group_inside_plus() {
        assert_eq!(prefix_of(b"x(ab)+y"), b"xab");
    }

    #[test]
    fn literal_prefix_empty_for_leading_star() {
        assert_eq!(prefix_of(b"a*b"), Vec::<u8>::new());
    }

    #[test]
    fn literal_prefix_empty_for_leading_alternation() {
        assert_eq!(prefix_of(b"(a|b)c"), Vec::<u8>::new());
    }

    #[test]
    fn literal_prefix_empty_for_empty_pattern() {
        assert_eq!(prefix_of(b""), Vec::<u8>::new());
    }

    #[test]
    fn engine_with_prefix_finds_literal_match() {
        assert_eq!(find(b"error", b"the error: oops"), Some((4, 9)));
    }

    #[test]
    fn engine_with_prefix_finds_regex_match() {
        assert_eq!(find(b"error+", b"xyz errorrr more"), Some((4, 11)));
    }

    #[test]
    fn engine_with_prefix_skips_false_candidates() {
        assert_eq!(find(b"error.x", b"error error.x"), Some((6, 13)));
    }

    #[test]
    fn engine_with_prefix_find_all_non_overlapping() {
        assert_eq!(
            find_all(b"foo", b"foo bar foo baz foo"),
            vec![(0, 3), (8, 11), (16, 19)]
        );
    }

    #[test]
    fn engine_with_prefix_find_all_correct_with_regex_suffix() {
        assert_eq!(
            find_all(b"x.y", b"axay xby xcy zzz xdy"),
            vec![(1, 4), (5, 8), (9, 12), (17, 20)]
        );
    }

    #[test]
    fn engine_prefix_optimization_matches_no_prefix_path() {
        let cases: &[(&[u8], &[u8])] = &[
            (b"abc", b"xyz abc def"),
            (b"a+", b"bb aaa cc"),
            (b"(a|b)c", b"xy ac zz bc"),
            (b".x", b"hello ax world"),
            (b"a*b", b"bbab aab"),
        ];
        for (pat, input) in cases {
            let with = find_all(pat, input);
            assert_eq!(
                engine(pat).find_all_to_vec(input),
                with,
                "pattern {:?} input {:?}",
                std::str::from_utf8(pat).unwrap(),
                std::str::from_utf8(input).unwrap()
            );
        }
    }

    trait EngineExt {
        fn find_all_to_vec(&self, input: &[u8]) -> Vec<(usize, usize)>;
    }

    impl EngineExt for Engine {
        fn find_all_to_vec(&self, input: &[u8]) -> Vec<(usize, usize)> {
            let mut out = Vec::new();
            self.find_all(input, &mut out);
            out
        }
    }

    fn class(bytes: &[u8]) -> Ast {
        let mut s = ClassSet::new();
        for &b in bytes {
            s.add(b);
        }
        Ast::Class(s)
    }

    fn class_range(lo: u8, hi: u8) -> Ast {
        let mut s = ClassSet::new();
        s.add_range(lo, hi);
        Ast::Class(s)
    }

    #[test]
    fn parse_class_single_chars() {
        assert_eq!(parse(b"[abc]"), Ok(class(b"abc")));
    }

    #[test]
    fn parse_class_range() {
        assert_eq!(parse(b"[a-z]"), Ok(class_range(b'a', b'z')));
    }

    #[test]
    fn parse_class_multiple_ranges() {
        let mut s = ClassSet::new();
        s.add_range(b'a', b'z');
        s.add_range(b'A', b'Z');
        s.add_range(b'0', b'9');
        assert_eq!(parse(b"[a-zA-Z0-9]"), Ok(Ast::Class(s)));
    }

    #[test]
    fn parse_class_negated() {
        let mut s = ClassSet::new();
        s.add(b'a');
        s.add(b'b');
        s.negate();
        assert_eq!(parse(b"[^ab]"), Ok(Ast::Class(s)));
    }

    #[test]
    fn parse_class_dash_at_start_is_literal() {
        assert_eq!(parse(b"[-a]"), Ok(class(b"-a")));
    }

    #[test]
    fn parse_class_dash_at_end_is_literal() {
        assert_eq!(parse(b"[a-]"), Ok(class(b"a-")));
    }

    #[test]
    fn parse_class_escapes() {
        assert_eq!(parse(b"[\\]\\\\]"), Ok(class(b"]\\")));
    }

    #[test]
    fn parse_class_empty_errors() {
        assert_eq!(
            parse(b"[]"),
            Err(ParseError {
                kind: ParseErrorKind::EmptyCharacterClass,
                position: 0,
            })
        );
    }

    #[test]
    fn parse_class_unmatched_open_errors() {
        assert_eq!(
            parse(b"[abc"),
            Err(ParseError {
                kind: ParseErrorKind::UnmatchedOpenBracket,
                position: 0,
            })
        );
    }

    #[test]
    fn parse_class_invalid_range_errors() {
        assert!(matches!(
            parse(b"[z-a]"),
            Err(ParseError {
                kind: ParseErrorKind::InvalidClassRange,
                ..
            })
        ));
    }

    #[test]
    fn parse_unmatched_close_bracket_errors() {
        assert!(matches!(
            parse(b"abc]"),
            Err(ParseError {
                kind: ParseErrorKind::UnmatchedCloseBracket,
                ..
            })
        ));
    }

    #[test]
    fn engine_class_matches_any_member() {
        assert_eq!(find(b"[abc]", b"xyz a 123"), Some((4, 5)));
        assert_eq!(find(b"[abc]", b"xyz"), None);
    }

    #[test]
    fn engine_class_range_digits() {
        assert_eq!(find(b"[0-9]+", b"abc 1234 def"), Some((4, 8)));
    }

    #[test]
    fn engine_class_negated() {
        assert_eq!(find(b"[^a-z]", b"abcABC"), Some((3, 4)));
    }

    #[test]
    fn engine_class_in_concat() {
        assert_eq!(find(b"x[0-9]y", b"abc x5y def"), Some((4, 7)));
    }

    #[test]
    fn engine_class_with_alternation() {
        assert_eq!(find(b"([0-9]+|[a-z]+)", b"123 abc"), Some((0, 3)));
        assert_eq!(find(b"([0-9]+|[a-z]+)", b"!!abc"), Some((2, 5)));
    }

    #[test]
    fn parse_start_anchor() {
        assert_eq!(
            parse(b"^foo"),
            Ok(Ast::Concat(vec![
                Ast::StartAnchor,
                Ast::Literal(b'f'),
                Ast::Literal(b'o'),
                Ast::Literal(b'o'),
            ]))
        );
    }

    #[test]
    fn parse_end_anchor() {
        assert_eq!(
            parse(b"foo$"),
            Ok(Ast::Concat(vec![
                Ast::Literal(b'f'),
                Ast::Literal(b'o'),
                Ast::Literal(b'o'),
                Ast::EndAnchor,
            ]))
        );
    }

    #[test]
    fn parse_both_anchors() {
        assert_eq!(
            parse(b"^foo$"),
            Ok(Ast::Concat(vec![
                Ast::StartAnchor,
                Ast::Literal(b'f'),
                Ast::Literal(b'o'),
                Ast::Literal(b'o'),
                Ast::EndAnchor,
            ]))
        );
    }

    #[test]
    fn parse_only_start_anchor() {
        assert_eq!(parse(b"^"), Ok(Ast::StartAnchor));
    }

    #[test]
    fn parse_only_end_anchor() {
        assert_eq!(parse(b"$"), Ok(Ast::EndAnchor));
    }

    #[test]
    fn parse_anchor_pair() {
        assert_eq!(
            parse(b"^$"),
            Ok(Ast::Concat(vec![Ast::StartAnchor, Ast::EndAnchor]))
        );
    }

    #[test]
    fn parse_anchor_in_middle_errors() {
        assert!(matches!(
            parse(b"f^oo"),
            Err(ParseError {
                kind: ParseErrorKind::AnchorNotAtBoundary,
                ..
            })
        ));
        assert!(matches!(
            parse(b"foo$bar"),
            Err(ParseError {
                kind: ParseErrorKind::AnchorNotAtBoundary,
                ..
            })
        ));
    }

    #[test]
    fn parse_escaped_anchors_are_literal() {
        assert_eq!(parse(b"\\^"), Ok(Ast::Literal(b'^')));
        assert_eq!(parse(b"\\$"), Ok(Ast::Literal(b'$')));
    }

    #[test]
    fn parse_escaped_dollar_at_end_is_literal() {
        assert_eq!(
            parse(b"a\\$"),
            Ok(Ast::Concat(vec![Ast::Literal(b'a'), Ast::Literal(b'$')]))
        );
    }

    #[test]
    fn engine_start_anchor_matches_only_at_zero() {
        assert_eq!(find(b"^foo", b"foo bar"), Some((0, 3)));
        assert_eq!(find(b"^foo", b"bar foo"), None);
    }

    #[test]
    fn engine_end_anchor_matches_only_at_end() {
        assert_eq!(find(b"foo$", b"bar foo"), Some((4, 7)));
        assert_eq!(find(b"foo$", b"foo bar"), None);
    }

    #[test]
    fn engine_both_anchors_requires_full_match() {
        assert_eq!(find(b"^foo$", b"foo"), Some((0, 3)));
        assert_eq!(find(b"^foo$", b"foobar"), None);
        assert_eq!(find(b"^foo$", b"xfoo"), None);
    }

    #[test]
    fn engine_start_anchor_with_quantifier() {
        assert_eq!(find(b"^a+", b"aaab"), Some((0, 3)));
        assert_eq!(find(b"^a+", b"baaa"), None);
    }

    #[test]
    fn engine_end_anchor_with_quantifier() {
        assert_eq!(find(b"a+$", b"baaa"), Some((1, 4)));
        assert_eq!(find(b"a+$", b"aaab"), None);
    }

    #[test]
    fn engine_anchors_with_class() {
        assert_eq!(find(b"^[a-z]+$", b"hello"), Some((0, 5)));
        assert_eq!(find(b"^[a-z]+$", b"Hello"), None);
        assert_eq!(find(b"^[a-z]+$", b"hello!"), None);
    }

    #[test]
    fn engine_anchor_only_patterns() {
        assert_eq!(find(b"^", b"abc"), Some((0, 0)));
        assert_eq!(find(b"$", b"abc"), Some((3, 3)));
        assert_eq!(find(b"^$", b""), Some((0, 0)));
        assert_eq!(find(b"^$", b"x"), None);
    }

    #[test]
    fn engine_find_all_anchored_returns_single_match() {
        assert_eq!(find_all(b"^foo", b"foo foo foo"), vec![(0, 3)]);
        assert_eq!(find_all(b"foo$", b"foo foo foo"), vec![(8, 11)]);
    }

    #[test]
    fn parse_digit_shortcut() {
        assert_eq!(parse(b"\\d"), Ok(Ast::Class(ClassSet::digit())));
    }

    #[test]
    fn parse_non_digit_shortcut() {
        let mut s = ClassSet::digit();
        s.negate();
        assert_eq!(parse(b"\\D"), Ok(Ast::Class(s)));
    }

    #[test]
    fn parse_word_shortcut() {
        assert_eq!(parse(b"\\w"), Ok(Ast::Class(ClassSet::word())));
    }

    #[test]
    fn parse_whitespace_shortcut() {
        assert_eq!(parse(b"\\s"), Ok(Ast::Class(ClassSet::whitespace())));
    }

    #[test]
    fn parse_shortcut_inside_class() {
        let mut s = ClassSet::digit();
        s.add(b'_');
        assert_eq!(parse(b"[\\d_]"), Ok(Ast::Class(s)));
    }

    #[test]
    fn parse_negated_shortcut_inside_class() {
        let mut digit = ClassSet::digit();
        digit.negate();
        let mut expected = digit;
        expected.add(b'!');
        assert_eq!(parse(b"[\\D!]"), Ok(Ast::Class(expected)));
    }

    #[test]
    fn parse_combined_shortcuts_in_class() {
        let mut s = ClassSet::digit();
        s.union(&ClassSet::word());
        s.union(&ClassSet::whitespace());
        assert_eq!(parse(b"[\\d\\w\\s]"), Ok(Ast::Class(s)));
    }

    #[test]
    fn parse_escape_n_t_r() {
        assert_eq!(parse(b"\\n"), Ok(Ast::Literal(b'\n')));
        assert_eq!(parse(b"\\t"), Ok(Ast::Literal(b'\t')));
        assert_eq!(parse(b"\\r"), Ok(Ast::Literal(b'\r')));
    }

    #[test]
    fn engine_digit_shortcut_matches() {
        assert_eq!(find(b"\\d+", b"abc 123 def"), Some((4, 7)));
        assert_eq!(find(b"\\d+", b"no digits"), None);
    }

    #[test]
    fn engine_non_digit_shortcut() {
        assert_eq!(find(b"\\D+", b"123abc456"), Some((3, 6)));
    }

    #[test]
    fn engine_word_shortcut() {
        assert_eq!(find(b"\\w+", b"!! hello !!"), Some((3, 8)));
    }

    #[test]
    fn engine_whitespace_shortcut() {
        assert_eq!(find(b"\\s+", b"foo   bar"), Some((3, 6)));
    }

    #[test]
    fn parse_counted_exact() {
        assert_eq!(
            parse(b"a{3}"),
            Ok(Ast::Concat(vec![
                Ast::Literal(b'a'),
                Ast::Literal(b'a'),
                Ast::Literal(b'a'),
            ]))
        );
    }

    #[test]
    fn parse_counted_min() {
        assert_eq!(
            parse(b"a{2,}"),
            Ok(Ast::Concat(vec![
                Ast::Literal(b'a'),
                Ast::Literal(b'a'),
                Ast::Star(Box::new(Ast::Literal(b'a'))),
            ]))
        );
    }

    #[test]
    fn parse_counted_range() {
        assert_eq!(
            parse(b"a{2,4}"),
            Ok(Ast::Concat(vec![
                Ast::Literal(b'a'),
                Ast::Literal(b'a'),
                Ast::Question(Box::new(Ast::Literal(b'a'))),
                Ast::Question(Box::new(Ast::Literal(b'a'))),
            ]))
        );
    }

    #[test]
    fn parse_counted_zero_min() {
        assert_eq!(
            parse(b"a{0,2}"),
            Ok(Ast::Concat(vec![
                Ast::Question(Box::new(Ast::Literal(b'a'))),
                Ast::Question(Box::new(Ast::Literal(b'a'))),
            ]))
        );
    }

    #[test]
    fn parse_counted_zero_zero_is_empty() {
        assert_eq!(parse(b"a{0}"), Ok(Ast::Empty));
    }

    #[test]
    fn parse_counted_one_is_identity() {
        assert_eq!(parse(b"a{1}"), Ok(Ast::Literal(b'a')));
    }

    #[test]
    fn parse_brace_not_quantifier_is_literal() {
        assert_eq!(
            parse(b"a{x}"),
            Ok(Ast::Concat(vec![
                Ast::Literal(b'a'),
                Ast::Literal(b'{'),
                Ast::Literal(b'x'),
                Ast::Literal(b'}'),
            ]))
        );
    }

    #[test]
    fn parse_lone_brace_is_literal() {
        assert_eq!(parse(b"{"), Ok(Ast::Literal(b'{')));
        assert_eq!(parse(b"}"), Ok(Ast::Literal(b'}')));
    }

    #[test]
    fn parse_escaped_braces_are_literal() {
        assert_eq!(parse(b"\\{"), Ok(Ast::Literal(b'{')));
        assert_eq!(parse(b"\\}"), Ok(Ast::Literal(b'}')));
    }

    #[test]
    fn parse_counted_invalid_range_errors() {
        assert!(matches!(
            parse(b"a{5,2}"),
            Err(ParseError {
                kind: ParseErrorKind::InvalidCountedRepetition,
                ..
            })
        ));
    }

    #[test]
    fn engine_counted_exact_match() {
        assert_eq!(find(b"a{3}", b"aa aaaa bb"), Some((3, 6)));
        assert_eq!(find(b"a{3}", b"aa bb"), None);
    }

    #[test]
    fn engine_counted_min_match() {
        assert_eq!(find(b"a{2,}", b"a aaa aaaaa"), Some((2, 5)));
        assert_eq!(find(b"a{2,}", b"single a"), None);
    }

    #[test]
    fn engine_counted_range_match() {
        assert_eq!(find(b"a{2,4}", b"aaaaa"), Some((0, 4)));
        assert_eq!(find(b"a{2,4}", b"aa"), Some((0, 2)));
        assert_eq!(find(b"a{2,4}", b"a"), None);
    }

    #[test]
    fn engine_counted_with_class() {
        assert_eq!(find(b"\\d{3}", b"abc 1234 def"), Some((4, 7)));
    }

    #[test]
    fn engine_phone_number_shape() {
        assert_eq!(
            find(b"\\d{3}-\\d{4}", b"call 555-1234 today"),
            Some((5, 13))
        );
    }

    #[test]
    fn engine_ipv4_like_pattern() {
        assert_eq!(
            find(
                b"\\d{1,3}\\.\\d{1,3}\\.\\d{1,3}\\.\\d{1,3}",
                b"server at 10.0.0.1 is up"
            ),
            Some((10, 18))
        );
    }

    fn parse_ci(pat: &[u8]) -> Ast {
        parse_with(pat, ParseOptions { case_insensitive: true }).unwrap()
    }

    fn engine_ci(pat: &[u8]) -> Engine {
        Engine::from_ast(&parse_ci(pat)).unwrap()
    }

    fn find_ci(pat: &[u8], input: &[u8]) -> Option<(usize, usize)> {
        engine_ci(pat).find(input)
    }

    fn ci_class(bytes: &[u8]) -> Ast {
        let mut s = ClassSet::new();
        for &b in bytes {
            s.add(b);
        }
        Ast::Class(s)
    }

    #[test]
    fn parse_ci_folds_lowercase_letter() {
        assert_eq!(parse_ci(b"a"), ci_class(&[b'a', b'A']));
    }

    #[test]
    fn parse_ci_folds_uppercase_letter() {
        assert_eq!(parse_ci(b"A"), ci_class(&[b'a', b'A']));
    }

    #[test]
    fn parse_ci_leaves_non_letter_literal_alone() {
        assert_eq!(parse_ci(b"7"), Ast::Literal(b'7'));
        assert_eq!(parse_ci(b"-"), Ast::Literal(b'-'));
    }

    #[test]
    fn parse_ci_default_options_is_case_sensitive() {
        assert_eq!(parse_with(b"a", ParseOptions::default()), Ok(Ast::Literal(b'a')));
    }

    #[test]
    fn parse_ci_class_range_folds_both_ways() {
        assert_eq!(parse_ci(b"[a-c]"), {
            let mut s = ClassSet::new();
            s.add_range(b'a', b'c');
            s.add_range(b'A', b'C');
            Ast::Class(s)
        });
        assert_eq!(parse_ci(b"[A-C]"), {
            let mut s = ClassSet::new();
            s.add_range(b'a', b'c');
            s.add_range(b'A', b'C');
            Ast::Class(s)
        });
    }

    #[test]
    fn parse_ci_negated_class_excludes_both_cases() {
        let mut s = ClassSet::new();
        s.add(b'a');
        s.add(b'A');
        s.negate();
        assert_eq!(parse_ci(b"[^a]"), Ast::Class(s));
    }

    #[test]
    fn engine_ci_literal_matches_mixed_case() {
        assert_eq!(find_ci(b"abc", b"--ABC--"), Some((2, 5)));
        assert_eq!(find_ci(b"abc", b"--AbC--"), Some((2, 5)));
        assert_eq!(find_ci(b"ERROR", b"oops error: bad"), Some((5, 10)));
    }

    #[test]
    fn engine_ci_class_matches_mixed_case() {
        assert_eq!(find_ci(b"[a-z]+", b"  HELLO  "), Some((2, 7)));
    }

    #[test]
    fn engine_ci_negated_class_rejects_both_cases() {
        assert_eq!(find_ci(b"[^a]+", b"AaAa"), None);
        assert_eq!(find_ci(b"[^a]+", b"AaXY"), Some((2, 4)));
    }

    #[test]
    fn engine_ci_keeps_digits_intact() {
        assert_eq!(find_ci(b"\\d{3}", b"id 7B 482 z"), Some((6, 9)));
    }

    #[test]
    fn engine_ci_alternation_each_branch_folds() {
        assert_eq!(find_ci(b"foo|bar", b"--BAR--"), Some((2, 5)));
        assert_eq!(find_ci(b"foo|bar", b"--FOO--"), Some((2, 5)));
    }

    #[test]
    fn engine_ci_anchored_match() {
        assert_eq!(find_ci(b"^hello$", b"HELLO"), Some((0, 5)));
        assert_eq!(find_ci(b"^hello$", b"HELLO!"), None);
    }
}
