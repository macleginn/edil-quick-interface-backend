use serde::{Deserialize, Serialize};
use serde_json;
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader, Read};
use unicode_segmentation::UnicodeSegmentation;

#[derive(Clone, Serialize)]
pub struct Candidate {
    pub entry_id: usize,
    pub entry: String,
}

impl Candidate {
    pub fn new(entry_id: usize, entry: String) -> Candidate {
        Candidate { entry_id, entry }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct LemmaInfo {
    pub id: usize,
    pub headword: String,
}

impl LemmaInfo {
    pub fn new(lemma_id: usize, headword: String) -> LemmaInfo {
        LemmaInfo {
            id: lemma_id,
            headword,
        }
    }
}

pub fn get_variants() -> (HashMap<String, Vec<Candidate>>, Vec<String>) {
    let mut variant_map = HashMap::new();
    let mut sorted_variants = Vec::new();
    let input = File::open("forms.csv").expect("Couldn't open the variant file.");
    let buffered = BufReader::new(input);
    for line in buffered.lines().skip(1) {
        match line {
            Ok(line) => {
                let fields: Vec<&str> = line.split(',').collect();
                assert_eq!(fields.len(), 3);
                let key = fields[1].to_string();
                let candidate = Candidate::new(
                    fields[0].parse::<usize>().expect("Bad id!"),
                    fields[2].to_string(),
                );
                match variant_map.entry(key) {
                    Entry::Vacant(e) => {
                        sorted_variants.push(fields[1].to_string());
                        e.insert(vec![candidate]);
                    }
                    Entry::Occupied(mut e) => {
                        e.get_mut().push(candidate);
                    }
                }
            }
            Err(_) => continue,
        }
    }
    sorted_variants.sort_by(|a, b| {
        normalise_string(a.to_lowercase()).cmp(&normalise_string(b.to_lowercase()))
    });
    (variant_map, sorted_variants)
}

pub fn get_lemmas() -> (HashMap<String, LemmaInfo>, Vec<String>) {
    let mut lemma_map = HashMap::new();
    let mut sorted_lemmas = Vec::new();
    let mut file = File::open("lemma_dict.json").expect("Couldn't open the lemma file.");
    let mut data = String::new();
    file.read_to_string(&mut data)
        .expect("Couldn't read from the lemma file.");
    let data_dict: HashMap<String, LemmaInfo> =
        serde_json::from_str(&data).expect("Malformed JSON");
    for (key, value) in data_dict.into_iter() {
        sorted_lemmas.push(String::from(&key));
        lemma_map.insert(String::from(key), value);
    }
    sorted_lemmas.sort_by(|a, b| {
        normalise_string(a.to_lowercase()).cmp(&normalise_string(b.to_lowercase()))
    });
    (lemma_map, sorted_lemmas)
}

pub fn normalise_string(input: String) -> String {
    let g = input.graphemes(true).collect::<Vec<&str>>();
    let mut result = String::from("");
    for glyph in g {
        match glyph {
            "(" | ")" => continue,
            "á" => result.push_str("a"),
            "ó" => result.push_str("o"),
            "ú" => result.push_str("u"),
            "í" => result.push_str("i"),
            "é" => result.push_str("e"),
            _ => result.push_str(glyph),
        }
    }
    return result;
}
