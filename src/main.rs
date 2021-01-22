#![feature(proc_macro_hygiene, decl_macro)]
#[macro_use]
extern crate rocket;

mod data_structures;

use data_structures::{get_lemmas, get_variants, normalise_string, Candidate, LemmaInfo};
use reqwest;
use rocket::config::{Config, Environment};
use rocket::http::{Header, RawStr};
use rocket::response::Responder;
use rocket::State;
use rocket_contrib::json::Json;
use std::collections::HashMap;
use std::sync::Arc;
use std::{env, process};

const DEFAULT_PORT: u16 = 9000;

fn main() {
    let config = CommandLineConfig::new(env::args()).unwrap_or_else(|err| {
        eprintln!("{}", err);
        process::exit(1);
    });
    if config.port != DEFAULT_PORT {
        println!("Using port {}", config.port)
    }

    let config = Config::build(Environment::Production)
        .address("127.0.0.1")
        .secret_key("B0YUNf5UvvgcERWkrX0fUHHykRFxeebDUoxA4cFXjGU=")
        .port(config.port)
        .finalize()
        .unwrap();

    // Initialise the datastructures to serve queries
    let (variant_map, sorted_variants) = get_variants();
    let (lemma_map, sorted_lemmas) = get_lemmas();
    let global_state = MyState {
        variant_mapping: Arc::new(variant_map),
        sorted_variants: Arc::new(sorted_variants),
        lemma_mapping: Arc::new(lemma_map),
        sorted_lemmas: Arc::new(sorted_lemmas),
    };

    rocket::custom(config)
        .manage(global_state)
        .mount("/", routes![index, serve_forms, serve_lemmas, dil])
        .launch();
}

struct MyState {
    variant_mapping: Arc<HashMap<String, Vec<Candidate>>>,
    sorted_variants: Arc<Vec<String>>,
    lemma_mapping: Arc<HashMap<String, LemmaInfo>>,
    sorted_lemmas: Arc<Vec<String>>,
}

// Command-line options

#[derive(Debug)]
struct CommandLineConfig {
    pub port: u16,
}

impl CommandLineConfig {
    pub fn new(mut args: env::Args) -> Result<CommandLineConfig, &'static str> {
        args.next();

        let port = match args.next() {
            Some(arg) => match arg.trim().parse::<u16>() {
                Ok(port_no) => port_no,
                Err(_) => return Err("The port number must be an integer."),
            },
            None => {
                println!("Using the default port {}", DEFAULT_PORT);
                DEFAULT_PORT
            }
        };
        Ok(CommandLineConfig { port })
    }
}

// Routes

#[get("/")]
fn index() -> &'static str {
    "This server only serves as an API endpoint. Use the /query/WORDFORM route."
}

#[derive(Responder)]
struct LemmaJsonWithOrigin {
    data: Json<Vec<(String, usize)>>,
    header: Header<'static>,
}

#[get("/query/lemmas/<query_string>?<numvars>")]
fn serve_lemmas(
    global_state: State<MyState>,
    query_string: &RawStr,
    numvars: Option<String>,
) -> Option<LemmaJsonWithOrigin> {
    let query_string = match query_string.url_decode() {
        Ok(query_string) => query_string,
        Err(_) => return None,
    };
    let num_variants = match numvars {
        Some(num) => num
            .parse::<usize>()
            .expect("numvars parameter should be an integer"),
        None => 20, // Default.
    };
    let mut result = Vec::new();
    let lemma_mapping = Arc::clone(&global_state.inner().lemma_mapping);
    let sorted_lemmas = Arc::clone(&global_state.inner().sorted_lemmas);
    let query_string = normalise_string(query_string.to_lowercase());
    let index = match sorted_lemmas
        .binary_search_by(|probe| normalise_string(probe.to_lowercase()).cmp(&query_string))
    {
        // We don't care if the element is present or not;
        // we only want to know where the appropriate key range starts.
        Ok(index) => index,
        Err(index) => index,
    };
    let mut var_count = 0;
    for key in &sorted_lemmas[index..] {
        if var_count == num_variants {
            break;
        }
        let key_norm = normalise_string(key.to_string().to_lowercase());
        if key_norm.starts_with(&query_string) {
            let lemma_info = &lemma_mapping.get(key).unwrap();
            result.push((lemma_info.headword.clone(), lemma_info.id));
            var_count += 1;
        } else {
            break;
        }
    }
    Some(LemmaJsonWithOrigin {
        data: Json(result),
        header: Header::new("Access-Control-Allow-Origin", "*"),
    })
}

#[derive(Responder)]
struct FormJsonWithOrigin {
    data: Json<Vec<(String, usize, String)>>,
    header: Header<'static>,
}

#[get("/query/wordforms/<wordform>?<numvars>")]
fn serve_forms(
    global_state: State<MyState>,
    wordform: &RawStr,
    numvars: Option<String>,
) -> Option<FormJsonWithOrigin> {
    let wordform = match wordform.url_decode() {
        Ok(wordform) => wordform,
        Err(_) => return None,
    };
    let num_variants = match numvars {
        Some(num) => num
            .parse::<usize>()
            .expect("numvars parameter should be an integer"),
        None => 20, // Default.
    };
    let mut result = Vec::new();
    let variant_mapping = Arc::clone(&global_state.inner().variant_mapping);
    let sorted_variants = Arc::clone(&global_state.inner().sorted_variants);
    let wordform = normalise_string(wordform.to_lowercase());
    let index = match sorted_variants
        .binary_search_by(|probe| normalise_string(probe.to_lowercase()).cmp(&wordform))
    {
        // We don't care if the element is present or not;
        // we only want to know where the appropriate key range starts.
        Ok(index) => index,
        Err(index) => index,
    };
    let mut var_count = 0;
    for key in &sorted_variants[index..] {
        let key_norm = normalise_string(key.to_string().to_lowercase());
        if key_norm.starts_with(&wordform) {
            let candidates = variant_mapping.get(key).unwrap();
            for candidate in candidates.iter().cloned() {
                if var_count == num_variants {
                    return Some(FormJsonWithOrigin {
                        data: Json(result),
                        header: Header::new("Access-Control-Allow-Origin", "*"),
                    });
                }
                result.push((key.clone(), candidate.entry_id, candidate.entry));
                var_count += 1;
            }
        } else {
            break;
        }
    }
    Some(FormJsonWithOrigin {
        data: Json(result),
        header: Header::new("Access-Control-Allow-Origin", "*"),
    })
}

#[derive(Responder)]
struct TextWithOrigin {
    data: String,
    header: Header<'static>,
}

#[get("/dil/<entryid>")]
fn dil(entryid: &RawStr) -> Option<TextWithOrigin> {
    let entryid = match entryid.url_decode() {
        Ok(entryid) => entryid
            .parse::<usize>()
            .expect("entryid parameter should be an integer"),
        Err(_) => return None,
    };
    let url = format!("http://dil.ie/browse/show?q={}", entryid);
    let body = reqwest::blocking::get(&url)
        .expect("Couldn't connect to the DIL server")
        .text()
        .unwrap();
    Some(TextWithOrigin {
        data: body,
        header: Header::new("Access-Control-Allow-Origin", "*"),
    })
}
