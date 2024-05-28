use std::env;
use std::fs::File;
use std::io::{self, prelude::*, BufReader};
use std::collections::HashMap;
use std::cmp;
use boyer_moore_magiclen::*;

fn main() -> io::Result<()> {
    let args: Vec<String> = env::args().collect();

    let input_pattern: &String = &args[1];

    let input_file_path: &String = &args[2];

    let mut bad_char_table: HashMap<char, i32> =  bad_char_table(input_pattern.to_string());
    let mut line_num: i32 = 1;

    let file = File::open(input_file_path)?;
    let reader = BufReader::new(file);
    
     for line in reader.lines() {

        let bm_bytes = BMByte::from(input_pattern).unwrap();
        let item: Vec<usize> = bm_bytes.rfind_full_all_in(line.unwrap());
        
        //for hit in item {
       // }

        
       // compare(input_pattern.chars().collect(), line.unwrap().chars().collect(),  &mut bad_char_table, line_num);
        line_num = line_num+1;


     }
    
     Ok(())
} 

fn bad_char_table(pattern: String) -> HashMap<char, i32> {
    
    let mut table = HashMap::new();

    let char_arr: Vec<char> = pattern.chars().collect();
    let length: i32 = char_arr.len() as i32;

    for (index, char) in char_arr.iter().enumerate() {
       
        table.insert(*char, cmp::max(1, length - (index as i32) - 1));


    }

    return table;

}
/*
fn compare(pattern: Vec<char>, text: Vec<char>, bad_char_table: &mut HashMap<char,i32>, line_num: i32){
   
    //let pat_loc = (pattern.len() as i32)-1;
    //let text_loc = pat_loc;
    //
    let line_string: String = String::from_iter(&text);
    let mut shift: i32 = 0;

    let m: i32 = pattern.len() as i32;
    let n: i32 = text.len() as i32;

    while shift <= (n - m) {

        let mut j: i32 = m - 1;

        while j >= 0 && pattern[j as usize] == text[(shift + j) as usize] {j = j - 1;}

            if j < 0 {
                
                println!("Patterns occur at shift = {}", shift);
                println!("{}: {}", line_num, line_string);
                shift += if shift + m < n {m - *bad_char_table.entry(text[(shift + m)as usize]).or_insert(m)} else {1};

                
            } else {shift += cmp::max(1, j - *bad_char_table.entry(text[(shift + j)as usize]).or_insert(m))}


    }





}
*/
