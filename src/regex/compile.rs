// Ast -> Program (Glushkov NFA, anchor flags).

use super::ast::{Ast, ClassSet};

pub const MAX_POSITIONS: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Label {
    Literal(u8),
    AnyByte,
    Class(ClassSet),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Program {
    pub labels: Vec<Label>,
    pub first: u64,
    pub last: u64,
    pub follow: Vec<u64>,
    pub nullable: bool,
    pub start_anchor: bool,
    pub end_anchor: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompileError {
    TooManyPositions,
}

impl std::fmt::Display for CompileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CompileError::TooManyPositions => write!(
                f,
                "pattern too complex (more than {} positions)",
                MAX_POSITIONS
            ),
        }
    }
}

impl std::error::Error for CompileError {}

pub fn compile(ast: &Ast) -> Result<Program, CompileError> {
    let (start_anchor, end_anchor, core) = extract_anchors(ast);
    let mut builder = Builder::default();
    let (nullable, first, last) = builder.walk(&core)?;
    Ok(Program {
        labels: builder.labels,
        follow: builder.follow,
        first,
        last,
        nullable,
        start_anchor,
        end_anchor,
    })
}

fn extract_anchors(ast: &Ast) -> (bool, bool, Ast) {
    match ast {
        Ast::StartAnchor => (true, false, Ast::Empty),
        Ast::EndAnchor => (false, true, Ast::Empty),
        Ast::Concat(children) => {
            let mut kids: Vec<Ast> = children.clone();
            let mut start = false;
            let mut end = false;
            if kids.first() == Some(&Ast::StartAnchor) {
                start = true;
                kids.remove(0);
            }
            if kids.last() == Some(&Ast::EndAnchor) {
                end = true;
                kids.pop();
            }
            let core = match kids.len() {
                0 => Ast::Empty,
                1 => kids.into_iter().next().unwrap(),
                _ => Ast::Concat(kids),
            };
            (start, end, core)
        }
        other => (false, false, other.clone()),
    }
}

#[derive(Default)]
struct Builder {
    labels: Vec<Label>,
    follow: Vec<u64>,
}

impl Builder {
    fn assign(&mut self, label: Label) -> Result<u64, CompileError> {
        let p = self.labels.len();
        if p >= MAX_POSITIONS {
            return Err(CompileError::TooManyPositions);
        }
        self.labels.push(label);
        self.follow.push(0);
        Ok(1u64 << p)
    }

    fn walk(&mut self, ast: &Ast) -> Result<(bool, u64, u64), CompileError> {
        match ast {
            Ast::Empty | Ast::StartAnchor | Ast::EndAnchor => Ok((true, 0, 0)),
            Ast::Literal(b) => {
                let bit = self.assign(Label::Literal(*b))?;
                Ok((false, bit, bit))
            }
            Ast::AnyByte => {
                let bit = self.assign(Label::AnyByte)?;
                Ok((false, bit, bit))
            }
            Ast::Class(set) => {
                let bit = self.assign(Label::Class(*set))?;
                Ok((false, bit, bit))
            }
            Ast::Concat(children) => {
                let mut nullable = true;
                let mut first = 0u64;
                let mut tail = 0u64;
                let mut first_open = true;
                for child in children {
                    let (cn, cf, cl) = self.walk(child)?;
                    for_each_bit(tail, |p| self.follow[p] |= cf);
                    if first_open {
                        first |= cf;
                        if !cn {
                            first_open = false;
                        }
                    }
                    if cn {
                        tail |= cl;
                    } else {
                        tail = cl;
                    }
                    nullable = nullable && cn;
                }
                Ok((nullable, first, tail))
            }
            Ast::Alternate(children) => {
                let mut nullable = false;
                let mut first = 0u64;
                let mut last = 0u64;
                for child in children {
                    let (cn, cf, cl) = self.walk(child)?;
                    nullable |= cn;
                    first |= cf;
                    last |= cl;
                }
                Ok((nullable, first, last))
            }
            Ast::Star(inner) => {
                let (_, cf, cl) = self.walk(inner)?;
                for_each_bit(cl, |p| self.follow[p] |= cf);
                Ok((true, cf, cl))
            }
            Ast::Plus(inner) => {
                let (cn, cf, cl) = self.walk(inner)?;
                for_each_bit(cl, |p| self.follow[p] |= cf);
                Ok((cn, cf, cl))
            }
            Ast::Question(inner) => {
                let (_, cf, cl) = self.walk(inner)?;
                Ok((true, cf, cl))
            }
        }
    }
}

fn for_each_bit<F: FnMut(usize)>(mut bits: u64, mut f: F) {
    while bits != 0 {
        let p = bits.trailing_zeros() as usize;
        f(p);
        bits &= bits - 1;
    }
}
