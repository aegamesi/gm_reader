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
}

#[derive(Default)]
pub struct Sprite {
    pub id: u32,
    pub name: String,
    pub origin: (i32, i32),

    pub frames: Vec<SpriteFrame>,
    pub masks: Vec<SpriteMask>,
}

#[derive(Default)]
pub struct SpriteFrame {
    pub size: (u32, u32),
    pub data: Vec<u8>,
}

#[derive(Default)]
pub struct SpriteMask {
    pub size: (u32, u32),
    pub left: i32,
    pub right: i32,
    pub bottom: i32,
    pub top: i32,
    pub data: Vec<bool>,
}