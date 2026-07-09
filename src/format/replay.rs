pub const REPLAY_MAGIC: &[u8; 4] = b"BRCR";
pub const REPLAY_VERSION: u8 = 1;
pub const EOF_MARKER: u32 = 0xFFFFFFFF;

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameType {
    MapResize = 0x01,
    TurfUpdate = 0x02,
    MovableUpdate = 0x03,
    MovableDelete = 0x04,
    Animation = 0x05,
    Flick = 0x06,
    IconMeta = 0x07,
    EyeUpdate = 0x08,
}

pub const UPD_APPEARANCE: u8 = 0x01;
pub const UPD_LOCATION: u8 = 0x02;
pub const UPD_PIXEL: u8 = 0x04;
pub const UPD_GLIDE: u8 = 0x08;
pub const UPD_VIS: u8 = 0x10;

#[derive(Debug, Clone)]
pub struct ReplayHeader {
    pub byond_major: u16,
    pub byond_minor: u16,
    pub round_id: String,
    pub ckey: String,
    pub start_time: u64,
    pub maxx: u16,
    pub maxy: u16,
    pub maxz: u16,
}

#[derive(Debug, Clone)]
pub struct TurfEntry {
    pub x: u16,
    pub y: u16,
    pub z: u16,
    pub appearance_id: u32,
}

#[derive(Debug, Clone)]
pub struct MovableEntry {
    pub atom_id: u32,
    pub flags: u8,
    pub appearance_id: Option<u32>,
    pub location: Option<(u16, u16, u16)>,
    pub pixel: Option<(i16, i16, i16, i16)>,
    pub glide_size: Option<f32>,
    pub vis_contents: Option<Vec<u32>>,
}

#[derive(Debug, Clone)]
pub struct AnimationKeyframe {
    pub from_appearance_id: u32,
    pub to_appearance_id: u32,
    pub duration: f32,
    pub loop_count: i32,
    pub easing: u8,
    pub flags: u8,
    pub parallel_start: Option<f32>,
}

#[derive(Debug, Clone)]
pub struct AnimationFrame {
    pub atom_id: u32,
    pub start_time: f32,
    pub is_new: bool,
    pub sequence: u16,
    pub keyframes: Vec<AnimationKeyframe>,
}

#[derive(Debug, Clone)]
pub struct FlickFrame {
    pub atom_id: u32,
    pub icon_id: Option<u32>,
    pub icon_state: Option<String>,
}

#[derive(Debug, Clone)]
pub struct IconState {
    pub name: String,
    pub flags: u8,
    pub num_dirs: u8,
    pub num_frames: u16,
    pub loop_count: u16,
    pub delays: Option<Vec<f32>>,
    pub frame_indices: Vec<Vec<u16>>,
}

#[derive(Debug, Clone)]
pub struct IconMeta {
    pub icon_id: u32,
    pub resource_id: u32,
    pub width: u16,
    pub height: u16,
    pub states: Vec<IconState>,
}

#[derive(Debug, Clone)]
pub struct EyeUpdate {
    pub x: f32,
    pub y: f32,
    pub z: i16,
    pub sight: u32,
    pub see_invisible: u8,
}

#[derive(Debug, Clone)]
pub enum Frame {
    MapResize { maxx: u16, maxy: u16, maxz: u16 },
    TurfUpdate(Vec<TurfEntry>),
    MovableUpdate(Vec<MovableEntry>),
    MovableDelete(u32),
    Animation(AnimationFrame),
    Flick(FlickFrame),
    IconMeta(IconMeta),
    EyeUpdate(EyeUpdate),
}
