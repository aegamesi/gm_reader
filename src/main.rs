use std::{env, process};
use std::fs::File;
use std::io::BufReader;

mod project;
mod gmstream;

use gmstream::GmStream;

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

    let wrapped = Box::new(file);
    let project = project::Project::parse(wrapped)?;

    println!("Version: {:?}", project.version);

    Ok(())
}
