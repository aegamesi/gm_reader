extern crate crc;

mod gmstream;

use gmstream::GmStream;
use crate::game::{Game, Version, Sound, Sprite, SpriteFrame, SpriteMask, Background, Path, PathPoint, Script, Font, Action, Timeline, TimelineMoment, Object, ObjectEvent, Constant};
use std::io;
use std::io::{Cursor, Read, Seek, SeekFrom};

fn drain<T: Read>(mut s: T) -> io::Result<u64> {
    io::copy(&mut s, &mut io::sink())
}

fn assert_eof<T: Read>(s: T) {
    let bytes_remaining = drain(s).unwrap();
    assert_eq!(bytes_remaining, 0)
}

fn decrypt_gm8xx<T: Read>(mut stream: T) -> io::Result<Cursor<Vec<u8>>> {
    let mut forward_table: [u8; 256] = [0; 256];
    let mut reverse_table: [u8; 256] = [0; 256];

    // Read and construct tables.
    let d1 = stream.next_u32()?;
    let d2 = stream.next_u32()?;
    stream.skip(4 * d1)?;
    stream.read_exact(&mut forward_table)?;
    stream.skip(4 * d2)?;
    for i in 0..256 {
        reverse_table[forward_table[i] as usize] = i as u8;
    }

    // Read data into memory.
    let len = stream.next_u32()? as usize;
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
    let key = format!("_MJD{}#RWK", stream.next_u32()?);
    let mut key_buffer = Vec::new();
    for b in key.bytes() {
        key_buffer.push(b);
        key_buffer.push(0);
    }
    let mut seed2: u32 = crc::crc32::checksum_ieee(&key_buffer) ^ 0xFFFFFFFF;
    let mut seed1: u32 = stream.next_u32()?;

    // Read version
    let mut version = stream.next_u32()?;
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
        let mut input = [0u8; 4];
        input.copy_from_slice(chunk);
        pos += 4;

        let input = u32::from_le_bytes(input);
        seed1 = (0xFFFF & seed1) * 0x9069 + (seed1 >> 16);
        seed2 = (0xFFFF & seed2) * 0x4650 + (seed2 >> 16);
        let mask = (seed1 << 16) + (seed2 & 0xFFFF);
        let output: u32 = input ^ mask;
        chunk.copy_from_slice(&output.to_le_bytes());
    }

    Ok(Cursor::new(buf))
}

fn decrypt_gm530<T: Read + Seek>(stream: &mut T, key: u32) -> io::Result<Cursor<Vec<u8>>> {
    let mut arr0: [u32; 256] = [0; 256];
    let mut arr1: [u32; 256] = [0; 256];

    for i in 0..256 {
        arr0[i] = i as u32;
    }
    for i in 1..10001 {
        let j = ((i * key) % 254 + 1) as usize;
        let k = arr0[j];
        arr0[j] = arr0[j + 1];
        arr0[j + 1] = k;
    }
    for i in 1..256 {
        arr1[arr0[i] as usize] = i as u32;
    }

    // Decrypt.
    let mut buf: Vec<u8> = Vec::new();
    stream.read_to_end(&mut buf)?;

    for i in 0..buf.len() {
        buf[i] = arr1[buf[i] as usize] as u8;
    }

    Ok(Cursor::new(buf))
}

fn detect_gm800<T: Read + Seek>(stream: &mut T) -> io::Result<bool> {
    stream.seek(SeekFrom::Start(2000000))?;

    Ok(stream.next_u32()? == 1234321)
}

fn detect_gm810<T: Read + Seek>(stream: &mut T) -> io::Result<bool> {
    stream.seek(SeekFrom::Start(0x0039FBC4))?;

    for _ in 0..1024 {
        if stream.next_u32()? & 0xFF00FF00 == 0xF7000000 {
            if stream.next_u32()? & 0x00FF00FF == 0x00140067 {
                return Ok(true);
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
        if stream.next_u32()? == 1234321 && stream.next_u32()? == 600 {
            return Ok(true);
        }
    }
    Ok(false)
}

fn detect_gm530<T: Read + Seek>(stream: &mut T) -> io::Result<bool> {
    stream.seek(SeekFrom::Start(1500000))?;
    let magic = stream.next_u32()?;
    if magic != 1230500 {
        return Ok(false);
    }

    Ok(true)
}

fn detect_gm700<T: Read + Seek>(stream: &mut T) -> io::Result<bool> {
    stream.seek(SeekFrom::Start(1980000))?;

    Ok(stream.next_u32()? == 1234321)
}

fn read_action<T: Read>(stream: &mut T) -> io::Result<Action> {
    let mut action = Action::default();
    let _version = stream.next_u32()?;
    action.library_id = stream.next_u32()?;
    action.action_id = stream.next_u32()?;
    action.action_kind = stream.next_u32()?;
    action.has_relative = stream.next_bool()?;
    action.is_question = stream.next_bool()?;
    action.has_target = stream.next_bool()?;
    action.action_type = stream.next_u32()?;
    action.name = stream.next_string()?;
    action.code = stream.next_string()?;
    action.parameters_used = stream.next_u32()?;

    let num_parameters = stream.next_u32()?;
    action.parameters.reserve(num_parameters as usize);
    for _ in 0..num_parameters as usize {
        action.parameters.push(stream.next_u32()?);
    }

    action.target = stream.next_i32()?;
    action.relative = stream.next_bool()?;

    let num_arguments = stream.next_u32()?;
    action.arguments.reserve(num_arguments as usize);
    for _ in 0..num_arguments as usize {
        action.arguments.push(stream.next_string()?);
    }

    action.negate = stream.next_bool()?;

    Ok(action)
}

fn read_actions<T: Read>(stream: &mut T) -> io::Result<Vec<Action>> {
    let mut actions = Vec::new();
    let _version = stream.next_u32()?;
    let num_actions = stream.next_u32()?;
    actions.reserve(num_actions as usize);
    for _ in 0..num_actions {
        actions.push(read_action(stream)?);
    }
    Ok(actions)
}

fn parse_exe<T: Read + Seek>(game: &mut Game, mut stream: T) -> io::Result<()> {
    println!("Reading header...");
    if let Version::Gm810 = game.version {
        stream.next_u32()?;
    }
    let _version = stream.next_u32()?;
    game.debug = stream.next_bool()?;

    println!("Reading settings...");
    let _version = stream.next_u32()?;
    assert_eq!(_version, 800);
    let compressed = stream.next_compressed()?;
    drain(compressed)?;

    // Skip d3dx8.dll (name and then content).
    let len = stream.next_u32()?;
    stream.skip(len)?;
    let len = stream.next_u32()?;
    stream.skip(len)?;

    // Do the main "decryption".
    println!("Decrypting inner...");
    let mut stream = decrypt_gm8xx(stream)?;

    // Skip junk
    let len = stream.next_u32()?;
    stream.skip(len * 4)?;

    game.pro = stream.next_bool()?;
    game.game_id = stream.next_u32()?;
    for i in 0..4 {
        game.guid[i] = stream.next_u32()?;
    }

    println!("Reading extensions...");
    let _version = stream.next_u32()?;
    let num_extensions = stream.next_u32()?;
    for _ in 0..num_extensions {
        stream.skip(4)?;
        println!("Extension Name: {}", stream.next_string()?);
        stream.skip_section()?;

        let count = stream.next_u32()?;
        for _ in 0..count {
            stream.skip(4)?;
            stream.skip_section()?;
            stream.skip(4)?;
            stream.skip_section()?;
            stream.skip_section()?;

            // Args?
            let count = stream.next_u32()?;
            for _ in 0..count {
                stream.skip(4)?;
                stream.skip_section()?;
                stream.skip_section()?;
                stream.skip(4 * 3)?;

                stream.skip(4 * 18)?;
            }

            // Constants
            let count = stream.next_u32()?;
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
    let _version = stream.next_u32()?;
    let num_triggers = stream.next_u32()?;
    for _ in 0..num_triggers {
        stream.skip_section()?;
        // TODO read triggers
    }

    println!("Reading constants...");
    let _version = stream.next_u32()?;
    let num_constants = stream.next_u32()?;
    game.constants.reserve(num_constants as usize);
    for _ in 0..num_constants {
        let mut constant = Constant::default();
        constant.name = stream.next_string()?;
        constant.value = stream.next_string()?;
        game.constants.push(constant);
    }

    println!("Reading sounds...");
    let _version = stream.next_u32()?;
    let num_sounds = stream.next_u32()?;
    game.sounds.reserve(num_sounds as usize);
    for i in 0..num_sounds {
        let mut stream = stream.next_compressed()?;
        if !stream.next_bool()? {
            continue;
        }

        let mut sound = Sound::default();
        sound.id = i;
        sound.name = stream.next_string()?;
        let _version = stream.next_u32()?;
        sound.kind = stream.next_u32()?;
        sound.filetype = stream.next_string()?;
        sound.filename = stream.next_string()?;
        if stream.next_bool()? {
            sound.data = stream.next_section()?;
        }
        sound.effects = stream.next_u32()?;
        sound.volume = stream.next_f64()?;
        sound.pan = stream.next_f64()?;
        sound.preload = stream.next_bool()?;
        game.sounds.push(sound);
        assert_eof(stream);
    }

    println!("Reading sprites...");
    let _version = stream.next_u32()?;
    let num_sprites = stream.next_u32()?;
    game.sprites.reserve(num_sprites as usize);
    for i in 0..num_sprites {
        let mut stream = stream.next_compressed()?;
        if !stream.next_bool()? {
            continue;
        }

        let mut sprite = Sprite::default();
        sprite.id = i;
        sprite.name = stream.next_string()?;
        let _version = stream.next_u32()?;
        sprite.origin = (stream.next_i32()?, stream.next_i32()?);

        let num_frames = stream.next_u32()? as usize;
        if num_frames == 0 {
            continue;
        }

        sprite.frames.reserve(num_frames);
        for _ in 0..num_frames {
            let mut frame = SpriteFrame::default();
            let _version = stream.next_u32()?;
            frame.size = (stream.next_u32()?, stream.next_u32()?);
            frame.data = stream.next_section()?;
            sprite.frames.push(frame);
        }

        let has_separate_masks = stream.next_bool()?;
        let num_masks = if has_separate_masks { num_frames } else { 1 };
        sprite.masks.reserve(num_masks);
        for _ in 0..num_masks {
            let mut mask = SpriteMask::default();
            let _version = stream.next_u32()?;
            mask.size = (stream.next_u32()?, stream.next_u32()?);
            mask.left = stream.next_i32()?;
            mask.right = stream.next_i32()?;
            mask.bottom = stream.next_i32()?;
            mask.top = stream.next_i32()?;
            let data_length = (mask.size.0 * mask.size.1) as usize;
            mask.data.reserve(data_length);
            for _ in 0..data_length {
                mask.data.push(stream.next_bool()?);
            }
            sprite.masks.push(mask);
        }

        game.sprites.push(sprite);
        assert_eof(stream);
    }

    println!("Reading backgrounds...");
    let _version = stream.next_u32()?;
    let num_backgrounds = stream.next_u32()?;
    game.backgrounds.reserve(num_backgrounds as usize);
    for i in 0..num_backgrounds {
        let mut stream = stream.next_compressed()?;
        if !stream.next_bool()? {
            continue;
        }

        let mut background = Background::default();
        background.id = i;
        background.name = stream.next_string()?;
        let _version = stream.next_u32()?;
        let _version = stream.next_u32()?;
        background.size = (stream.next_u32()?, stream.next_u32()?);
        if background.size.0 > 0 && background.size.1 > 0 {
            background.data = stream.next_section()?;
        }
        game.backgrounds.push(background);
        assert_eof(stream);
    }

    println!("Reading paths...");
    let _version = stream.next_u32()?;
    let num_paths = stream.next_u32()?;
    game.paths.reserve(num_paths as usize);
    for i in 0..num_paths {
        let mut stream = stream.next_compressed()?;
        if !stream.next_bool()? {
            continue;
        }

        let mut path = Path::default();
        path.id = i;
        path.name = stream.next_string()?;
        let _version = stream.next_u32()?;
        path.connection_type = stream.next_u32()?;
        path.closed = stream.next_bool()?;
        path.precision = stream.next_u32()?;
        let num_points = stream.next_u32()? as usize;
        path.points.reserve(num_points);
        for _ in 0..num_points {
            let mut point = PathPoint::default();
            point.x = stream.next_f64()?;
            point.y = stream.next_f64()?;
            point.speed = stream.next_f64()?;
            path.points.push(point);
        }
        game.paths.push(path);
        assert_eof(stream);
    }

    println!("Reading scripts...");
    let _version = stream.next_u32()?;
    let num_scripts = stream.next_u32()?;
    game.scripts.reserve(num_scripts as usize);
    for i in 0..num_scripts {
        let mut stream = stream.next_compressed()?;
        if !stream.next_bool()? {
            continue;
        }

        let mut script = Script::default();
        script.id = i;
        script.name = stream.next_string()?;
        let _version = stream.next_u32()?;
        script.script = stream.next_string()?;
        game.scripts.push(script);
        assert_eof(stream);
    }

    println!("Reading fonts...");
    let _version = stream.next_u32()?;
    let num_fonts = stream.next_u32()?;
    game.fonts.reserve(num_fonts as usize);
    for i in 0..num_fonts {
        let mut stream = stream.next_compressed()?;
        if !stream.next_bool()? {
            continue;
        }

        let mut font = Font::default();
        font.id = i;
        font.name = stream.next_string()?;
        let _version = stream.next_u32()?;
        font.font_name = stream.next_string()?;
        font.size = stream.next_u32()?;
        font.bold = stream.next_bool()?;
        font.italic = stream.next_bool()?;
        font.range_start = stream.next_u32()?;
        font.range_end = stream.next_u32()?;

        if let Version::Gm810 = game.version {
            font.charset = (font.range_start & 0xFF000000) >> 24;
            font.aa_level = (font.range_start & 0x00FF0000) >> 16;
            font.range_start = font.range_start & 0x0000FFFF;
        }

        for i in 0..256 {
            let glyph = &mut font.atlas.glyphs[i];
            glyph.pos = (stream.next_u32()?, stream.next_u32()?);
            glyph.size = (stream.next_u32()?, stream.next_u32()?);
            glyph.horizontal_advance = stream.next_i32()?;
            glyph.kerning = stream.next_i32()?;
        }
        font.atlas.size = (stream.next_u32()?, stream.next_u32()?);
        font.atlas.data = stream.next_section()?;

        game.fonts.push(font);
        assert_eof(stream);
    }

    println!("Reading timelines...");
    let _version = stream.next_u32()?;
    let num_timelines = stream.next_u32()?;
    game.timelines.reserve(num_timelines as usize);
    for i in 0..num_timelines {
        let mut stream = stream.next_compressed()?;
        if !stream.next_bool()? {
            continue;
        }

        let mut timeline = Timeline::default();
        timeline.id = i;
        timeline.name = stream.next_string()?;
        let _version = stream.next_u32()?;
        let num_moments = stream.next_u32()?;
        timeline.moments.reserve(num_moments as usize);
        for _ in 0..num_moments {
            let mut moment = TimelineMoment::default();
            moment.position = stream.next_u32()?;
            moment.actions = read_actions(&mut stream)?;
            timeline.moments.push(moment);
        }
        game.timelines.push(timeline);
        assert_eof(stream);
    }

    println!("Reading objects...");
    let _version = stream.next_u32()?;
    let num_objects = stream.next_u32()?;
    game.objects.reserve(num_objects as usize);
    for i in 0..num_objects {
        let mut stream = stream.next_compressed()?;
        if !stream.next_bool()? {
            continue;
        }

        let mut object = Object::default();
        object.id = i;
        object.name = stream.next_string()?;
        let _version = stream.next_u32()?;
        object.sprite = stream.next_i32()?;
        object.solid = stream.next_bool()?;
        object.visible = stream.next_bool()?;
        object.depth = stream.next_i32()?;
        object.persistent = stream.next_bool()?;
        object.parent = stream.next_i32()?;
        object.mask = stream.next_i32()?;

        let num_events = stream.next_u32()? + 1;
        for event_type in 0..num_events {
            loop {
                let event_number = stream.next_i32()?;
                if event_number == -1 {
                    break;
                }

                let mut event = ObjectEvent::default();
                event.event_type = event_type;
                event.event_number = event_number;
                event.actions = read_actions(&mut stream)?;
                object.events.push(event);
            }
        }

        game.objects.push(object);
        assert_eof(stream);
    }

    println!("Reading rooms...");
    let _version = stream.next_u32()?;
    let num_rooms = stream.next_u32()?;
    for _ in 0..num_rooms {
        let mut section = stream.next_compressed()?;
        if section.next_bool()? {
            let name = section.next_string()?;
            println!("Room name: {}", name);
        }
        drain(section)?;
    }

    let _last_object_id = stream.next_u32()?;
    let _last_tile_id = stream.next_u32()?;
    println!(
        "Last object: {}, last tile: {}",
        _last_object_id, _last_tile_id
    );

    println!("Reading includes...");
    let _version = stream.next_u32()?;
    let num_includes = stream.next_u32()?;
    for _ in 0..num_includes {
        let mut section = stream.next_compressed()?;
        if section.next_bool()? {
            let name = section.next_string()?;
            println!("Include name: {}", name);
        }
        drain(section)?;
    }

    println!("Reading help...");
    let _version = stream.next_u32()?;
    stream.skip_section()?;

    println!("Reading library init code...");
    let _version = stream.next_u32()?;
    let num_inits = stream.next_u32()?;
    for _ in 0..num_inits {
        // println!("Library init: {}", stream.read_string()?);
        stream.skip_section()?;
    }

    println!("Reading room order...");
    let _version = stream.next_u32()?;
    let num_rooms = stream.next_u32()?;
    for _ in 0..num_rooms {
        let _order = stream.next_u32()?;
        // println!("room {}", _order);
    }

    let remaining = drain(stream)?;
    println!("Remaining bytes: {}", remaining);

    println!("Done");
    // println!("#### {}", stream.read_string()?);

    Ok(())
}

pub fn decode<T: Read + Seek>(mut stream: T) -> io::Result<Game> {
    let mut project = Game::default();
    project.version = Version::Unknown;

    if detect_gm530(&mut stream)? {
        println!("Detected GM 5.3A Exe");
        project.version = Version::Gm530;

        let key = stream.next_u32()?;
        let mut stream = decrypt_gm530(&mut stream, key)?;

        let _ = stream.next_u32()?;
        stream.skip_section()?;

        // At this point, stream contains a V 5.3a GMD.
    } else if detect_gm6xx(&mut stream)? {
        println!("Detected GM 6.0/6.1 Exe");
        project.version = Version::Gm600;
    } else if detect_gm700(&mut stream)? {
        println!("Detected GM 7.0 Exe");
        project.version = Version::Gm700;
    } else if detect_gm800(&mut stream)? {
        println!("Detected GM 8.0 Exe");
        project.version = Version::Gm800;
        parse_exe(&mut project, &mut stream)?;
    } else if detect_gm810(&mut stream)? {
        println!("Detected GM 8.1 Exe");
        project.version = Version::Gm810;
        let mut stream = decrypt_gm810(&mut stream)?;
        parse_exe(&mut project, &mut stream)?;
    } else {
        println!("Unknown file");
    }

    Ok(project)
}
