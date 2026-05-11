// Pattern bytes -> Ast.

use super::ast::{Ast, ClassSet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    pub kind: ParseErrorKind,
    pub position: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseErrorKind {
    UnmatchedOpenParen,
    UnmatchedCloseParen,
    NothingToRepeat,
    InvalidEscape(u8),
    TrailingBackslash,
    UnmatchedOpenBracket,
    UnmatchedCloseBracket,
    EmptyCharacterClass,
    InvalidClassRange,
    AnchorNotAtBoundary,
    InvalidCountedRepetition,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.kind {
            ParseErrorKind::UnmatchedOpenParen => {
                write!(f, "unmatched '(' at position {}", self.position)
            }
            ParseErrorKind::UnmatchedCloseParen => {
                write!(f, "unmatched ')' at position {}", self.position)
            }
            ParseErrorKind::NothingToRepeat => {
                write!(f, "nothing to repeat at position {}", self.position)
            }
            ParseErrorKind::InvalidEscape(b) => {
                write!(
                    f,
                    "invalid escape '\\{}' at position {}",
                    *b as char, self.position
                )
            }
            ParseErrorKind::TrailingBackslash => {
                write!(f, "trailing backslash at position {}", self.position)
            }
            ParseErrorKind::UnmatchedOpenBracket => {
                write!(f, "unmatched '[' at position {}", self.position)
            }
            ParseErrorKind::UnmatchedCloseBracket => {
                write!(f, "unmatched ']' at position {}", self.position)
            }
            ParseErrorKind::EmptyCharacterClass => {
                write!(f, "empty character class at position {}", self.position)
            }
            ParseErrorKind::InvalidClassRange => {
                write!(f, "invalid class range at position {}", self.position)
            }
            ParseErrorKind::AnchorNotAtBoundary => {
                write!(f, "anchor not at pattern boundary at position {}", self.position)
            }
            ParseErrorKind::InvalidCountedRepetition => {
                write!(f, "invalid counted repetition at position {}", self.position)
            }
        }
    }
}

impl std::error::Error for ParseError {}

pub fn parse(pattern: &[u8]) -> Result<Ast, ParseError> {
    let mut start = 0;
    let mut end = pattern.len();
    let mut start_anchor = false;
    let mut end_anchor = false;

    if pattern.first() == Some(&b'^') {
        start_anchor = true;
        start = 1;
    }
    if end > start && pattern[end - 1] == b'$' && !is_escaped_at(pattern, end - 1) {
        end_anchor = true;
        end -= 1;
    }

    let inner = &pattern[start..end];
    let mut p = Parser {
        input: inner,
        pos: 0,
        offset: start,
    };
    let ast = p.parse_alternation()?;
    if p.pos < p.input.len() {
        return Err(ParseError {
            kind: ParseErrorKind::UnmatchedCloseParen,
            position: p.offset + p.pos,
        });
    }

    Ok(wrap_anchors(start_anchor, end_anchor, ast))
}

fn is_escaped_at(pattern: &[u8], pos: usize) -> bool {
    let mut count = 0;
    let mut i = pos;
    while i > 0 && pattern[i - 1] == b'\\' {
        count += 1;
        i -= 1;
    }
    count % 2 == 1
}

fn wrap_anchors(start_anchor: bool, end_anchor: bool, inner: Ast) -> Ast {
    if !start_anchor && !end_anchor {
        return inner;
    }
    let mut children: Vec<Ast> = Vec::new();
    if start_anchor {
        children.push(Ast::StartAnchor);
    }
    match inner {
        Ast::Concat(inner_children) => children.extend(inner_children),
        Ast::Empty => {}
        other => children.push(other),
    }
    if end_anchor {
        children.push(Ast::EndAnchor);
    }
    match children.len() {
        0 => Ast::Empty,
        1 => children.into_iter().next().unwrap(),
        _ => Ast::Concat(children),
    }
}

struct Parser<'a> {
    input: &'a [u8],
    pos: usize,
    offset: usize,
}

impl<'a> Parser<'a> {
    fn peek(&self) -> Option<u8> {
        self.input.get(self.pos).copied()
    }

    fn peek_ahead(&self, n: usize) -> Option<u8> {
        self.input.get(self.pos + n).copied()
    }

    fn bump(&mut self) -> Option<u8> {
        let b = self.peek()?;
        self.pos += 1;
        Some(b)
    }

    fn err(&self, kind: ParseErrorKind, pos: usize) -> ParseError {
        ParseError {
            kind,
            position: self.offset + pos,
        }
    }

    fn parse_alternation(&mut self) -> Result<Ast, ParseError> {
        let mut branches = vec![self.parse_concat()?];
        while self.peek() == Some(b'|') {
            self.bump();
            branches.push(self.parse_concat()?);
        }
        if branches.len() == 1 {
            Ok(branches.pop().unwrap())
        } else {
            Ok(Ast::Alternate(branches))
        }
    }

    fn parse_concat(&mut self) -> Result<Ast, ParseError> {
        let mut items: Vec<Ast> = Vec::new();
        loop {
            match self.peek() {
                None | Some(b'|') | Some(b')') => break,
                _ => items.push(self.parse_repetition()?),
            }
        }
        match items.len() {
            0 => Ok(Ast::Empty),
            1 => Ok(items.pop().unwrap()),
            _ => Ok(Ast::Concat(items)),
        }
    }

    fn parse_repetition(&mut self) -> Result<Ast, ParseError> {
        let atom_pos = self.pos;
        let atom = self.parse_atom()?;
        match self.peek() {
            Some(b'*') | Some(b'+') | Some(b'?') => {
                let q = self.bump().unwrap();
                if matches!(atom, Ast::Empty) {
                    return Err(self.err(ParseErrorKind::NothingToRepeat, atom_pos));
                }
                Ok(match q {
                    b'*' => Ast::Star(Box::new(atom)),
                    b'+' => Ast::Plus(Box::new(atom)),
                    b'?' => Ast::Question(Box::new(atom)),
                    _ => unreachable!(),
                })
            }
            Some(b'{') if self.looks_like_counted_rep() => {
                self.parse_counted_rep(atom, atom_pos)
            }
            _ => Ok(atom),
        }
    }

    fn looks_like_counted_rep(&self) -> bool {
        let mut i = self.pos + 1;
        let start = i;
        while let Some(&b) = self.input.get(i) {
            if !b.is_ascii_digit() {
                break;
            }
            i += 1;
        }
        if i == start {
            return false;
        }
        match self.input.get(i) {
            Some(&b'}') => true,
            Some(&b',') => {
                i += 1;
                while let Some(&b) = self.input.get(i) {
                    if !b.is_ascii_digit() {
                        break;
                    }
                    i += 1;
                }
                self.input.get(i) == Some(&b'}')
            }
            _ => false,
        }
    }

    fn parse_counted_rep(&mut self, atom: Ast, atom_pos: usize) -> Result<Ast, ParseError> {
        let brace_pos = self.pos;
        self.bump();
        let n = self.parse_count(brace_pos)?;
        let m = match self.peek() {
            Some(b',') => {
                self.bump();
                if self.peek() == Some(b'}') {
                    None
                } else {
                    Some(self.parse_count(brace_pos)?)
                }
            }
            Some(b'}') => Some(n),
            _ => return Err(self.err(ParseErrorKind::InvalidCountedRepetition, brace_pos)),
        };
        if self.peek() != Some(b'}') {
            return Err(self.err(ParseErrorKind::InvalidCountedRepetition, brace_pos));
        }
        self.bump();
        if matches!(atom, Ast::Empty) {
            return Err(self.err(ParseErrorKind::NothingToRepeat, atom_pos));
        }
        let mut children: Vec<Ast> = Vec::new();
        for _ in 0..n {
            children.push(atom.clone());
        }
        match m {
            Some(m_val) => {
                if n > m_val {
                    return Err(self.err(ParseErrorKind::InvalidCountedRepetition, brace_pos));
                }
                for _ in n..m_val {
                    children.push(Ast::Question(Box::new(atom.clone())));
                }
            }
            None => {
                children.push(Ast::Star(Box::new(atom)));
            }
        }
        Ok(match children.len() {
            0 => Ast::Empty,
            1 => children.into_iter().next().unwrap(),
            _ => Ast::Concat(children),
        })
    }

    fn parse_count(&mut self, brace_pos: usize) -> Result<usize, ParseError> {
        let start = self.pos;
        let mut n: usize = 0;
        while let Some(b) = self.peek() {
            if !b.is_ascii_digit() {
                break;
            }
            n = n
                .checked_mul(10)
                .and_then(|n| n.checked_add((b - b'0') as usize))
                .ok_or_else(|| self.err(ParseErrorKind::InvalidCountedRepetition, brace_pos))?;
            self.bump();
        }
        if self.pos == start {
            return Err(self.err(ParseErrorKind::InvalidCountedRepetition, brace_pos));
        }
        Ok(n)
    }

    fn parse_atom(&mut self) -> Result<Ast, ParseError> {
        let start = self.pos;
        match self.peek() {
            None | Some(b'|') | Some(b')') => Ok(Ast::Empty),
            Some(b'(') => {
                self.bump();
                let inner = self.parse_alternation()?;
                match self.peek() {
                    Some(b')') => {
                        self.bump();
                        Ok(inner)
                    }
                    _ => Err(self.err(ParseErrorKind::UnmatchedOpenParen, start)),
                }
            }
            Some(b'.') => {
                self.bump();
                Ok(Ast::AnyByte)
            }
            Some(b'[') => self.parse_class(),
            Some(b']') => Err(self.err(ParseErrorKind::UnmatchedCloseBracket, start)),
            Some(b'^') | Some(b'$') => {
                Err(self.err(ParseErrorKind::AnchorNotAtBoundary, start))
            }
            Some(b'*') | Some(b'+') | Some(b'?') => {
                Err(self.err(ParseErrorKind::NothingToRepeat, start))
            }
            Some(b'\\') => {
                self.bump();
                match self.peek() {
                    None => Err(self.err(ParseErrorKind::TrailingBackslash, start)),
                    Some(b'd') => {
                        self.bump();
                        Ok(Ast::Class(ClassSet::digit()))
                    }
                    Some(b'D') => {
                        self.bump();
                        let mut s = ClassSet::digit();
                        s.negate();
                        Ok(Ast::Class(s))
                    }
                    Some(b'w') => {
                        self.bump();
                        Ok(Ast::Class(ClassSet::word()))
                    }
                    Some(b'W') => {
                        self.bump();
                        let mut s = ClassSet::word();
                        s.negate();
                        Ok(Ast::Class(s))
                    }
                    Some(b's') => {
                        self.bump();
                        Ok(Ast::Class(ClassSet::whitespace()))
                    }
                    Some(b'S') => {
                        self.bump();
                        let mut s = ClassSet::whitespace();
                        s.negate();
                        Ok(Ast::Class(s))
                    }
                    Some(b'n') => {
                        self.bump();
                        Ok(Ast::Literal(b'\n'))
                    }
                    Some(b't') => {
                        self.bump();
                        Ok(Ast::Literal(b'\t'))
                    }
                    Some(b'r') => {
                        self.bump();
                        Ok(Ast::Literal(b'\r'))
                    }
                    Some(b) if is_meta(b) => {
                        self.bump();
                        Ok(Ast::Literal(b))
                    }
                    Some(b) => Err(self.err(ParseErrorKind::InvalidEscape(b), start)),
                }
            }
            Some(b'{') | Some(b'}') => {
                let b = self.bump().unwrap();
                Ok(Ast::Literal(b))
            }
            Some(b) => {
                self.bump();
                Ok(Ast::Literal(b))
            }
        }
    }

    fn parse_class(&mut self) -> Result<Ast, ParseError> {
        let bracket_pos = self.pos;
        self.bump();

        let mut negate = false;
        if self.peek() == Some(b'^') {
            negate = true;
            self.bump();
        }

        let mut set = ClassSet::new();
        let mut added = false;

        loop {
            match self.peek() {
                None => return Err(self.err(ParseErrorKind::UnmatchedOpenBracket, bracket_pos)),
                Some(b']') => break,
                _ => {}
            }

            match self.parse_class_atom(bracket_pos)? {
                ClassAtom::Set(s) => {
                    set.union(&s);
                    added = true;
                }
                ClassAtom::Byte(a) => {
                    if self.peek() == Some(b'-') && self.peek_ahead(1) != Some(b']') {
                        let dash_pos = self.pos;
                        self.bump();
                        if self.peek().is_none() {
                            return Err(self.err(ParseErrorKind::UnmatchedOpenBracket, bracket_pos));
                        }
                        let high = match self.parse_class_atom(bracket_pos)? {
                            ClassAtom::Byte(b) => b,
                            ClassAtom::Set(_) => {
                                return Err(self.err(ParseErrorKind::InvalidClassRange, dash_pos));
                            }
                        };
                        if a > high {
                            return Err(self.err(ParseErrorKind::InvalidClassRange, dash_pos));
                        }
                        set.add_range(a, high);
                    } else {
                        set.add(a);
                    }
                    added = true;
                }
            }
        }

        self.bump();

        if !added && !negate {
            return Err(self.err(ParseErrorKind::EmptyCharacterClass, bracket_pos));
        }

        if negate {
            set.negate();
        }
        Ok(Ast::Class(set))
    }

    fn parse_class_atom(&mut self, bracket_pos: usize) -> Result<ClassAtom, ParseError> {
        let start = self.pos;
        match self.peek() {
            None => Err(self.err(ParseErrorKind::UnmatchedOpenBracket, bracket_pos)),
            Some(b'\\') => {
                self.bump();
                match self.peek() {
                    None => Err(self.err(ParseErrorKind::TrailingBackslash, start)),
                    Some(b'd') => {
                        self.bump();
                        Ok(ClassAtom::Set(ClassSet::digit()))
                    }
                    Some(b'D') => {
                        self.bump();
                        let mut s = ClassSet::digit();
                        s.negate();
                        Ok(ClassAtom::Set(s))
                    }
                    Some(b'w') => {
                        self.bump();
                        Ok(ClassAtom::Set(ClassSet::word()))
                    }
                    Some(b'W') => {
                        self.bump();
                        let mut s = ClassSet::word();
                        s.negate();
                        Ok(ClassAtom::Set(s))
                    }
                    Some(b's') => {
                        self.bump();
                        Ok(ClassAtom::Set(ClassSet::whitespace()))
                    }
                    Some(b'S') => {
                        self.bump();
                        let mut s = ClassSet::whitespace();
                        s.negate();
                        Ok(ClassAtom::Set(s))
                    }
                    Some(b'n') => {
                        self.bump();
                        Ok(ClassAtom::Byte(b'\n'))
                    }
                    Some(b't') => {
                        self.bump();
                        Ok(ClassAtom::Byte(b'\t'))
                    }
                    Some(b'r') => {
                        self.bump();
                        Ok(ClassAtom::Byte(b'\r'))
                    }
                    Some(b) => {
                        self.bump();
                        Ok(ClassAtom::Byte(b))
                    }
                }
            }
            Some(b) => {
                self.bump();
                Ok(ClassAtom::Byte(b))
            }
        }
    }
}

enum ClassAtom {
    Byte(u8),
    Set(ClassSet),
}

fn is_meta(b: u8) -> bool {
    matches!(
        b,
        b'.' | b'*'
            | b'+'
            | b'?'
            | b'('
            | b')'
            | b'|'
            | b'\\'
            | b'^'
            | b'$'
            | b'['
            | b']'
            | b'{'
            | b'}'
    )
}
