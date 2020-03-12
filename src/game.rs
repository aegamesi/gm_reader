#[derive(Debug)]
pub enum Version {
    Unknown = 0,
    Gm530 = 530,
    Gm600 = 600,
    Gm700 = 700,
    Gm800 = 800,
    Gm810 = 810,
}

impl Default for Version {
    fn default() -> Self {
        Version::Unknown
    }
}

#[derive(Default)]
pub struct Game {
    pub version: Version,
    pub debug: bool,
    pub pro: bool,
    pub game_id: u32,
    pub guid: [u32; 4],

    pub sprites: Vec<Sprite>,
    pub sounds: Vec<Sound>,
    pub backgrounds: Vec<Background>,
    pub paths: Vec<Path>,
    pub scripts: Vec<Script>,
    pub fonts: Vec<Font>,
}

#[derive(Default, Debug)]
pub struct Sound {
    pub id: u32,
    pub name: String,
    pub kind: u32,
    pub filetype: String,
    pub filename: String,
    pub data: Vec<u8>,
    pub effects: u32,
    pub volume: f64,
    pub pan: f64,
    pub preload: bool,
}

#[derive(Default, Debug)]
pub struct Sprite {
    pub id: u32,
    pub name: String,
    pub origin: (i32, i32),

    pub frames: Vec<SpriteFrame>,
    pub masks: Vec<SpriteMask>,
}

#[derive(Default, Debug)]
pub struct SpriteFrame {
    pub size: (u32, u32),
    pub data: Vec<u8>,
}

#[derive(Default, Debug)]
pub struct SpriteMask {
    pub size: (u32, u32),
    pub left: i32,
    pub right: i32,
    pub bottom: i32,
    pub top: i32,
    pub data: Vec<bool>,
}

#[derive(Default, Debug)]
pub struct Background {
    pub id: u32,
    pub name: String,
    pub size: (u32, u32),
    pub data: Vec<u8>,
}

#[derive(Default, Debug)]
pub struct Path {
    pub id: u32,
    pub name: String,
    pub connection_type: u32,
    pub closed: bool,
    pub precision: u32,
    pub points: Vec<PathPoint>,
}

#[derive(Default, Debug)]
pub struct PathPoint {
    pub x: f64,
    pub y: f64,
    pub speed: f64,
}

#[derive(Default, Debug)]
pub struct Script {
    pub id: u32,
    pub name: String,
    pub script: String,
}

#[derive(Default)]
pub struct Font {
    pub id: u32,
    pub name: String,
    pub font_name: String,
    pub size: u32,
    pub bold: bool,
    pub italic: bool,
    pub range_start: u32,
    pub range_end: u32,
    pub charset: u32,
    pub aa_level: u32,
    pub atlas: FontAtlas,
}

pub struct FontAtlas {
    pub glyphs: [FontAtlasGlyph; 256],
    pub size: (u32, u32),
    pub data: Vec<u8>,
}

impl Default for FontAtlas {
    fn default() -> Self {
        FontAtlas {
            glyphs: [FontAtlasGlyph::default(); 256],
            size: (0, 0),
            data: vec![],
        }
    }
}

#[derive(Default, Copy, Clone, Debug)]
pub struct FontAtlasGlyph {
    pub pos: (u32, u32),
    pub size: (u32, u32),
    pub horizontal_advance: i32,
    pub kerning: i32,
}