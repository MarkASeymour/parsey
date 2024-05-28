use core::{ops::Deref, slice::Iter};

pub trait ByteSearchable {
    fn len(&self) -> usize;

    fn value_at(&self, index: usize) -> u8;

    fn iter(&self) -> Iter<u8>;
}
//
impl ByteSearchable for String {
    #[inline]
    fn len(&self) -> usize {
        String::len(self)
    }

    #[inline]
    fn value_at(&self, index: usize) -> u8 {
        self.as_bytes()[index]
    }

    #[inline]
    fn iter(&self) -> Iter<u8> {
        self.as_bytes().iter()
    }
}
impl ByteSearchable for Vec<u8> {
    #[inline]
    fn len(&self) -> usize {
        Vec::len(self)
    }

    #[inline]
    fn value_at(&self, index: usize) -> u8 {
        self[index]
    }

    #[inline]
    fn iter(&self) -> Iter<u8> {
        self.as_slice().iter()
    }
}
impl<T: ByteSearchable> ByteSearchable for &T {
    #[inline]
    fn len(&self) -> usize {
        <dyn ByteSearchable>::len(*self)
    }

    #[inline]
    fn value_at(&self, index: usize) -> u8 {
        <dyn ByteSearchable>::value_at(*self, index)
    }

    #[inline]
    fn iter(&self) -> Iter<u8> {
        <dyn ByteSearchable>::iter(*self)
    }
}

pub struct BadCharMapByte {
    t: [usize; 256],
}


impl Deref for BadCharMapByte {
    type Target = [usize];

    #[inline]
    fn deref(&self) -> &[usize] {
        self.t.as_ref()
    }
}

pub struct BadCharMapByteRev {
    t: [usize; 256],
}


impl Deref for BadCharMapByteRev {
    type Target = [usize];

    #[inline]
    fn deref(&self) -> &[usize] {
        self.t.as_ref()
    }
}

impl BadCharMapByteRev {
    pub fn create_bad_char_map<T: ByteSearchable>(
        pattern: T,
    ) -> Option<BadCharMapByteRev> {
        let pattern_len = pattern.len();

        if pattern_len == 0 {
            return None;
        }

        let pattern_len_dec = pattern_len - 1;

        let mut bad_char_map = [pattern_len; 256];

        for (i, c) in
        pattern.iter().enumerate().rev().take(pattern_len_dec).map(|(i, &c)| (i, c as usize))
        {
            bad_char_map[c] = i;
        }

        Some(BadCharMapByteRev {
            t: bad_char_map
        })
    }
}


pub struct Byte {
    bad_char_map_rev: BadCharMapByteRev,
    pattern:                Vec<u8>,
}

impl Byte {
    pub fn from<T: ByteSearchable>(pattern: T) -> Option<Byte> {
        let bad_char_map_rev = BadCharMapByteRev::create_bad_char_map(&pattern)?;

        Some(Byte {
            bad_char_map_rev,
            pattern: pattern.iter().copied().collect(),
        })
    }
}

impl Byte {
    pub fn find_full_all_in<T: ByteSearchable>(&self, text: T) -> Vec<usize> {
        find_full(text, &self.pattern, &self.bad_char_map_rev, 0)
    }
}

pub fn find_full<TT: ByteSearchable, TP: ByteSearchable>(
    text: TT,
    pattern: TP,
    bad_char_map: &BadCharMapByteRev,
    limit: usize,
) -> Vec<usize> {
    let text_len = text.len();
    let pattern_len = pattern.len();

    if text_len == 0 || pattern_len == 0 || text_len < pattern_len {
        return vec![];
    }

    let pattern_len_dec = pattern_len - 1;

    let first_pattern_char = pattern.value_at(0);

    let mut shift = text_len - 1;

    let start_index = pattern_len_dec;

    let mut result = vec![];

    'outer: loop {
        for (i, pc) in pattern.iter().copied().enumerate() {
            if text.value_at(shift - pattern_len_dec + i) != pc {
                if shift < pattern_len {
                    break 'outer;
                }
                let s = bad_char_map[text.value_at(shift - pattern_len_dec) as usize].max({
                    let c = text.value_at(shift - pattern_len);

                    if c == first_pattern_char {
                        1
                    } else {
                        bad_char_map[c as usize] + 1
                    }
                });
                if shift < s {
                    break 'outer;
                }
                shift -= s;
                if shift < start_index {
                    break 'outer;
                }
                continue 'outer;
            }
        }
        result.push(shift - pattern_len_dec);

        if shift == start_index {
            break;
        }

        if result.len() == limit {
            break;
        }

        let s = bad_char_map[text.value_at(shift - pattern_len_dec) as usize].max({
            let c = text.value_at(shift - pattern_len);

            if c == first_pattern_char {
                1
            } else {
                bad_char_map[c as usize] + 1
            }
        });
        if shift < s {
            break;
        }
        shift -= s;
        if shift < start_index {
            break;
        }
    }

    result
}