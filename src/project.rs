use std::io::{SeekFrom, Seek, Read, Take, Cursor};
use crate::project::Version::Gm800;
use std::io;
use crate::gmstream::GmStream;
use flate2::read::ZlibDecoder;

#[derive(Debug)]
pub enum Version {
    Unknown = 0,
    Gm800 = 800,
}

pub struct Project {
    pub version: Version
}

fn drain<T: Read>(s: ZlibDecoder<Take<&mut T>>) -> io::Result<()> {
    match io::copy(&mut s.into_inner(), &mut io::sink()) {
        Err(e) => Err(e),
        Ok(_) => Ok(())
    }
}

fn decrypt_gm800<T: Read>(mut stream: T) -> io::Result<Cursor<Vec<u8>>> {
    let mut forward_table: [u8; 256] = [0; 256];
    let mut reverse_table: [u8; 256] = [0; 256];

    // Read and construct tables.
    let d1 = stream.read_u32()?;
    let d2 = stream.read_u32()?;
    stream.skip(4 * d1)?;
    stream.read_exact(&mut forward_table)?;
    stream.skip(4 * d2)?;
    for i in 0..256 {
        reverse_table[forward_table[i] as usize] = i as u8;
    }

    // Read data into memory.
    let len = stream.read_u32()? as usize;
    let mut buf = Vec::with_capacity(len);
    stream.take(len as u64).read_to_end(&mut buf)?;

    // Phase 1.
    for i in (1..len).rev() {
        let a = reverse_table[buf[i] as usize] as u64;
        let b = buf[i - 1] as u64;
        let c = a.wrapping_sub(b).wrapping_sub(i as u64);
        buf[i] = c as u8;
    }

    // Phase 2.
    for i in (0..len).rev() {
        let a = forward_table[i & 0xFF] as i64;
        let mut b = (i as i64) - a;
        if b < 0 {
            b = 0;
        }

        let a = buf[i];
        buf[i] = buf[b as usize];
        buf[b as usize] = a;
    }

    Ok(Cursor::new(buf))
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
        let compressed = stream.read_compressed()?;
        drain(compressed)?;

        // Skip d3dx8.dll (name and then content).
        let len = stream.read_u32()?;
        stream.skip(len)?;
        let len = stream.read_u32()?;
        stream.skip(len)?;

        // Do the main "decryption".
        println!("Decrypting inner...");
        let mut stream = decrypt_gm800(stream)?;

        // Skip junk
        let len = stream.read_u32()?;
        stream.skip(len * 4)?;

        let _pro = stream.read_bool()?;
        let _game_id = stream.read_u32()?;
        stream.skip(16)?;

        println!("Reading extensions...");
        let _version = stream.read_u32()?;
        let num_extensions = stream.read_u32()?;
        for _ in 0..num_extensions {
            stream.skip(4)?;
            println!("Extension Name: {}", stream.read_string()?);
            stream.skip_section()?;

            let count = stream.read_u32()?;
            for _ in 0..count {
                stream.skip(4)?;
                stream.skip_section()?;
                stream.skip(4)?;
                stream.skip_section()?;
                stream.skip_section()?;

                // Args?
                let count = stream.read_u32()?;
                for _ in 0..count {
                    stream.skip(4)?;
                    stream.skip_section()?;
                    stream.skip_section()?;
                    stream.skip(4 * 3)?;

                    stream.skip(4 * 18)?;
                }

                // Constants
                let count = stream.read_u32()?;
                for _ in 0..count {
                    stream.skip(4)?;
                    stream.skip_section()?;
                    stream.skip_section()?;
                }
            }

            // read resources files?
            stream.skip_section()?;
        }

        println!("Reading triggers...");
        let _version = stream.read_u32()?;
        let num_triggers = stream.read_u32()?;
        for _ in 0..num_triggers {
            stream.skip_section()?;
            // TODO read triggers
        }

        println!("Reading constants...");
        let _version = stream.read_u32()?;
        let num_constants = stream.read_u32()?;
        for _ in 0..num_constants {
            let name = stream.read_string()?;
            let value = stream.read_string()?;
            println!("Constant: {}: {}", name, value);
        }

        println!("Reading sounds...");
        let _version = stream.read_u32()?;
        let num_sounds = stream.read_u32()?;
        for _ in 0..num_sounds {
            stream.skip_section()?;
        }

        println!("Reading sprites...");
        let _version = stream.read_u32()?;
        let num_sprites = stream.read_u32()?;
        for _ in 0..num_sprites {
            let mut section = stream.read_compressed()?;
            if section.read_bool()? {
                let name = section.read_string()?;
                println!("Sprite name: {}", name);
            }
            drain(section)?;
        }

        println!("Done");
        // println!("#### {}", stream.read_string()?);

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