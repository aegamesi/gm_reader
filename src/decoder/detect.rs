use std::io::{Read, Seek, SeekFrom, Result};
use crate::game::Version;
use super::gmstream::GmStream;
use super::decrypt;

pub struct GameData {
    pub data: Vec<u8>,
    pub version: Version,
}

fn read_rest<T: Read + Seek>(stream: &mut T) -> Result<Vec<u8>> {
    let mut data = vec![];
    stream.read_to_end(&mut data)?;
    Ok(data)
}

fn detect_gm530<T: Read + Seek>(mut stream: &mut T) -> Result<Option<Vec<u8>>> {
    stream.seek(SeekFrom::Start(1500000))?;
    let magic = stream.next_u32()?;
    if magic == 1230500 {
        let mut stream = decrypt::decrypt_gm530(&mut stream)?;

        let _a = stream.next_u32()?;
        stream.skip_blob()?;

        let magic = stream.next_u32()?;
        let version = stream.next_u32()?;
        if magic == 1234321 && version == 530 {
            return Ok(Some(read_rest(&mut stream)?));
        }
    }

    Ok(None)
}

fn detect_gm600<T: Read + Seek>(stream: &mut T) -> Result<Option<Vec<u8>>> {
    let offsets = [
        (0, -1), // Raw game data
        (700000, 457832), // GM 6.0
        (800000, 486668), // GM 6.1
        (1420000, 1296488), // GM 6.0 with Vista support
        (1600000, 1393932), // GM 6.1 with Vista support
    ];

    for (data_offset, _icon_offset) in &offsets {
        stream.seek(SeekFrom::Start(*data_offset))?;
        let magic = stream.next_u32()?;
        let version = stream.next_u32()?;
        if magic == 1234321 && version == 600 {
            return Ok(Some(read_rest(stream)?));
        }
    }
    Ok(None)
}

fn detect_gm700<T: Read + Seek>(stream: &mut T) -> Result<Option<Vec<u8>>> {
    stream.seek(SeekFrom::Start(1980000))?;
    let magic = stream.next_u32()?;
    let version = stream.next_u32()?;
    if magic == 1234321 && version == 700 {
        return Ok(Some(read_rest(stream)?));
    }
    Ok(None)
}

fn detect_gm800<T: Read + Seek>(stream: &mut T) -> Result<Option<Vec<u8>>> {
    stream.seek(SeekFrom::Start(2000000))?;
    let magic = stream.next_u32()?;
    let version = stream.next_u32()?;
    if magic == 1234321 && version == 800 {
        return Ok(Some(read_rest(stream)?));
    }
    Ok(None)
}

fn detect_gm810<T: Read + Seek>(mut stream: &mut T) -> Result<Option<Vec<u8>>> {
    stream.seek(SeekFrom::Start(0x0039FBC4))?;

    for _ in 0..1024 {
        if stream.next_u32()? & 0xFF00FF00 == 0xF7000000 {
            if stream.next_u32()? & 0x00FF00FF == 0x00140067 {
                let mut stream = decrypt::decrypt_gm810(&mut stream)?;
                let magic = stream.next_u32()?;
                let version = stream.next_u32()?;

                if magic == 0 && version == 0 {
                    return Ok(Some(read_rest(&mut stream)?));
                } else {
                    return Ok(None)
                }
            }
        }
    }

    Ok(None)
}

pub fn decode<T: Read + Seek>(mut stream: T) -> Option<GameData> {
    let detectors: Vec<(Version, fn(&mut T) -> Result<Option<Vec<u8>>>)> = vec![
        (Version::Gm530, detect_gm530),
        (Version::Gm600, detect_gm600),
        (Version::Gm700, detect_gm700),
        (Version::Gm800, detect_gm800),
        (Version::Gm810, detect_gm810),
    ];
    detectors.iter().find_map(|(version, detector)| {
        match detector(&mut stream) {
            Ok(Some(data)) => {
                Some(GameData {
                    data,
                    version: *version,
                })
            }
            _ => None
        }
    })
}