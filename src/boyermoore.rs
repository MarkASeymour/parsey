// Literal prefix fast skip. Engine invokes the regex NFA only at hits.

/// Boyer Moore substring searcher.
///
/// Build it once for a pattern, then reuse it across many haystacks.
pub struct Searcher {
    pattern: Vec<u8>,
    bad_char: [usize; 256],
}

impl Searcher {
    /// Returns `None` if the pattern is empty.
    pub fn new(pattern: impl Into<Vec<u8>>) -> Option<Self> {
        let pattern = pattern.into();
        let plen = pattern.len();
        if plen == 0 {
            return None;
        }

        let mut bad_char = [plen; 256];
        for (i, &c) in pattern.iter().enumerate().rev().take(plen - 1) {
            bad_char[c as usize] = i;
        }

        Some(Self { pattern, bad_char })
    }

    #[allow(dead_code)]
    pub fn pattern(&self) -> &[u8] {
        &self.pattern
    }

    /// Returns the byte offset of every occurrence of the pattern in `text`,
    /// in ascending order.
    pub fn find_all(&self, text: &[u8]) -> Vec<usize> {
        let tlen = text.len();
        let plen = self.pattern.len();
        if tlen < plen {
            return Vec::new();
        }

        let plen_dec = plen - 1;
        let first = self.pattern[0];
        let mut shift = tlen - 1;
        let mut result = Vec::new();

        'outer: loop {
            for (i, &pc) in self.pattern.iter().enumerate() {
                if text[shift - plen_dec + i] != pc {
                    match self.next_shift(text, shift, first) {
                        Some(next) => {
                            shift = next;
                            continue 'outer;
                        }
                        None => break 'outer,
                    }
                }
            }

            result.push(shift - plen_dec);
            if shift == plen_dec {
                break;
            }
            match self.next_shift(text, shift, first) {
                Some(next) => shift = next,
                None => break,
            }
        }

        result.reverse();
        result
    }

    #[inline]
    fn next_shift(&self, text: &[u8], shift: usize, first: u8) -> Option<usize> {
        let plen = self.pattern.len();
        let plen_dec = plen - 1;

        if shift < plen {
            return None;
        }
        let a = self.bad_char[text[shift - plen_dec] as usize];
        let c = text[shift - plen];
        let b = if c == first {
            1
        } else {
            self.bad_char[c as usize] + 1
        };
        let step = a.max(b);
        let next = shift.checked_sub(step)?;
        if next < plen_dec {
            None
        } else {
            Some(next)
        }
    }
}
