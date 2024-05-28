mod boyermoore;
use std::env;
use std::fs::File;
use std::io::{self, prelude::*, BufReader};
use colored::Colorize;

fn main() -> io::Result<()> {
    let args: Vec<String> = env::args().collect();

    let input_pattern: &String = &args[1];

    let input_file_path: &String = &args[2];

    let mut line_num: i32 = 1;

    let file = File::open(input_file_path)?;
    let reader = BufReader::new(file);

    //TODO: total f'ing refactor this hogwash
     for line in reader.lines() {
        let line_string: String = line.unwrap().clone();
         let line_string_for_byte: String = line_string.clone();
        let bm_bytes = boyermoore::Byte::from(input_pattern).unwrap();
        let hits: Vec<usize> = bm_bytes.find_full_all_in(line_string_for_byte);
        for hit in hits {
            let after_hit_index: i32 = hit as i32 + input_pattern.len() as i32;
            let thing: Vec<char> = line_string.chars().collect();
            let prefix: String = thing.get(..hit).unwrap().iter().collect();
            let hit: String = thing.get(hit..(hit + (input_pattern.len()))).unwrap().iter().collect();
            let suffix: String = thing.get(after_hit_index as usize..).unwrap().iter().collect();

            let output_string = format!(
                "{}: {}{}{}",
                line_num.to_string().cyan(),
                prefix,
                hit.red(),
                suffix
            );
            println!("{output_string}")

       }
        line_num = line_num+1;


     }
    
     Ok(())
}
