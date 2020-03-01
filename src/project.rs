use std::io::{SeekFrom, Seek, Read};
use crate::project::Version::Gm800;
use std::io;
use crate::gmstream::GmStream;

#[derive(Debug)]
pub enum Version {
    Unknown = 0,
    Gm800 = 800,
}

pub struct Project {
    pub version: Version
}

impl Project {
    fn detect_gm800<T: Read + Seek>(stream: &mut T) -> io::Result<bool> {
        stream.seek(SeekFrom::Start(2000000))?;

        Ok(stream.read_u32()? == 1234321)
    }

    fn parse_gm800<T: Read + Seek>(&mut self, mut stream: T) -> io::Result<()> {
        Project::detect_gm800(&mut stream)?;

        println!("Reading header...");
        let _version = stream.read_u32()?;
        let _debug = stream.read_u32()?;
        assert_eq!(_version, 800);

        println!("Reading settings...");
        let _version = stream.read_u32()?;
        assert_eq!(_version, 800);
        let mut compressed = stream.read_compressed()?;
        loop {
            println!("{}", compressed.read_u32()?);
        }


        Ok(())
    }

    pub fn parse<T: Read + Seek>(mut stream: T) -> io::Result<Project> {
        let mut project = Project {
            version: Version::Unknown,
        };

        if Project::detect_gm800(&mut stream)? {
            println!("Detected GM 8.0 Exe");
            project.version = Gm800;
            project.parse_gm800(&mut stream)?;
        } else {
            println!("Unknown file");
        }

        Ok(project)
    }
}