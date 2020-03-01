use std::io::SeekFrom;
use crate::project::Version::Gm800;
use std::io;

#[derive(Debug)]
pub enum Version {
    Unknown = 0,
    Gm800 = 800,
}

pub struct Project {
    pub version: Version
}

impl Project {
    fn detect_gm800(stream: &mut Box<dyn super::GmStream>) -> io::Result<bool> {
        stream.seek(SeekFrom::Start(2000000))?;
        Ok(stream.read_u32()? == 1234321)
    }

    fn parse_gm800(&mut self, stream: &mut Box<dyn super::GmStream>) -> io::Result<()> {
        Project::detect_gm800(stream)?;

        Ok(())
    }

    pub fn parse(mut stream: Box<dyn super::GmStream>) -> io::Result<Project> {
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