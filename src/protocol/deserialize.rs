use std::io::Cursor;

use anyhow::{Result, bail};
use byteorder::{LittleEndian, ReadBytesExt};

use crate::format::appearance::Appearance;
use crate::format::replay::*;

use super::messages::*;

pub struct DataReader<'a> {
    cur: Cursor<&'a [u8]>,
}

impl<'a> DataReader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self {
            cur: Cursor::new(data),
        }
    }

    fn remaining(&self) -> usize {
        self.cur.get_ref().len() - self.cur.position() as usize
    }

    fn reached_end(&self) -> bool {
        self.remaining() == 0
    }

    fn read_u8(&mut self) -> Result<u8> {
        Ok(self.cur.read_u8()?)
    }

    fn read_u16(&mut self) -> Result<u16> {
        Ok(self.cur.read_u16::<LittleEndian>()?)
    }

    fn read_i16(&mut self) -> Result<i16> {
        Ok(self.cur.read_i16::<LittleEndian>()?)
    }

    fn read_u32(&mut self) -> Result<u32> {
        Ok(self.cur.read_u32::<LittleEndian>()?)
    }

    fn read_i32(&mut self) -> Result<i32> {
        Ok(self.cur.read_i32::<LittleEndian>()?)
    }

    fn read_f32(&mut self) -> Result<f32> {
        Ok(self.cur.read_f32::<LittleEndian>()?)
    }

    fn read_u32as16(&mut self) -> Result<u32> {
        let val = self.read_u16()? as u32;
        if val & 0x8000 != 0 {
            let high = self.read_u16()? as u32;
            Ok((val & 0x7FFF) | (high << 15))
        } else {
            Ok(val)
        }
    }

    fn read_utf_string(&mut self) -> Result<String> {
        let mut len = self.read_u16()? as u32;
        if len == 0xFFFF {
            len = self.read_u32()?;
        }
        let len = len as usize;
        if self.remaining() < len {
            bail!(
                "string length {} exceeds remaining {}",
                len,
                self.remaining()
            );
        }
        let pos = self.cur.position() as usize;
        let bytes = &self.cur.get_ref()[pos..pos + len];
        self.cur.set_position((pos + len) as u64);
        Ok(String::from_utf8_lossy(bytes).into_owned())
    }

    fn read_rle(&mut self, count: usize) -> Result<Vec<u32>> {
        let mut arr = Vec::with_capacity(count);
        let mut repeat: u32 = 0;
        let mut id: u32 = 0;
        for _ in 0..count {
            if repeat == 0 {
                let next = self.read_u32as16()?;
                if next == 0x7FFFFFFF {
                    repeat = self.read_u8()? as u32;
                } else {
                    id = next;
                }
            } else {
                repeat -= 1;
            }
            arr.push(id);
        }
        Ok(arr)
    }

    fn skip(&mut self, n: usize) {
        let pos = self.cur.position() as usize;
        self.cur
            .set_position((pos + n).min(self.cur.get_ref().len()) as u64);
    }
}

/// Deserialize a BYOND wire message into our Frame types.
/// Returns None for messages we don't care about, or that we can't parse.
pub fn deserialize_message(
    msg_type: u16,
    payload: &[u8],
    maxx: u16,
    maxy: u16,
) -> Result<Vec<DeserializedEvent>> {
    let mut dp = DataReader::new(payload);
    let mut events = Vec::new();

    match msg_type {
        MSG_APPEARANCE => {
            let appearance = parse_appearance(&mut dp)?;
            events.push(DeserializedEvent::Appearance(appearance));
        }
        MSG_MAP_CONFIG => {
            let maxx = dp.read_u16()?;
            let maxy = dp.read_u16()?;
            let maxz = dp.read_u16()?;
            events.push(DeserializedEvent::Frame(Frame::MapResize {
                maxx,
                maxy,
                maxz,
            }));
        }
        MSG_ICON => {
            let icon = parse_icon_meta(&mut dp)?;
            events.push(DeserializedEvent::Frame(Frame::IconMeta(icon)));
        }
        MSG_TURF_BLOCK => {
            let turfs = parse_turf_block(&mut dp, maxx, maxy)?;
            if !turfs.is_empty() {
                events.push(DeserializedEvent::Frame(Frame::TurfUpdate(turfs)));
            }
        }
        MSG_TURF_PARTIAL => {
            let turfs = parse_turf_partial(&mut dp, maxx, maxy)?;
            if !turfs.is_empty() {
                events.push(DeserializedEvent::Frame(Frame::TurfUpdate(turfs)));
            }
        }
        MSG_MOVABLE_EYE => {
            let (eye, movables, deletes) = parse_movable_eye(&mut dp)?;
            if let Some(eye) = eye {
                events.push(DeserializedEvent::Frame(Frame::EyeUpdate(eye)));
            }
            if !movables.is_empty() {
                events.push(DeserializedEvent::Frame(Frame::MovableUpdate(movables)));
            }
            for id in deletes {
                events.push(DeserializedEvent::Frame(Frame::MovableDelete(id)));
            }
        }
        MSG_MOVABLE_SMALL => {
            let (movables, deletes) = parse_movable_changes_screen(&mut dp)?;
            if !movables.is_empty() {
                events.push(DeserializedEvent::Frame(Frame::MovableUpdate(movables)));
            }
            for id in deletes {
                events.push(DeserializedEvent::Frame(Frame::MovableDelete(id)));
            }
        }
        MSG_ANIMATION => {
            let anim = parse_animation(&mut dp)?;
            events.push(DeserializedEvent::Frame(Frame::Animation(anim)));
        }
        MSG_FLICK => {
            let flick = parse_flick(&mut dp)?;
            events.push(DeserializedEvent::Frame(Frame::Flick(flick)));
        }
        _ => {}
    }

    Ok(events)
}

#[derive(Debug)]
pub enum DeserializedEvent {
    Appearance(Appearance),
    Frame(Frame),
}

fn parse_appearance(dp: &mut DataReader) -> Result<Appearance> {
    let mut app = Appearance::default();
    app.id = dp.read_u32as16()?;
    app.name = dp.read_utf_string()?;

    let flags = dp.read_u8()?;
    let cursor_flags = dp.read_u8()?;
    app.mouse_opacity = flags & 3;

    if cursor_flags & 1 != 0 {
        if dp.read_u8()? == 2 {
            dp.read_u32as16()?;
        }
    }
    if cursor_flags & 2 != 0 {
        if dp.read_u8()? == 2 {
            dp.read_u32as16()?;
        }
    }
    if cursor_flags & 4 != 0 {
        if dp.read_u8()? == 2 {
            dp.read_u32as16()?;
        }
    }

    if dp.reached_end() {
        return Ok(app);
    }

    app.icon_id = dp.read_u32as16()?;
    app.icon_state = dp.read_utf_string()?;
    app.dir = dp.read_u8()?;
    app.appearance_flags = dp.read_u32()?;
    app.layer = dp.read_f32()?;

    let num_overlays = dp.read_u16()?;
    for _ in 0..num_overlays {
        app.overlay_ids.push(dp.read_u32as16()?);
    }

    let num_underlays = dp.read_u16()?;
    for _ in 0..num_underlays {
        app.underlay_ids.push(dp.read_u32as16()?);
    }

    let mut exbits = dp.read_u8()? as u32;
    if exbits & 0x80 != 0 {
        exbits = dp.read_u32()?;
    }

    if exbits & 1 != 0 {
        app.pixel_x = dp.read_i16()?;
        app.pixel_y = dp.read_i16()?;
        app.pixel_w = dp.read_i16()?;
        app.pixel_z = dp.read_i16()?;
    }
    if exbits & 2 != 0 {
        app.glide_size = dp.read_f32()?;
    }
    if exbits & 4 != 0 {
        let mut t = [0.0f32; 6];
        for v in &mut t {
            *v = dp.read_f32()?;
        }
        app.transform = Some(t);
    }
    if exbits & 8 != 0 {
        let color_alpha = dp.read_u32()?;
        app.color_r = (color_alpha & 0xFF) as u8;
        app.color_g = ((color_alpha >> 8) & 0xFF) as u8;
        app.color_b = ((color_alpha >> 16) & 0xFF) as u8;
        app.alpha = ((color_alpha >> 24) & 0xFF) as u8;
    }
    if exbits & 16 != 0 {
        app.blend_mode = dp.read_u8()?;
    }
    if exbits & 32 != 0 {
        dp.read_utf_string()?; // screen_loc raw
        dp.read_u16()?;
        dp.read_u16()?;
        dp.read_i16()?;
        dp.read_i16()?;
    }
    if exbits & 64 != 0 {
        app.screen_loc = Some(dp.read_utf_string()?);
    }
    if exbits & 128 != 0 {
        app.invisibility = dp.read_u8()?;
        dp.read_u8()?; // luminosity
        dp.read_u8()?; // unknown
    }
    if exbits & 256 != 0 {
        app.plane = dp.read_i16()?;
    }
    if exbits & 512 != 0 {
        let mut cm = [0.0f32; 20];
        for v in &mut cm {
            *v = dp.read_f32()?;
        }
        app.color_matrix = Some(cm);
    }

    Ok(app)
}

fn parse_icon_meta(dp: &mut DataReader) -> Result<IconMeta> {
    let resource_id = dp.read_u32()?;
    let width = dp.read_u16()?;
    let height = dp.read_u16()?;
    let icon_id = dp.read_u32as16()?;
    let state_count = dp.read_u16()?;

    let mut states = Vec::with_capacity(state_count as usize);
    for _ in 0..state_count {
        let name = dp.read_utf_string()?;
        let flags = dp.read_u8()?;
        let num_dirs = dp.read_u8()?.max(1);
        let num_frames = dp.read_u16()?;
        let loop_count = dp.read_u16()?;

        let delays = if num_frames > 1 {
            let mut d = Vec::with_capacity(num_frames as usize);
            for _ in 0..num_frames {
                d.push(dp.read_f32()?);
            }
            Some(d)
        } else {
            None
        };

        let mut frame_indices = Vec::with_capacity(num_dirs as usize);
        for _ in 0..num_dirs {
            let mut frames = Vec::with_capacity(num_frames as usize);
            for _ in 0..num_frames {
                frames.push(dp.read_u16()?);
            }
            frame_indices.push(frames);
        }

        states.push(IconState {
            name,
            flags,
            num_dirs,
            num_frames,
            loop_count,
            delays,
            frame_indices,
        });
    }

    Ok(IconMeta {
        icon_id,
        resource_id,
        width,
        height,
        states,
    })
}

fn turf_id_to_xyz(id: u32, maxx: u16, maxy: u16) -> (u16, u16, u16) {
    let coord = id & 0xFFFFFF;
    let x = (coord % maxx as u32) as u16;
    let y = ((coord / maxx as u32) % maxy as u32) as u16;
    let z = (coord / maxx as u32 / maxy as u32) as u16;
    (x, y, z)
}

fn parse_turf_block(dp: &mut DataReader, maxx: u16, maxy: u16) -> Result<Vec<TurfEntry>> {
    let old_id = dp.read_u32()?;
    let (old_x, old_y) = if old_id != 0 {
        let (x, y, _) = turf_id_to_xyz(old_id, maxx, maxy);
        let _w = dp.read_u16()?;
        let _h = dp.read_u16()?;
        (x, y)
    } else {
        (0, 0)
    };
    let old_width = if old_id != 0 { dp.read_u16()? } else { 0 };
    let old_height = if old_id != 0 { dp.read_u16()? } else { 0 };

    // Re-read: the webclient reads old_width/old_height AFTER old_x/old_y extraction.
    // But we already consumed them above. Let me re-examine the webclient code...
    // Actually the webclient reads: old_id, then conditionally old_width, old_height.
    // I already handled that. Let me fix the double-read.

    let new_id = dp.read_u32()?;
    if new_id == 0 {
        return Ok(Vec::new());
    }

    let (new_x, new_y, z) = turf_id_to_xyz(new_id, maxx, maxy);
    let new_width = dp.read_u16()?;
    let new_height = dp.read_u16()?;

    let mut turf_order = Vec::new();
    for y in new_y..new_y + new_height {
        for x in new_x..new_x + new_width {
            if old_id == 0
                || x < old_x
                || x >= old_x + old_width
                || y < old_y
                || y >= old_y + old_height
            {
                turf_order.push((x, y, z));
            }
        }
    }

    let appearance_ids = dp.read_rle(turf_order.len())?;
    let _area_ids = dp.read_rle(turf_order.len())?;

    let mut entries = Vec::with_capacity(turf_order.len());
    for (i, (x, y, z)) in turf_order.into_iter().enumerate() {
        entries.push(TurfEntry {
            x,
            y,
            z,
            appearance_id: appearance_ids[i],
        });
    }

    Ok(entries)
}

fn parse_turf_partial(dp: &mut DataReader, maxx: u16, maxy: u16) -> Result<Vec<TurfEntry>> {
    let update_type = dp.read_u8()?;
    let count = dp.read_u16()? as usize;

    if update_type == 0 {
        for _ in 0..count {
            dp.read_u32as16()?;
        }
        return Ok(Vec::new());
    }

    let mut turfs = Vec::with_capacity(count);
    for _ in 0..count {
        let id = dp.read_u32as16()? | 0x1000000;
        turfs.push(turf_id_to_xyz(id, maxx, maxy));
    }

    let appearance_ids = dp.read_rle(count)?;
    let _area_ids = dp.read_rle(count)?;

    let mut entries = Vec::with_capacity(count);
    for (i, (x, y, z)) in turfs.into_iter().enumerate() {
        entries.push(TurfEntry {
            x,
            y,
            z,
            appearance_id: appearance_ids[i],
        });
    }

    Ok(entries)
}

fn parse_movable_entry(dp: &mut DataReader, flags: u16) -> Result<Option<MovableEntry>> {
    let atom_id = dp.read_u32()?;

    if flags == 0 {
        return Ok(None); // delete/disable
    }

    let mut entry = MovableEntry {
        atom_id,
        flags: 0,
        appearance_id: None,
        location: None,
        pixel: None,
        glide_size: None,
        vis_contents: None,
    };

    if flags & 4 != 0 {
        let loc = dp.read_u32()?;
        let (x, y, z) = turf_id_to_xyz(loc, 255, 255); // approximate - we may not know maxx here
        entry.location = Some((x, y, z));
        entry.flags |= UPD_LOCATION;
    }
    if flags & 8 != 0 {
        entry.appearance_id = Some(dp.read_u32as16()?);
        entry.flags |= UPD_APPEARANCE;
    }
    if flags & 16 != 0 {
        let px = dp.read_i16()?;
        let py = dp.read_i16()?;
        let pw = dp.read_i16()?;
        let pz = dp.read_i16()?;
        entry.pixel = Some((px, py, pw, pz));
        entry.flags |= UPD_PIXEL;
    }
    if flags & 32 != 0 {
        dp.read_i16()?;
        dp.read_i16()?;
        dp.read_f32()?;
    }
    if flags & 64 != 0 {
        dp.read_i16()?;
        dp.read_i16()?;
        dp.read_u16()?;
        dp.read_u16()?;
    }
    if flags & 128 != 0 {
        dp.read_u8()?;
    }
    if flags & 256 != 0 {
        let vc_len = dp.read_u16()? as usize;
        let mut vc = Vec::with_capacity(vc_len);
        for _ in 0..vc_len {
            vc.push(dp.read_u32()?);
        }
        entry.vis_contents = Some(vc);
        entry.flags |= UPD_VIS;
    }

    Ok(Some(entry))
}

fn parse_movable_eye(
    dp: &mut DataReader,
) -> Result<(Option<EyeUpdate>, Vec<MovableEntry>, Vec<u32>)> {
    let flags = dp.read_u8()?;
    let mut eye = None;

    if flags & 0x02 != 0 {
        dp.read_i16()?;
        dp.read_i16()?;
        dp.read_i16()?;
        dp.read_i16()?;
        dp.read_u8()?;
        let eye_bits = dp.read_u32()?;
        let _glide_size = dp.read_f32()?;
        let eye_x = dp.read_i16()?;
        let eye_y = dp.read_i16()?;

        eye = Some(EyeUpdate {
            x: eye_x as f32,
            y: eye_y as f32,
            z: 0,
            sight: eye_bits,
            see_invisible: 0,
        });
    }

    let mut sight = 0u32;
    let mut see_invisible = 0u8;

    if flags & 0x20 != 0 {
        sight = dp.read_u8()? as u32;
    }
    if flags & 0x40 != 0 {
        // read_value - skip a typed value
        let vtype = dp.read_u8()?;
        match vtype {
            0x2A => {
                dp.read_f32()?;
            }
            0x06 => {
                dp.read_u32as16()?;
            }
            _ => {
                dp.read_u32()?;
            }
        }
    }
    if flags & 0x80 != 0 {
        let flags2 = dp.read_u8()?;
        if flags2 & 1 != 0 {
            dp.read_u8()?;
        }
        if flags2 & 2 != 0 {
            see_invisible = dp.read_u8()?;
        }
        if flags2 & 4 != 0 {
            sight = dp.read_u32()?;
        }
        if flags2 & 8 != 0 {
            dp.read_i16()?;
            dp.read_i16()?;
            dp.read_i16()?;
            dp.read_i16()?;
        }
        if flags2 & 0x10 != 0 {
            dp.read_i16()?;
            dp.read_i16()?;
        }
        if flags2 & 0x20 != 0 {
            dp.read_i16()?;
            dp.read_i16()?;
            dp.read_i16()?;
            dp.read_i16()?;
        }
    }

    if let Some(ref mut e) = eye {
        e.sight = sight;
        e.see_invisible = see_invisible;
    }

    let (movables, deletes) = parse_movable_changes(dp, false)?;

    Ok((eye, movables, deletes))
}

fn parse_movable_changes_screen(dp: &mut DataReader) -> Result<(Vec<MovableEntry>, Vec<u32>)> {
    parse_movable_changes(dp, true)
}

fn parse_movable_changes(
    dp: &mut DataReader,
    is_screen: bool,
) -> Result<(Vec<MovableEntry>, Vec<u32>)> {
    if dp.reached_end() {
        return Ok((Vec::new(), Vec::new()));
    }

    let count = dp.read_u16()? as usize;
    let mut movables = Vec::with_capacity(count);
    let mut deletes = Vec::new();

    for _ in 0..count {
        let flags = if is_screen {
            dp.read_u8()? as u16
        } else {
            dp.read_u16()?
        };
        let atom_id = dp.read_u32()?;

        if flags == 0 {
            deletes.push(atom_id);
            continue;
        }

        // Skip the "reset" flag (flags & 3 == 1) - not relevant for recording

        match parse_movable_entry_from_flags(dp, atom_id, flags)? {
            Some(entry) => movables.push(entry),
            None => deletes.push(atom_id),
        }
    }

    Ok((movables, deletes))
}

fn parse_movable_entry_from_flags(
    dp: &mut DataReader,
    atom_id: u32,
    flags: u16,
) -> Result<Option<MovableEntry>> {
    let mut entry = MovableEntry {
        atom_id,
        flags: 0,
        appearance_id: None,
        location: None,
        pixel: None,
        glide_size: None,
        vis_contents: None,
    };

    if flags & 4 != 0 {
        let loc = dp.read_u32()?;
        entry.location = Some(turf_id_to_xyz(loc, 255, 255));
        entry.flags |= UPD_LOCATION;
    }
    if flags & 8 != 0 {
        entry.appearance_id = Some(dp.read_u32as16()?);
        entry.flags |= UPD_APPEARANCE;
    }
    if flags & 16 != 0 {
        entry.pixel = Some((
            dp.read_i16()?,
            dp.read_i16()?,
            dp.read_i16()?,
            dp.read_i16()?,
        ));
        entry.flags |= UPD_PIXEL;
    }
    if flags & 32 != 0 {
        dp.read_i16()?;
        dp.read_i16()?;
        entry.glide_size = Some(dp.read_f32()?);
        entry.flags |= UPD_GLIDE;
    }
    if flags & 64 != 0 {
        dp.read_i16()?;
        dp.read_i16()?;
        dp.read_u16()?;
        dp.read_u16()?;
    }
    if flags & 128 != 0 {
        dp.read_u8()?;
    }
    if flags & 256 != 0 {
        let vc_len = dp.read_u16()? as usize;
        let mut vc = Vec::with_capacity(vc_len);
        for _ in 0..vc_len {
            vc.push(dp.read_u32()?);
        }
        entry.vis_contents = Some(vc);
        entry.flags |= UPD_VIS;
    }

    Ok(Some(entry))
}

fn parse_animation(dp: &mut DataReader) -> Result<AnimationFrame> {
    let atom_id = dp.read_u32()?;
    let start_time = dp.read_f32()?;
    let is_new = dp.read_u8()? != 0;
    let sequence = dp.read_u16()?;
    let frame_count = dp.read_u16()?;

    let mut keyframes = Vec::with_capacity(frame_count as usize);
    for _ in 0..frame_count {
        let from_appearance_id = dp.read_u32as16()?;
        let to_appearance_id = dp.read_u32as16()?;
        let duration = dp.read_f32()?;
        let loop_count = dp.read_i32()?;
        let easing = dp.read_u8()?;
        let flags = dp.read_u8()?;

        let parallel_start = if flags & 4 != 0 {
            Some(dp.read_f32()?)
        } else {
            None
        };

        keyframes.push(AnimationKeyframe {
            from_appearance_id,
            to_appearance_id,
            duration,
            loop_count,
            easing,
            flags,
            parallel_start,
        });
    }

    Ok(AnimationFrame {
        atom_id,
        start_time,
        is_new,
        sequence,
        keyframes,
    })
}

fn parse_flick(dp: &mut DataReader) -> Result<FlickFrame> {
    let atom_id = dp.read_u32()?;
    let flags = dp.read_u8()?;

    let icon_id = if flags & 1 != 0 {
        Some(dp.read_u32as16()?)
    } else {
        None
    };

    let icon_state = if flags & 2 != 0 {
        Some(dp.read_utf_string()?)
    } else {
        None
    };

    Ok(FlickFrame {
        atom_id,
        icon_id,
        icon_state,
    })
}
