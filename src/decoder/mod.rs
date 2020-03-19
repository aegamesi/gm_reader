mod gmstream;
mod decrypt;
mod detect;

use gmstream::{decode_string, GmStream};
use crate::game::*;
use std::io;
use std::io::{Read, Seek, Cursor};

use image::{ConvertBuffer, RgbaImage, Pixel};

type BufferStream = Cursor<Vec<u8>>;
type BgraImage = image::ImageBuffer<image::Bgra<u8>, Vec<u8>>;

fn drain<T: Read>(mut s: T) -> io::Result<u64> {
    io::copy(&mut s, &mut io::sink())
}

fn assert_eof<T: Read>(s: T) {
    let bytes_remaining = drain(s).unwrap();
    assert_eq!(bytes_remaining, 0)
}

enum SectionWrapper<'a> {
    Owned(BufferStream),
    Borrowed(&'a mut BufferStream),
}

impl<'a> SectionWrapper<'a> {
    fn new(stream: &'a mut BufferStream, compressed: bool) -> io::Result<Self> {
        if compressed {
            Ok(SectionWrapper::Owned(stream.next_compressed()?))
        } else {
            Ok(SectionWrapper::Borrowed(stream))
        }
    }
}

impl Read for SectionWrapper<'_> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self {
            SectionWrapper::Owned(stream) => stream.read(buf),
            SectionWrapper::Borrowed(stream) => stream.read(buf),
        }
    }
}

impl Drop for SectionWrapper<'_> {
    fn drop(&mut self) {
        if let SectionWrapper::Owned(stream) = self {
            assert_eof(stream);
        }
    }
}

fn read_actions(stream: &mut SectionWrapper) -> io::Result<Vec<Action>> {
    let mut actions = Vec::new();
    let version = stream.next_u32()?;
    if version == 400 {
        let num_actions = stream.next_u32()?;
        actions.reserve(num_actions as usize);
        for _ in 0..num_actions {
            let mut action = Action::default();
            let version = stream.next_u32()?;
            if version == 440 {
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
            } else {
                unimplemented!();
            }
            actions.push(action);
        }
    } else {
        unimplemented!();
    }
    Ok(actions)
}

fn read_settings(game: &mut Game, stream: &mut BufferStream) -> io::Result<()> {
    println!("Reading settings...");
    let version = stream.next_u32()?;
    let mut stream = SectionWrapper::new(stream, version >= 800)?;

    game.settings.fullscreen = stream.next_bool()?;
    if version >= 600 {
        game.settings.interpolation = stream.next_bool()?;
    }
    game.settings.hide_border = stream.next_bool()?;
    game.settings.show_cursor = stream.next_bool()?;
    if version >= 542 {
        game.settings.scaling = stream.next_i32()?;
        game.settings.resizable = stream.next_bool()?;
        game.settings.always_on_top = stream.next_bool()?;
        game.settings.background_color = stream.next_u32()?;
    }

    game.settings.set_resolution = stream.next_bool()?;
    if version >= 542 {
        game.settings.color_depth = stream.next_u32()?;
        game.settings.resolution = stream.next_u32()?;
        game.settings.frequency = stream.next_u32()?;
    }
    game.settings.hide_buttons = stream.next_bool()?;
    if version >= 542 {
        game.settings.vsync = stream.next_bool()?;
    }
    if version >= 800 {
        game.settings.disable_screensaver = stream.next_bool()?;
    }

    game.settings.default_f4 = stream.next_bool()?;
    game.settings.default_f1 = stream.next_bool()?;
    game.settings.default_esc = stream.next_bool()?;
    game.settings.default_f5 = stream.next_bool()?;
    if version >= 702 {
        game.settings.default_f9 = stream.next_bool()?;
        game.settings.close_as_esc = stream.next_bool()?;
    }
    game.settings.priority = stream.next_u32()?;
    game.settings.freeze = stream.next_bool()?;

    game.settings.loading_bar = stream.next_u32()?;
    if game.settings.loading_bar > 0 {
        if stream.next_bool()? {
            game.settings.loading_bar_back = if game.version >= Version::Gm800 {
                Some(stream.next_blob()?)
            } else {
                Some(stream.next_compressed()?.into_inner())
            };
        }
        if stream.next_bool()? {
            game.settings.loading_bar_front = if game.version >= Version::Gm800 {
                Some(stream.next_blob()?)
            } else {
                Some(stream.next_compressed()?.into_inner())
            };
        }
    }

    game.settings.loading_background = None;
    if stream.next_bool()? {
        game.settings.loading_background = if game.version >= Version::Gm800 {
            Some(stream.next_blob()?)
        } else {
            Some(stream.next_compressed()?.into_inner())
        };
    }

    game.settings.load_transparent = stream.next_bool()?;
    game.settings.load_alpha = stream.next_u32()?;
    game.settings.load_scale = stream.next_bool()?;

    game.settings.error_display = stream.next_bool()?;
    game.settings.error_log = stream.next_bool()?;
    game.settings.error_abort = stream.next_bool()?;
    if version >= 800 {
        let data = stream.next_u32()?;
        game.settings.uninitialized_zero = (data & 0x1) > 0;
        game.settings.uninitialized_arguments_error = (data & 0x2) > 0;
    } else {
        game.settings.uninitialized_zero = stream.next_bool()?;

        // Read constants.
        let num_constants = stream.next_u32()?;
        game.constants.reserve(num_constants as usize);
        for _ in 0..num_constants {
            let mut constant = Constant::default();
            constant.name = stream.next_string()?;
            constant.value = stream.next_string()?;
            game.constants.push(constant);
        }
    }
    Ok(())
}

fn read_extensions(_game: &mut Game, stream: &mut BufferStream) -> io::Result<()> {
    println!("Reading extensions...");
    let version = stream.next_u32()?;
    assert_eq!(version, 700);
    let num_extensions = stream.next_u32()?;
    for _ in 0..num_extensions {
        let version = stream.next_u32()?;
        assert_eq!(version, 700);
        let mut extension = Extension::default();
        extension.name = stream.next_string()?;
        extension.temp_name = stream.next_string()?;

        let file_count = stream.next_u32()?;
        for _ in 0..file_count {
            let version = stream.next_u32()?;
            assert_eq!(version, 700);
            let mut file = ExtensionFile::default();
            file.name = stream.next_string()?;
            file.file_type = stream.next_u32()?;
            file.initialization_function = stream.next_string()?;
            file.finalization_function = stream.next_string()?;

            let function_count = stream.next_u32()?;
            for _ in 0..function_count {
                let version = stream.next_u32()?;
                assert_eq!(version, 700);
                let mut function = ExtensionFunction::default();
                function.name = stream.next_string()?;
                function.external_name = stream.next_string()?;
                function.calling_convention = stream.next_u32()?;
                function.id = stream.next_u32()?;
                let num_arguments = stream.next_i32()?;
                for i in 0..17 {
                    let argument_type = stream.next_u32()?;
                    if i < num_arguments {
                        function.argument_types.push(argument_type);
                    }
                }
                function.return_type = stream.next_u32()?;
                file.functions.push(function);
            }

            // Constants
            let num_constants = stream.next_u32()?;
            for _ in 0..num_constants {
                let version = stream.next_u32()?;
                assert_eq!(version, 700);
                let mut constant = Constant::default();
                constant.name = stream.next_string()?;
                constant.value = stream.next_string()?;
                file.constants.push(constant);
            }
            extension.files.push(file);
        }

        // Read file data.
        let encrypted = Cursor::new(stream.next_blob()?);
        let decrypted = decrypt::deobfuscate(encrypted, 0, false, false)?;
        let mut decrypted = Cursor::new(decrypted);
        for file in &mut extension.files {
            file.data = decrypted.next_compressed()?.into_inner();
        }
    }
    Ok(())
}

fn read_triggers(game: &mut Game, stream: &mut BufferStream) -> io::Result<()> {
    println!("Reading triggers...");
    let _version = stream.next_u32()?;
    let num_triggers = stream.next_u32()?;
    game.triggers.reserve(num_triggers as usize);
    for i in 0..num_triggers {
        let mut stream = stream.next_compressed()?;
        if !stream.next_bool()? {
            continue;
        }

        let _version = stream.next_u32()?;
        let mut trigger = Trigger::default();
        trigger.id = i;
        trigger.name = stream.next_string()?;
        trigger.condition = stream.next_string()?;
        trigger.check_moment = stream.next_u32()?;
        trigger.constant_name = stream.next_string()?;
        game.triggers.push(trigger);
        assert_eof(stream);
    }
    Ok(())
}

fn read_constants(game: &mut Game, stream: &mut BufferStream) -> io::Result<()> {
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
    Ok(())
}

fn read_sounds(game: &mut Game, stream: &mut BufferStream) -> io::Result<()> {
    println!("Reading sounds...");
    let version = stream.next_u32()?;
    let num_sounds = stream.next_u32()?;
    game.sounds.reserve(num_sounds as usize);
    for i in 0..num_sounds {
        let mut stream = SectionWrapper::new(stream, version >= 800)?;
        if !stream.next_bool()? {
            continue;
        }

        let mut sound = Sound::default();
        sound.id = i;
        sound.name = stream.next_string()?;
        let version = stream.next_u32()?;
        if version == 600 || version == 800 {
            sound.kind = stream.next_u32()?;
            sound.filetype = stream.next_string()?;
            sound.filename = stream.next_string()?;
            if stream.next_bool()? {
                sound.data = stream.next_blob()?;
            }
            sound.effects = stream.next_u32()?;
            sound.volume = stream.next_f64()?;
            sound.pan = stream.next_f64()?;
            sound.preload = stream.next_bool()?;
        } else {
            unimplemented!();
        }
        game.sounds.push(sound);
    }
    Ok(())
}

fn read_sprites(game: &mut Game, stream: &mut BufferStream) -> io::Result<()> {
    println!("Reading sprites...");
    let version = stream.next_u32()?;
    let num_sprites = stream.next_u32()?;
    game.sprites.reserve(num_sprites as usize);
    for i in 0..num_sprites {
        let mut stream = SectionWrapper::new(stream, version >= 800)?;
        if !stream.next_bool()? {
            continue;
        }

        let mut sprite = Sprite::default();
        sprite.id = i;
        sprite.name = stream.next_string()?;
        let version = stream.next_u32()?;
        if version == 542 {
            let mut base_mask = SpriteMask::default();
            base_mask.size = (stream.next_u32()?, stream.next_u32()?);
            base_mask.left = stream.next_i32()?;
            base_mask.right = stream.next_i32()?;
            base_mask.bottom = stream.next_i32()?;
            base_mask.top = stream.next_i32()?;
            let _transparent = stream.next_bool()?;
            let _smooth_edges = stream.next_bool()?;
            let _preload = stream.next_bool()?;
            let _bb_type = stream.next_u32()?;
            let precise_collisions = stream.next_bool()?;
            sprite.origin = (stream.next_i32()?, stream.next_i32()?);
            let num_frames = stream.next_u32()? as usize;
            for _ in 0..num_frames {
                let _version = stream.next_u32()?;
                let _present = stream.next_u32()?;
                let width = stream.next_u32()?;
                let height = stream.next_u32()?;
                let data = stream.next_compressed()?.into_inner();
                let image = RgbaImage::from_raw(width, height, data).unwrap();
                sprite.frames.push(Image { inner: image })
            }

            if precise_collisions {
                // If we have precise collisions, do a separate mask for each subimage based on transparency.
                for frame in &sprite.frames {
                    let mut mask = base_mask.clone();
                    mask.size = (frame.inner.width(), frame.inner.height());
                    mask.data.reserve((mask.size.0 * mask.size.1) as usize);
                    for y in 0..mask.size.1 {
                        for x in 0..mask.size.0 {
                            mask.data.push(frame.inner.get_pixel(x, y).channels()[3] == 255);
                        }
                    }
                    sprite.masks.push(mask);
                }
            } else {
                // Otherwise copy the bounding box.
                let mut mask = base_mask;
                mask.data.reserve((mask.size.0 * mask.size.1) as usize);
                for y in 0..mask.size.1 {
                    for x in 0..mask.size.0 {
                        let x = x as i32;
                        let y = y as i32;
                        mask.data.push(x >= mask.left && x <= mask.right && y >= mask.top && y <= mask.bottom);
                    }
                }
                sprite.masks.push(mask);
            }
        } else if version == 800 {
            sprite.origin = (stream.next_i32()?, stream.next_i32()?);

            let num_frames = stream.next_u32()? as usize;
            if num_frames > 0 {
                sprite.frames.reserve(num_frames);
                for _ in 0..num_frames {
                    let _version = stream.next_u32()?;
                    let width = stream.next_u32()?;
                    let height = stream.next_u32()?;
                    let data = stream.next_blob()?;
                    sprite.frames.push(Image {
                        inner: BgraImage::from_raw(width, height, data).unwrap().convert(),
                    });
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
            } else {
                // Weird, because if it has no frames it doesn't matter.
                let _has_separate_masks = stream.next_bool()?;
            }
        } else {
            unimplemented!();
        }

        game.sprites.push(sprite);
    }
    Ok(())
}

fn read_backgrounds(game: &mut Game, stream: &mut BufferStream) -> io::Result<()> {
    println!("Reading backgrounds...");
    let version = stream.next_u32()?;
    let num_backgrounds = stream.next_u32()?;
    game.backgrounds.reserve(num_backgrounds as usize);
    for i in 0..num_backgrounds {
        let mut stream = SectionWrapper::new(stream, version >= 800)?;
        if !stream.next_bool()? {
            continue;
        }

        let mut background = Background::default();
        background.id = i;
        background.name = stream.next_string()?;
        let version = stream.next_u32()?;
        if version == 543 {
            let _width = stream.next_u32()?;
            let _height = stream.next_u32()?;
            let _transparent = stream.next_bool()?;
            let _smooth_edges = stream.next_bool()?;
            let _preload_texture = stream.next_bool()?;
            let has_image = stream.next_bool()?;
            if has_image {
                let _version = stream.next_u32()?;
                let _present = stream.next_u32()?;
                let width = stream.next_u32()?;
                let height = stream.next_u32()?;
                let data = stream.next_compressed()?.into_inner();
                let image = RgbaImage::from_raw(width, height, data).unwrap();
                background.image = Image { inner: image }
            }
        } else if version == 710 {
            let _version2 = stream.next_u32()?;
            let width = stream.next_u32()?;
            let height = stream.next_u32()?;
            let data = if width > 0 && height > 0 {
                stream.next_blob()?
            } else {
                vec![]
            };
            background.image = Image {
                inner: BgraImage::from_raw(width, height, data).unwrap().convert(),
            };
        } else {
            unimplemented!();
        }
        game.backgrounds.push(background);
    }
    Ok(())
}

fn read_paths(game: &mut Game, stream: &mut BufferStream) -> io::Result<()> {
    println!("Reading paths...");
    let version = stream.next_u32()?;
    let num_paths = stream.next_u32()?;
    game.paths.reserve(num_paths as usize);
    for i in 0..num_paths {
        let mut stream = SectionWrapper::new(stream, version >= 800)?;
        if !stream.next_bool()? {
            continue;
        }

        let mut path = Path::default();
        path.id = i;
        path.name = stream.next_string()?;
        let version = stream.next_u32()?;
        if version == 530 {
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
        } else {
            unimplemented!();
        }
        game.paths.push(path);
    }
    Ok(())
}

fn read_scripts(game: &mut Game, stream: &mut BufferStream) -> io::Result<()> {
    println!("Reading scripts...");
    let version = stream.next_u32()?;
    let num_scripts = stream.next_u32()?;
    game.scripts.reserve(num_scripts as usize);
    for i in 0..num_scripts {
        let mut stream = SectionWrapper::new(stream, version >= 800)?;
        if !stream.next_bool()? {
            continue;
        }

        let mut script = Script::default();
        script.id = i;
        script.name = stream.next_string()?;
        let version = stream.next_u32()?;
        if version == 400 {
            let mut compressed = stream.next_compressed()?.into_inner();
            let swap_table = decrypt::make_swap_table(12345);
            decrypt::do_swap(&mut compressed, swap_table, false, 0);
            let mut decrypted = Cursor::new(compressed);
            script.script = decrypted.next_string()?;
        } else if version == 800 {
            script.script = stream.next_string()?;
        } else {
            unimplemented!();
        }
        game.scripts.push(script);
    }
    Ok(())
}

fn read_fonts(game: &mut Game, stream: &mut BufferStream) -> io::Result<()> {
    println!("Reading fonts...");
    let version = stream.next_u32()?;
    let num_fonts = stream.next_u32()?;
    game.fonts.reserve(num_fonts as usize);
    for i in 0..num_fonts {
        let mut stream = SectionWrapper::new(stream, version >= 800)?;
        if !stream.next_bool()? {
            continue;
        }

        let mut font = Font::default();
        font.id = i;
        font.name = stream.next_string()?;
        let version = stream.next_u32()?;
        if version >= 540 {
            font.font_name = stream.next_string()?;
            font.size = stream.next_u32()?;
            font.bold = stream.next_bool()?;
            font.italic = stream.next_bool()?;
            font.range_start = stream.next_u32()?;
            font.range_end = stream.next_u32()?;

            // For GM 8.1.
            font.charset = (font.range_start & 0xFF000000) >> 24;
            font.aa_level = (font.range_start & 0x00FF0000) >> 16;
            font.range_start = font.range_start & 0x0000FFFF;

            let num_glyphs = 256;
            font.atlas.glyphs.reserve(num_glyphs);
            for _ in 0..num_glyphs {
                let mut glyph = FontAtlasGlyph::default();
                glyph.pos = (stream.next_u32()?, stream.next_u32()?);
                glyph.size = (stream.next_u32()?, stream.next_u32()?);
                glyph.horizontal_advance = stream.next_i32()?;
                glyph.kerning = stream.next_i32()?;
                font.atlas.glyphs.push(glyph);
            }
            font.atlas.size = (stream.next_u32()?, stream.next_u32()?);

            if version == 540 {
                font.atlas.data = stream.next_compressed()?.into_inner();
            } else {
                font.atlas.data = stream.next_blob()?;
            }
        } else {
            unimplemented!();
        }

        game.fonts.push(font);
    }
    Ok(())
}

fn read_timelines(game: &mut Game, stream: &mut BufferStream) -> io::Result<()> {
    println!("Reading timelines...");
    let version = stream.next_u32()?;
    let num_timelines = stream.next_u32()?;
    game.timelines.reserve(num_timelines as usize);
    for i in 0..num_timelines {
        let mut stream = SectionWrapper::new(stream, version >= 800)?;
        if !stream.next_bool()? {
            continue;
        }

        let mut timeline = Timeline::default();
        timeline.id = i;
        timeline.name = stream.next_string()?;
        let version = stream.next_u32()?;
        if version == 500 {
            let num_moments = stream.next_u32()?;
            timeline.moments.reserve(num_moments as usize);
            for _ in 0..num_moments {
                let mut moment = TimelineMoment::default();
                moment.position = stream.next_u32()?;
                moment.actions = read_actions(&mut stream)?;
                timeline.moments.push(moment);
            }
        } else {
            unimplemented!();
        }
        game.timelines.push(timeline);
    }
    Ok(())
}

fn read_objects(game: &mut Game, stream: &mut BufferStream) -> io::Result<()> {
    println!("Reading objects...");
    let version = stream.next_u32()?;
    let num_objects = stream.next_u32()?;
    game.objects.reserve(num_objects as usize);
    for i in 0..num_objects {
        let mut stream = SectionWrapper::new(stream, version >= 800)?;
        if !stream.next_bool()? {
            continue;
        }

        let mut object = Object::default();
        object.id = i;
        object.name = stream.next_string()?;
        let version = stream.next_u32()?;
        if version == 430 {
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
        } else {
            unimplemented!();
        }

        game.objects.push(object);
    }
    Ok(())
}

fn read_rooms(game: &mut Game, stream: &mut BufferStream) -> io::Result<()> {
    println!("Reading rooms...");
    let version = stream.next_u32()?;
    let num_rooms = stream.next_u32()?;
    game.rooms.reserve(num_rooms as usize);
    for i in 0..num_rooms {
        let mut stream = SectionWrapper::new(stream, version >= 800)?;
        if !stream.next_bool()? {
            continue;
        }

        let mut room = Room::default();
        room.id = i;
        room.name = stream.next_string()?;
        let version = stream.next_u32()?;
        if version == 541 {
            room.caption = stream.next_string()?;
            room.width = stream.next_u32()?;
            room.height = stream.next_u32()?;
            room.speed = stream.next_u32()?;
            room.persistent = stream.next_bool()?;
            room.clear_color = stream.next_u32()?;
            room.clear = stream.next_bool()?;
            room.creation_code = stream.next_string()?;

            let num_backgrounds = stream.next_u32()?;
            for _ in 0..num_backgrounds {
                let mut background = RoomBackground::default();
                background.visible = stream.next_bool()?;
                background.foreground = stream.next_bool()?;
                background.background = stream.next_i32()?;
                background.x = stream.next_i32()?;
                background.y = stream.next_i32()?;
                background.tile_h = stream.next_bool()?;
                background.tile_v = stream.next_bool()?;
                background.h_speed = stream.next_i32()?;
                background.v_speed = stream.next_i32()?;
                background.stretch = stream.next_bool()?;
                room.backgrounds.push(background);
            }

            room.enable_views = stream.next_bool()?;
            let num_views = stream.next_u32()?;
            for _ in 0..num_views {
                let mut view = RoomView::default();
                view.visible = stream.next_bool()?;
                view.view_x = stream.next_u32()?;
                view.view_y = stream.next_u32()?;
                view.view_width = stream.next_u32()?;
                view.view_height = stream.next_u32()?;
                view.port_x = stream.next_u32()?;
                view.port_y = stream.next_u32()?;
                view.port_width = stream.next_u32()?;
                view.port_height = stream.next_u32()?;
                view.h_border = stream.next_u32()?;
                view.v_border = stream.next_u32()?;
                view.h_speed = stream.next_i32()?;
                view.v_speed = stream.next_i32()?;
                view.target_object = stream.next_i32()?;
                room.views.push(view);
            }

            let num_instances = stream.next_u32()?;
            for _ in 0..num_instances {
                let mut instance = RoomInstance::default();
                instance.x = stream.next_i32()?;
                instance.y = stream.next_i32()?;
                instance.object = stream.next_i32()?;
                instance.id = stream.next_i32()?;
                instance.creation_code = stream.next_string()?;
                room.instances.push(instance);
            }

            let num_tiles = stream.next_u32()?;
            for _ in 0..num_tiles {
                let mut tile = RoomTile::default();
                tile.x = stream.next_i32()?;
                tile.y = stream.next_i32()?;
                tile.background = stream.next_i32()?;
                tile.tile_x = stream.next_i32()?;
                tile.tile_y = stream.next_i32()?;
                tile.width = stream.next_u32()?;
                tile.height = stream.next_u32()?;
                tile.depth = stream.next_i32()?;
                tile.id = stream.next_i32()?;
                room.tiles.push(tile);
            }
        } else {
            unimplemented!();
        }

        game.rooms.push(room);
    }
    Ok(())
}

fn read_includes(game: &mut Game, stream: &mut BufferStream) -> io::Result<()> {
    println!("Reading includes...");
    let version = stream.next_u32()?;
    let num_includes = stream.next_u32()?;
    game.includes.reserve(num_includes as usize);
    for _ in 0..num_includes {
        let mut stream = SectionWrapper::new(stream, version >= 800)?;
        let mut include = Include::default();
        let version = stream.next_u32()?;

        if version == 620 || version == 800 {
            include.name = stream.next_string()?;
            include.original_path = stream.next_string()?;
            include.original_chosen = stream.next_bool()?;
            include.original_size = stream.next_u32()?;
            include.store_in_editable = stream.next_bool()?;
            if include.original_chosen && include.store_in_editable {
                if version == 620 {
                    include.data = stream.next_compressed()?.into_inner();
                } else {
                    include.data = stream.next_blob()?;
                }
            }
            include.export = stream.next_u32()?;
            include.export_folder = stream.next_string()?;
            include.overwrite = stream.next_bool()?;
            include.free_memory = stream.next_bool()?;
            include.remove_at_end = stream.next_bool()?;
        } else {
            unimplemented!();
        }

        game.includes.push(include);
    }
    Ok(())
}

fn read_help(game: &mut Game, stream: &mut BufferStream) -> io::Result<()> {
    println!("Reading help...");
    let version = stream.next_u32()?;
    let mut stream = SectionWrapper::new(stream, version >= 800)?;
    if version >= 600 {
        game.help.background_color = stream.next_u32()?;
        game.help.separate_window = stream.next_bool()?;
        game.help.caption = stream.next_string()?;
        game.help.left = stream.next_i32()?;
        game.help.top = stream.next_i32()?;
        game.help.width = stream.next_i32()?;
        game.help.height = stream.next_i32()?;
        game.help.show_border = stream.next_bool()?;
        game.help.allow_resize = stream.next_bool()?;
        game.help.always_on_top = stream.next_bool()?;
        game.help.freeze_game = stream.next_bool()?;
        if version == 800 {
            game.help.content = stream.next_string()?;
        } else {
            let data = stream.next_compressed()?.into_inner();
            game.help.content = decode_string(&data);
        }
    } else {
        unimplemented!();
    }
    Ok(())
}

fn read_library_init_scripts(game: &mut Game, stream: &mut BufferStream) -> io::Result<()> {
    println!("Reading library init scripts...");
    let version = stream.next_u32()?;
    if version == 500 {
        let num_init_scripts = stream.next_u32()?;
        for _ in 0..num_init_scripts {
            game.library_init_scripts.push(stream.next_string()?);
        }
    } else {
        unimplemented!();
    }
    Ok(())
}

fn read_room_order(game: &mut Game, stream: &mut BufferStream) -> io::Result<()> {
    println!("Reading room order...");
    let version = stream.next_u32()?;
    if version == 540 || version == 700 {
        // What is the difference between 540 and 700?
        let num_rooms = stream.next_u32()?;
        game.room_order.reserve(num_rooms as usize);
        for _ in 0..num_rooms {
            game.room_order.push(stream.next_u32()?);
        }
    } else {
        unimplemented!();
    }
    Ok(())
}

fn parse_gm8xx_exe(game: &mut Game, mut stream: &mut BufferStream) -> io::Result<()> {
    game.debug = stream.next_bool()?;

    read_settings(game, &mut stream)?;

    // Skip d3dx8.dll (name and then content).
    stream.skip_blob()?;
    stream.skip_blob()?;

    // Do the main "decryption" and skip junk.
    println!("Decrypting inner...");
    let mut stream = decrypt::decrypt_gm8xx(stream)?;
    let len = stream.next_u32()?;
    stream.skip(len * 4)?;

    game.pro = stream.next_bool()?;
    game.game_id = stream.next_u32()?;
    for i in 0..4 {
        game.guid[i] = stream.next_u32()?;
    }

    read_extensions(game, &mut stream)?;
    read_triggers(game, &mut stream)?;
    read_constants(game, &mut stream)?;
    read_sounds(game, &mut stream)?;
    read_sprites(game, &mut stream)?;
    read_backgrounds(game, &mut stream)?;
    read_paths(game, &mut stream)?;
    read_scripts(game, &mut stream)?;
    read_fonts(game, &mut stream)?;
    read_timelines(game, &mut stream)?;
    read_objects(game, &mut stream)?;
    read_rooms(game, &mut stream)?;

    game.last_instance_id = stream.next_u32()?;
    game.last_tile_id = stream.next_u32()?;

    read_includes(game, &mut stream)?;
    read_help(game, &mut stream)?;
    read_library_init_scripts(game, &mut stream)?;
    read_room_order(game, &mut stream)?;

    // Garbage data here.

    Ok(())
}

fn parse_gm700_exe(game: &mut Game, mut stream: &mut BufferStream) -> io::Result<()> {
    game.debug = stream.next_bool()?;

    read_settings(game, &mut stream)?;

    // Skip d3dx8.dll (name and then content).
    stream.skip_blob()?;
    stream.skip_blob()?;

    println!("Decrypting inner...");
    let mut stream = decrypt::decrypt_gm700(stream)?;

    game.pro = stream.next_bool()?;
    game.game_id = stream.next_u32()?;
    for i in 0..4 {
        game.guid[i] = stream.next_u32()?;
    }

    read_extensions(game, &mut stream)?;
    read_sounds(game, &mut stream)?;
    read_sprites(game, &mut stream)?;
    read_backgrounds(game, &mut stream)?;
    read_paths(game, &mut stream)?;
    read_scripts(game, &mut stream)?;
    read_fonts(game, &mut stream)?;
    read_timelines(game, &mut stream)?;
    read_objects(game, &mut stream)?;
    read_rooms(game, &mut stream)?;

    game.last_instance_id = stream.next_u32()?;
    game.last_tile_id = stream.next_u32()?;

    read_includes(game, &mut stream)?;
    read_help(game, &mut stream)?;
    read_library_init_scripts(game, &mut stream)?;
    read_room_order(game, &mut stream)?;

    // Garbage data here.

    Ok(())
}

fn parse_gm600_exe(game: &mut Game, stream: &mut BufferStream) -> io::Result<()> {
    println!("Reading includes...");
    {
        let export_location = stream.next_u32()?;
        let overwrite = stream.next_bool()?;
        let remove_at_game_end = stream.next_bool()?;
        loop {
            let name = stream.next_string()?;
            if "READY" == name {
                break;
            } else if "D3DX8.dll" == name {
                stream.skip_blob()?;
            } else {
                let mut include = Include::default();
                include.name = name;
                include.data = stream.next_blob()?;
                include.export = export_location;
                include.overwrite = overwrite;
                include.free_memory = true;
                include.remove_at_end = remove_at_game_end;
                game.includes.push(include);
            }
        }
    }

    println!("Decrypting inner...");
    let mut stream = decrypt::decrypt_gm600(stream)?;

    println!("Reading header...");
    assert_eq!(stream.next_u32()?, 1230600);
    let _unknown1 = stream.next_u32()?;
    let _unknown2 = stream.next_u32()?;
    game.pro = stream.next_bool()?;
    let _unknown4 = stream.next_u32()?;
    assert_eq!(stream.next_u32()?, 1234321);
    assert_eq!(stream.next_u32()?, 600);
    game.debug = stream.next_bool()?;
    game.game_id = stream.next_u32()?;
    for i in 0..4 {
        game.guid[i] = stream.next_u32()?;
    }

    read_settings(game, &mut stream)?;
    read_sounds(game, &mut stream)?;
    read_sprites(game, &mut stream)?;
    read_backgrounds(game, &mut stream)?;
    read_paths(game, &mut stream)?;
    read_scripts(game, &mut stream)?;
    read_fonts(game, &mut stream)?;
    read_timelines(game, &mut stream)?;
    read_objects(game, &mut stream)?;
    read_rooms(game, &mut stream)?;

    game.last_instance_id = stream.next_u32()?;
    game.last_tile_id = stream.next_u32()?;

    read_help(game, &mut stream)?;
    read_library_init_scripts(game, &mut stream)?;
    read_room_order(game, &mut stream)?;

    // Garbage data here.

    Ok(())
}


pub fn decode<T: Read + Seek>(stream: T) -> io::Result<Game> {
    let mut project = Game::default();
    project.version = Version::Unknown;

    if let Some(data) = detect::decode(stream) {
        project.version = data.version;
        println!("Detected {:?}.", project.version);
        let mut stream = Cursor::new(data.data);
        match project.version {
            Version::Gm800 | Version::Gm810 => parse_gm8xx_exe(&mut project, &mut stream)?,
            Version::Gm700 => parse_gm700_exe(&mut project, &mut stream)?,
            Version::Gm600 => parse_gm600_exe(&mut project, &mut stream)?,
            _ => unimplemented!()
        }
    }

    Ok(project)
}
