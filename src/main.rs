use std::fs::File;
use std::io::BufReader;
use std::{env, process};

struct Config {
    input: String,
    output: Option<String>,
}

impl Config {
    fn new(args: &[String]) -> Result<Config, &'static str> {
        if args.len() < 2 {
            return Err("Not enough arguments");
        }

        Ok(Config {
            input: args[1].clone(),
            output: args.get(2).cloned(),
        })
    }
}

fn main() -> Result<(), std::io::Error> {
    let args: Vec<String> = env::args().collect();

    let config = Config::new(&args).unwrap_or_else(|err| {
        eprintln!("Problem parsing arguments: {}", err);
        process::exit(1);
    });

    println!("Reading {}", config.input);
    let file = File::open(config.input)?;
    let file = BufReader::new(file);

    let project = gm_reader::decode(file)?;
    println!("Read game with version {:?}", project.version);

    if let Some(output) = config.output {
        println!("Writing MessagePack to {}.", output);
        let mut f = std::fs::File::create(output)?;
        rmp_serde::encode::write(&mut f, &project).unwrap();
        println!("Done.");
    }

    Ok(())
}
