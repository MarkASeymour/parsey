// Parser output, compiler input.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClassSet {
    bits: [u64; 4],
}

impl ClassSet {
    pub fn new() -> Self {
        ClassSet { bits: [0; 4] }
    }

    pub fn add(&mut self, b: u8) {
        self.bits[(b >> 6) as usize] |= 1u64 << (b & 63);
    }

    pub fn add_range(&mut self, lo: u8, hi: u8) {
        for c in lo..=hi {
            self.add(c);
        }
    }

    pub fn negate(&mut self) {
        for w in self.bits.iter_mut() {
            *w = !*w;
        }
    }

    pub fn contains(&self, b: u8) -> bool {
        (self.bits[(b >> 6) as usize] >> (b & 63)) & 1 != 0
    }

    pub fn is_empty(&self) -> bool {
        self.bits.iter().all(|&w| w == 0)
    }

    pub fn union(&mut self, other: &ClassSet) {
        for i in 0..4 {
            self.bits[i] |= other.bits[i];
        }
    }

    pub fn fold_ascii_case(&mut self) {
        for b in b'a'..=b'z' {
            if self.contains(b) {
                self.add(b ^ 0x20);
            }
        }
        for b in b'A'..=b'Z' {
            if self.contains(b) {
                self.add(b ^ 0x20);
            }
        }
    }

    pub fn digit() -> Self {
        let mut s = Self::new();
        s.add_range(b'0', b'9');
        s
    }

    pub fn word() -> Self {
        let mut s = Self::new();
        s.add_range(b'a', b'z');
        s.add_range(b'A', b'Z');
        s.add_range(b'0', b'9');
        s.add(b'_');
        s
    }

    pub fn whitespace() -> Self {
        let mut s = Self::new();
        s.add(b' ');
        s.add(b'\t');
        s.add(b'\n');
        s.add(b'\r');
        s.add(0x0c);
        s.add(0x0b);
        s
    }
}

impl Default for ClassSet {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ast {
    Empty,
    Literal(u8),
    AnyByte,
    Class(ClassSet),
    StartAnchor,
    EndAnchor,
    Concat(Vec<Ast>),
    Alternate(Vec<Ast>),
    Star(Box<Ast>),
    Plus(Box<Ast>),
    Question(Box<Ast>),
}
