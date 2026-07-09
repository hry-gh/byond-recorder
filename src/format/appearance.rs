use std::io::{self, Write};

use byteorder::{LittleEndian, WriteBytesExt};

pub const APPEARANCE_MAGIC: &[u8; 4] = b"BYAP";
pub const APPEARANCE_VERSION: u8 = 1;

const HAS_TRANSFORM: u16 = 0x01;
const HAS_COLOR_MATRIX: u16 = 0x02;
const HAS_OVERLAYS: u16 = 0x04;
const HAS_UNDERLAYS: u16 = 0x08;
const HAS_SCREEN_LOC: u16 = 0x10;

#[derive(Debug, Clone)]
pub struct Appearance {
    pub id: u32,
    pub icon_id: u32,
    pub icon_state: String,
    pub name: String,
    pub dir: u8,
    pub pixel_x: i16,
    pub pixel_y: i16,
    pub pixel_w: i16,
    pub pixel_z: i16,
    pub layer: f32,
    pub plane: i16,
    pub alpha: u8,
    pub color_r: u8,
    pub color_g: u8,
    pub color_b: u8,
    pub blend_mode: u8,
    pub appearance_flags: u32,
    pub glide_size: f32,
    pub invisibility: u8,
    pub mouse_opacity: u8,
    pub transform: Option<[f32; 6]>,
    pub color_matrix: Option<[f32; 20]>,
    pub overlay_ids: Vec<u32>,
    pub underlay_ids: Vec<u32>,
    pub screen_loc: Option<String>,
}

impl Default for Appearance {
    fn default() -> Self {
        Self {
            id: 0,
            icon_id: 0,
            icon_state: String::new(),
            name: String::new(),
            dir: 2,
            pixel_x: 0,
            pixel_y: 0,
            pixel_w: 0,
            pixel_z: 0,
            layer: 2.0,
            plane: 0,
            alpha: 255,
            color_r: 255,
            color_g: 255,
            color_b: 255,
            blend_mode: 0,
            appearance_flags: 0,
            glide_size: 0.0,
            invisibility: 0,
            mouse_opacity: 1,
            transform: None,
            color_matrix: None,
            overlay_ids: Vec::new(),
            underlay_ids: Vec::new(),
            screen_loc: None,
        }
    }
}

fn write_string<W: Write>(w: &mut W, s: &str) -> io::Result<()> {
    let bytes = s.as_bytes();
    w.write_u16::<LittleEndian>(bytes.len() as u16)?;
    w.write_all(bytes)
}

impl Appearance {
    pub fn serialize_data(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(128);
        let w = &mut buf;

        w.write_u32::<LittleEndian>(self.icon_id).unwrap();
        write_string(w, &self.icon_state).unwrap();
        write_string(w, &self.name).unwrap();
        w.write_u8(self.dir).unwrap();
        w.write_i16::<LittleEndian>(self.pixel_x).unwrap();
        w.write_i16::<LittleEndian>(self.pixel_y).unwrap();
        w.write_i16::<LittleEndian>(self.pixel_w).unwrap();
        w.write_i16::<LittleEndian>(self.pixel_z).unwrap();
        w.write_f32::<LittleEndian>(self.layer).unwrap();
        w.write_i16::<LittleEndian>(self.plane).unwrap();
        w.write_u8(self.alpha).unwrap();
        w.write_u8(self.color_r).unwrap();
        w.write_u8(self.color_g).unwrap();
        w.write_u8(self.color_b).unwrap();
        w.write_u8(self.blend_mode).unwrap();
        w.write_u32::<LittleEndian>(self.appearance_flags).unwrap();
        w.write_f32::<LittleEndian>(self.glide_size).unwrap();
        w.write_u8(self.invisibility).unwrap();
        w.write_u8(self.mouse_opacity).unwrap();

        let mut presence: u16 = 0;
        if self.transform.is_some() {
            presence |= HAS_TRANSFORM;
        }
        if self.color_matrix.is_some() {
            presence |= HAS_COLOR_MATRIX;
        }
        if !self.overlay_ids.is_empty() {
            presence |= HAS_OVERLAYS;
        }
        if !self.underlay_ids.is_empty() {
            presence |= HAS_UNDERLAYS;
        }
        if self.screen_loc.is_some() {
            presence |= HAS_SCREEN_LOC;
        }
        w.write_u16::<LittleEndian>(presence).unwrap();

        if let Some(ref t) = self.transform {
            for v in t {
                w.write_f32::<LittleEndian>(*v).unwrap();
            }
        }
        if let Some(ref cm) = self.color_matrix {
            for v in cm {
                w.write_f32::<LittleEndian>(*v).unwrap();
            }
        }
        if !self.overlay_ids.is_empty() {
            w.write_u16::<LittleEndian>(self.overlay_ids.len() as u16)
                .unwrap();
            for id in &self.overlay_ids {
                w.write_u32::<LittleEndian>(*id).unwrap();
            }
        }
        if !self.underlay_ids.is_empty() {
            w.write_u16::<LittleEndian>(self.underlay_ids.len() as u16)
                .unwrap();
            for id in &self.underlay_ids {
                w.write_u32::<LittleEndian>(*id).unwrap();
            }
        }
        if let Some(ref sl) = self.screen_loc {
            write_string(w, sl).unwrap();
        }

        buf
    }
}
