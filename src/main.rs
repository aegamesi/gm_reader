use std::fs::File;
use std::io::BufReader;
use std::{env, process};

mod decoder;
mod game;

struct Config {
    filename: String,
}

impl Config {
    fn new(args: &[String]) -> Result<Config, &'static str> {
        if args.len() < 2 {
            return Err("Not enough arguments");
        }

        Ok(Config {
            filename: args[1].clone(),
        })
    }
}

fn main() -> Result<(), std::io::Error> {
    let args: Vec<String> = env::args().collect();

    let config = Config::new(&args).unwrap_or_else(|err| {
        eprintln!("Problem parsing arguments: {}", err);
        process::exit(1);
    });

    println!("Reading {}", config.filename);
    let file = File::open(config.filename)?;
    let file = BufReader::new(file);
    let project = decoder::decode(file)?;

    println!("Version: {:?}", project.version);

    Ok(())
}
