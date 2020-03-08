extern crate crc;
extern crate byteorder;

use std::io;
use std::io::{SeekFrom, Seek, Read, Cursor};
use crate::gmstream::GmStream;
use byteorder::ByteOrder;

#[derive(Debug)]
pub enum Version {
    Unknown = 0,
    Gm600 = 600,
    Gm800 = 800,
    Gm810 = 810,
}

pub struct Project {
    pub version: Version
}

fn drain<T: Read>(mut s: T) -> io::Result<u64> {
    io::copy(&mut s, &mut io::sink())
}

fn decrypt_gm8xx<T: Read>(mut stream: T) -> io::Result<Cursor<Vec<u8>>> {
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

fn decrypt_gm810<T: Read + Seek>(stream: &mut T) -> io::Result<Cursor<Vec<u8>>> {
    // Generate seeds
    let key = format!("_MJD{}#RWK", stream.read_u32()?);
    let mut key_buffer = Vec::new();
    for b in key.bytes() {
        key_buffer.push(b);
        key_buffer.push(0);
    }
    let mut seed2: u32 = crc::crc32::checksum_ieee(&key_buffer) ^ 0xFFFFFFFF;
    let mut seed1: u32 = stream.read_u32()?;

    // Read version
    let mut version = stream.read_u32()?;
    let mut pos = stream.seek(SeekFrom::Current(0))? as u32;
    pos -= 4 + 0x0039FBC4 + 0x11;
    if pos == 0 {
        pos += 3;
    }
    pos = ((pos as i32) >> 2) as u32;
    version = version ^ pos;
    assert_eq!(version, 810);

    // Decrypt.
    let mut buf: Vec<u8> = Vec::new();
    stream.read_to_end(&mut buf)?;

    let mut pos = ((seed2 & 0xFF) + 6) as usize;
    println!("pos: {}", pos);
    while pos <= buf.len() - 4 {
        let chunk = &mut buf[pos..(pos + 4)];
        pos += 4;

        let input = byteorder::LittleEndian::read_u32(chunk);
        seed1 = (0xFFFF & seed1) * 0x9069 + (seed1 >> 16);
        seed2 = (0xFFFF & seed2) * 0x4650 + (seed2 >> 16);
        let mask = (seed1 << 16) + (seed2 & 0xFFFF);
        let output = input ^ mask;
        byteorder::LittleEndian::write_u32(chunk, output);
    }

    Ok(Cursor::new(buf))
}

impl Project {
    fn detect_gm800<T: Read + Seek>(stream: &mut T) -> io::Result<bool> {
        stream.seek(SeekFrom::Start(2000000))?;

        Ok(stream.read_u32()? == 1234321)
    }

    fn detect_gm810<T: Read + Seek>(stream: &mut T) -> io::Result<bool> {
        stream.seek(SeekFrom::Start(0x0039FBC4))?;

        for _ in 0..1024 {
            if stream.read_u32()? & 0xFF00FF00 == 0xF7000000 {
                if stream.read_u32()? & 0x00FF00FF == 0x00140067 {
                    return Ok(true)
                }
            }
        }

        Ok(false)
    }

    fn detect_gm6xx<T: Read + Seek>(stream: &mut T) -> io::Result<bool> {
        let start_offsets = [0, 700000, 800000, 1420000, 1600000];
        let _icon_offsets = [-1, 457832, 486668, 1296488, 1393932];

        for offset in &start_offsets {
            stream.seek(SeekFrom::Start(*offset))?;
            if stream.read_u32()? == 1234321 && stream.read_u32()? == 600 {
                return Ok(true)
            }
        }
        Ok(false)
    }

    fn parse_gm8xx<T: Read + Seek>(&mut self, mut stream: T) -> io::Result<()> {
        println!("Reading header...");
        if let Version::Gm810 = self.version {
            stream.read_u32()?;
        }
        let _version = stream.read_u32()?;
        let _debug = stream.read_u32()?;

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
        let mut stream = decrypt_gm8xx(stream)?;

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

        println!("Reading backgrounds...");
        let _version = stream.read_u32()?;
        let num_backgrounds = stream.read_u32()?;
        for _ in 0..num_backgrounds {
            let mut section = stream.read_compressed()?;
            if section.read_bool()? {
                let name = section.read_string()?;
                println!("Background name: {}", name);
            }
            drain(section)?;
        }

        println!("Reading paths...");
        let _version = stream.read_u32()?;
        let num_paths = stream.read_u32()?;
        for _ in 0..num_paths {
            let mut section = stream.read_compressed()?;
            if section.read_bool()? {
                let name = section.read_string()?;
                println!("Path name: {}", name);
            }
            drain(section)?;
        }

        println!("Reading scripts...");
        let _version = stream.read_u32()?;
        let num_scripts = stream.read_u32()?;
        for _ in 0..num_scripts {
            let mut section = stream.read_compressed()?;
            if section.read_bool()? {
                let name = section.read_string()?;
                println!("Script name: {}", name);
            }
            drain(section)?;
        }

        println!("Reading fonts...");
        let _version = stream.read_u32()?;
        let num_fonts = stream.read_u32()?;
        for _ in 0..num_fonts {
            let mut section = stream.read_compressed()?;
            if section.read_bool()? {
                let name = section.read_string()?;
                let _version = section.read_u32()?;
                let font_name = section.read_string()?;
                println!("Font name: {} : {}", name, font_name);
                let size = section.read_u32()?;
                let bold = section.read_u32()?;
                let italic = section.read_u32()?;
                let mut range_start = section.read_u32()?;
                let range_end = section.read_u32()?;

                if let Version::Gm810 = self.version {
                    let _charset = range_start & 0xFF000000;
                    let _aa_level = range_start & 0x00FF0000;
                    range_start &= 0x0000FFFF;
                }

                println!("Size {}, Bold {}, Italic {}, Start {}, End {}", size, bold, italic, range_start, range_end);
            }
            drain(section)?;
        }

        println!("Reading timelines...");
        let _version = stream.read_u32()?;
        let num_timelines = stream.read_u32()?;
        for _ in 0..num_timelines {
            let mut section = stream.read_compressed()?;
            if section.read_bool()? {
                let name = section.read_string()?;
                println!("Timeline name: {}", name);
            }
            drain(section)?;
        }

        println!("Reading objects...");
        let _version = stream.read_u32()?;
        let num_objects = stream.read_u32()?;
        for _ in 0..num_objects {
            let mut section = stream.read_compressed()?;
            if section.read_bool()? {
                let name = section.read_string()?;
                println!("Object name: {}", name);
            }
            drain(section)?;
        }

        println!("Reading rooms...");
        let _version = stream.read_u32()?;
        let num_rooms = stream.read_u32()?;
        for _ in 0..num_rooms {
            let mut section = stream.read_compressed()?;
            if section.read_bool()? {
                let name = section.read_string()?;
                println!("Room name: {}", name);
            }
            drain(section)?;
        }

        let _last_object_id = stream.read_u32()?;
        let _last_tile_id = stream.read_u32()?;
        println!("Last object: {}, last tile: {}", _last_object_id, _last_tile_id);

        println!("Reading includes...");
        let _version = stream.read_u32()?;
        let num_includes = stream.read_u32()?;
        for _ in 0..num_includes {
            let mut section = stream.read_compressed()?;
            if section.read_bool()? {
                let name = section.read_string()?;
                println!("Include name: {}", name);
            }
            drain(section)?;
        }

        println!("Reading help...");
        let _version = stream.read_u32()?;
        stream.skip_section()?;

        println!("Reading library init code...");
        let _version = stream.read_u32()?;
        let num_inits = stream.read_u32()?;
        for _ in 0..num_inits {
            // println!("Library init: {}", stream.read_string()?);
            stream.skip_section()?;
        }

        println!("Reading room order...");
        let _version = stream.read_u32()?;
        let num_rooms = stream.read_u32()?;
        for _ in 0..num_rooms {
            let _order = stream.read_u32()?;
            // println!("room {}", _order);
        }

        let remaining = drain(stream)?;
        println!("Remaining bytes: {}", remaining);

        println!("Done");
        // println!("#### {}", stream.read_string()?);

        Ok(())
    }

    pub fn parse<T: Read + Seek>(mut stream: T) -> io::Result<Project> {
        let mut project = Project {
            version: Version::Unknown,
        };

        if Project::detect_gm6xx(&mut stream)? {
            println!("Detected GM 6.0/6.1 Exe");
            project.version = Version::Gm600;
        } else if Project::detect_gm800(&mut stream)? {
            println!("Detected GM 8.0 Exe");
            project.version = Version::Gm800;
            project.parse_gm8xx(&mut stream)?;
        } else if Project::detect_gm810(&mut stream)? {
            println!("Detected GM 8.1 Exe");
            project.version = Version::Gm810;
            let mut stream = decrypt_gm810(&mut stream)?;
            project.parse_gm8xx(&mut stream)?;
        } else {
            println!("Unknown file");
        }

        Ok(project)
    }
}