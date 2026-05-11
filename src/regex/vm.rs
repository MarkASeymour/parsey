// Glushkov NFA simulation using bitwise ops. Engine wraps a Program plus reverse, byte table, optional BM prefix.

use super::ast::Ast;
use super::compile::{compile, CompileError, Label, Program};
use crate::boyermoore::Searcher;

pub struct ByteTable {
    pub b: [u64; 256],
}

pub fn build_byte_table(prog: &Program) -> ByteTable {
    let mut b = [0u64; 256];
    for (i, label) in prog.labels.iter().enumerate() {
        let bit = 1u64 << i;
        match label {
            Label::Literal(c) => b[*c as usize] |= bit,
            Label::AnyByte => {
                for slot in b.iter_mut() {
                    *slot |= bit;
                }
            }
            Label::Class(set) => {
                for c in 0..=255u8 {
                    if set.contains(c) {
                        b[c as usize] |= bit;
                    }
                }
            }
        }
    }
    ByteTable { b }
}

#[inline(always)]
fn union_follow(d: u64, follow: &[u64]) -> u64 {
    let mut next = 0u64;
    let mut bits = d;
    while bits != 0 {
        let p = bits.trailing_zeros() as usize;
        next |= follow[p];
        bits &= bits - 1;
    }
    next
}

pub fn find_anchored(prog: &Program, table: &ByteTable, input: &[u8]) -> Option<(usize, usize)> {
    let last = prog.last;
    let follow = &prog.follow;
    let b = &table.b;
    let mut end: Option<usize> = if prog.nullable { Some(0) } else { None };
    let mut d = 0u64;

    for (i, &c) in input.iter().enumerate() {
        let cb = b[c as usize];
        d = if i == 0 {
            prog.first & cb
        } else {
            if d == 0 {
                break;
            }
            union_follow(d, follow) & cb
        };
        if d & last != 0 {
            end = Some(i + 1);
        }
    }

    end.map(|e| (0, e))
}

fn reverse_program(prog: &Program) -> Program {
    let n = prog.labels.len();
    let mut follow_rev = vec![0u64; n];
    for p in 0..n {
        let mut bits = prog.follow[p];
        while bits != 0 {
            let q = bits.trailing_zeros() as usize;
            follow_rev[q] |= 1u64 << p;
            bits &= bits - 1;
        }
    }
    Program {
        labels: prog.labels.clone(),
        first: prog.last,
        last: prog.first,
        follow: follow_rev,
        nullable: prog.nullable,
        start_anchor: prog.start_anchor,
        end_anchor: prog.end_anchor,
    }
}

fn find_forward_end(prog: &Program, table: &ByteTable, input: &[u8]) -> Option<usize> {
    let last = prog.last;
    let follow = &prog.follow;
    let first = prog.first;
    let b = &table.b;
    let mut d = 0u64;

    for (i, &c) in input.iter().enumerate() {
        d = (union_follow(d, follow) | first) & b[c as usize];
        if d & last != 0 {
            return Some(i + 1);
        }
    }

    if prog.nullable {
        Some(0)
    } else {
        None
    }
}

fn find_reverse_start(prog: &Program, table: &ByteTable, input: &[u8], end: usize) -> usize {
    let last = prog.last;
    let follow = &prog.follow;
    let first = prog.first;
    let b = &table.b;
    let mut d = 0u64;
    let mut start = end;

    for i in (0..end).rev() {
        d = (union_follow(d, follow) | first) & b[input[i] as usize];
        if d & last != 0 {
            start = i;
        }
        if d == 0 {
            break;
        }
    }

    start
}

pub(crate) fn literal_prefix(ast: &Ast) -> Vec<u8> {
    let mut prefix = Vec::new();
    walk_prefix(ast, &mut prefix);
    prefix
}

fn walk_prefix(ast: &Ast, prefix: &mut Vec<u8>) -> bool {
    match ast {
        Ast::Empty | Ast::StartAnchor | Ast::EndAnchor => true,
        Ast::Literal(b) => {
            prefix.push(*b);
            true
        }
        Ast::Concat(children) => {
            for c in children {
                if !walk_prefix(c, prefix) {
                    return false;
                }
            }
            true
        }
        Ast::Plus(inner) => {
            walk_prefix(inner, prefix);
            false
        }
        Ast::Star(_)
        | Ast::Question(_)
        | Ast::Alternate(_)
        | Ast::AnyByte
        | Ast::Class(_) => false,
    }
}

pub struct Engine {
    forward: Program,
    reverse: Program,
    table: ByteTable,
    prefix: Option<Searcher>,
}

impl Engine {
    pub fn from_ast(ast: &Ast) -> Result<Self, CompileError> {
        let forward = compile(ast)?;
        let reverse = reverse_program(&forward);
        let table = build_byte_table(&forward);
        let prefix_bytes = literal_prefix(ast);
        let prefix = if forward.start_anchor || prefix_bytes.len() < 2 {
            None
        } else {
            Searcher::new(prefix_bytes)
        };
        Ok(Self {
            forward,
            reverse,
            table,
            prefix,
        })
    }

    pub fn nullable(&self) -> bool {
        self.forward.nullable
    }

    // Dispatch: anchored shortcuts, BM prefix fast path, or unanchored
    // (forward to find end, reverse to find leftmost start, then anchored extend).
    pub fn find(&self, input: &[u8]) -> Option<(usize, usize)> {
        if self.forward.start_anchor {
            return self.find_start_anchored(input);
        }
        if self.forward.end_anchor {
            return self.find_end_anchored(input);
        }
        if let Some(scanner) = &self.prefix {
            for hit in scanner.find_all(input) {
                if let Some((_, end)) = find_anchored(&self.forward, &self.table, &input[hit..]) {
                    if end > 0 {
                        return Some((hit, hit + end));
                    }
                }
            }
            if self.forward.nullable {
                Some((0, 0))
            } else {
                None
            }
        } else {
            self.find_without_prefix(input)
        }
    }

    fn find_start_anchored(&self, input: &[u8]) -> Option<(usize, usize)> {
        let (_, end) = find_anchored(&self.forward, &self.table, input)?;
        if self.forward.end_anchor && end != input.len() {
            return None;
        }
        Some((0, end))
    }

    fn find_end_anchored(&self, input: &[u8]) -> Option<(usize, usize)> {
        let last = self.forward.last;
        let first = self.forward.first;
        let follow = &self.forward.follow;
        let b = &self.table.b;
        let mut d = 0u64;
        for &c in input {
            d = (union_follow(d, follow) | first) & b[c as usize];
        }
        if d & last == 0 {
            if self.forward.nullable {
                return Some((input.len(), input.len()));
            }
            return None;
        }
        let end = input.len();
        let start = find_reverse_start(&self.reverse, &self.table, input, end);
        Some((start, end))
    }

    fn find_without_prefix(&self, input: &[u8]) -> Option<(usize, usize)> {
        let end = find_forward_end(&self.forward, &self.table, input)?;
        if end == 0 {
            return Some((0, 0));
        }
        let start = find_reverse_start(&self.reverse, &self.table, input, end);
        let extended = find_anchored(&self.forward, &self.table, &input[start..])
            .map(|(_, e)| e)
            .unwrap_or(end - start);
        Some((start, start + extended))
    }

    pub fn find_all(&self, input: &[u8], out: &mut Vec<(usize, usize)>) {
        out.clear();
        if self.forward.start_anchor || self.forward.end_anchor {
            if let Some(m) = self.find(input) {
                if m.1 > m.0 {
                    out.push(m);
                }
            }
            return;
        }
        if let Some(scanner) = &self.prefix {
            let mut cursor = 0;
            for hit in scanner.find_all(input) {
                if hit < cursor {
                    continue;
                }
                if let Some((_, end)) = find_anchored(&self.forward, &self.table, &input[hit..]) {
                    if end > 0 {
                        out.push((hit, hit + end));
                        cursor = hit + end;
                    }
                }
            }
        } else {
            let mut pos = 0;
            while pos <= input.len() {
                let slice = &input[pos..];
                match self.find_without_prefix(slice) {
                    Some((s, e)) if e > s => {
                        out.push((pos + s, pos + e));
                        pos += e;
                    }
                    _ => break,
                }
            }
        }
    }
}
