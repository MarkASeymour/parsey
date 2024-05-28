use core::{ops::Deref, slice::Iter};
use colored::Colorize;

pub trait ByteSearchable {
    fn len(&self) -> usize;

    fn value_at(&self, index: usize) -> u8;

    fn iter(&self) -> Iter<u8>;
    
    fn stringify(&self) -> String;
    
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

    #[inline]
    fn stringify(&self) -> String {
        String::clone(self)
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
    #[inline]
    fn stringify(&self) -> String {
        String::from_utf8(self.clone()).unwrap()
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

    #[inline]
    fn stringify(&self) -> String {
        <dyn ByteSearchable>::stringify(*self)
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
    pub fn find_full_all_in<T: ByteSearchable>(&self, text: T, line_number: i32) {
        let line_text: String = text.stringify();
        let result: Vec<usize> = find_full(text, &self.pattern.clone(), &self.bad_char_map_rev, 0);
        display_and_format(result, line_text, self.pattern.clone(), line_number);

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
    let mut result = vec![];
    if text_len == 0 || pattern_len == 0 || text_len < pattern_len {
        return vec![];
    }

    let pattern_len_dec = pattern_len - 1;

    let first_pattern_char = pattern.value_at(0);

    let mut shift = text_len - 1;

    let start_index = pattern_len_dec;



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

fn display_and_format(result: Vec<usize>, text: String, pattern: Vec<u8>, line_num: i32) {
    for matched in result {
        let outable = ResultSet::from(matched, text.clone(), pattern.clone(), line_num).output_string;
        println!("{outable}")
    }

}



struct ResultSet {
    ps: String,
    ts: String,
    hit: usize,
    output_string: String,
    line_num: i32
}

impl ResultSet {
    fn from( hit: usize, text: String, pattern: Vec<u8>, line_num: i32) -> ResultSet {
        let ps = String::from_utf8(pattern).unwrap();
        let ts = text;
        let output_string = build_output(hit, ps.clone(), ts.clone(), line_num.to_string());

        ResultSet { ps, ts, hit, output_string, line_num }


    }
}

fn build_output(hit: usize, ps: String, ts: String, line_num: String) -> String {
    let ph_index: usize = (hit as i32 + ps.len() as i32) as usize;

    let t_arr: Vec<char> = ts.chars().collect();

    let prefix: String = t_arr.get(..hit).unwrap().iter().collect();

    let matched: String = t_arr.get(hit..(hit + (ps.len()))).unwrap().iter().collect();

    let suffix: String = t_arr.get(ph_index..).unwrap().iter().collect();

    format!(
        "{}: {}{}{}",
        line_num.bright_purple(),
        prefix,
        matched.red(),
        suffix

    )

}


