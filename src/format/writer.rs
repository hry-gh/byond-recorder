use std::io::{self, Write};
use std::time::Instant;

use byteorder::{LittleEndian, WriteBytesExt};

use super::appearance::{Appearance, APPEARANCE_MAGIC, APPEARANCE_VERSION};
use super::replay::*;

fn write_string<W: Write>(w: &mut W, s: &str) -> io::Result<()> {
    let bytes = s.as_bytes();
    w.write_u16::<LittleEndian>(bytes.len() as u16)?;
    w.write_all(bytes)
}

pub struct AppearanceWriter<W: Write> {
    inner: W,
}

impl<W: Write> AppearanceWriter<W> {
    pub fn new(mut inner: W) -> io::Result<Self> {
        inner.write_all(APPEARANCE_MAGIC)?;
        inner.write_u8(APPEARANCE_VERSION)?;
        Ok(Self { inner })
    }

    pub fn write_appearance(&mut self, appearance: &Appearance) -> io::Result<()> {
        let data = appearance.serialize_data();
        self.inner
            .write_u32::<LittleEndian>(appearance.id)?;
        self.inner
            .write_u32::<LittleEndian>(data.len() as u32)?;
        self.inner.write_all(&data)
    }

    pub fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

pub struct ReplayWriter<W: Write> {
    inner: W,
    last_frame: Instant,
}

impl<W: Write> ReplayWriter<W> {
    pub fn new(mut inner: W, header: &ReplayHeader) -> io::Result<Self> {
        inner.write_all(REPLAY_MAGIC)?;
        inner.write_u8(REPLAY_VERSION)?;
        inner.write_u16::<LittleEndian>(header.byond_major)?;
        inner.write_u16::<LittleEndian>(header.byond_minor)?;
        write_string(&mut inner, &header.round_id)?;
        write_string(&mut inner, &header.ckey)?;
        inner.write_u64::<LittleEndian>(header.start_time)?;
        inner.write_u16::<LittleEndian>(header.maxx)?;
        inner.write_u16::<LittleEndian>(header.maxy)?;
        inner.write_u16::<LittleEndian>(header.maxz)?;

        Ok(Self {
            inner,
            last_frame: Instant::now(),
        })
    }

    fn write_frame_header(&mut self, frame_type: FrameType, data: &[u8]) -> io::Result<()> {
        let now = Instant::now();
        let delta = now.duration_since(self.last_frame).as_millis() as u32;
        self.last_frame = now;

        self.inner.write_u32::<LittleEndian>(delta)?;
        self.inner.write_u8(frame_type as u8)?;
        self.inner
            .write_u32::<LittleEndian>(data.len() as u32)?;
        self.inner.write_all(data)
    }

    pub fn write_frame(&mut self, frame: &Frame) -> io::Result<()> {
        match frame {
            Frame::MapResize { maxx, maxy, maxz } => {
                let mut buf = Vec::with_capacity(6);
                buf.write_u16::<LittleEndian>(*maxx)?;
                buf.write_u16::<LittleEndian>(*maxy)?;
                buf.write_u16::<LittleEndian>(*maxz)?;
                self.write_frame_header(FrameType::MapResize, &buf)
            }
            Frame::TurfUpdate(entries) => {
                let mut buf = Vec::with_capacity(2 + entries.len() * 10);
                buf.write_u16::<LittleEndian>(entries.len() as u16)?;
                for e in entries {
                    buf.write_u16::<LittleEndian>(e.x)?;
                    buf.write_u16::<LittleEndian>(e.y)?;
                    buf.write_u16::<LittleEndian>(e.z)?;
                    buf.write_u32::<LittleEndian>(e.appearance_id)?;
                }
                self.write_frame_header(FrameType::TurfUpdate, &buf)
            }
            Frame::MovableUpdate(entries) => {
                let mut buf = Vec::with_capacity(2 + entries.len() * 32);
                buf.write_u16::<LittleEndian>(entries.len() as u16)?;
                for e in entries {
                    buf.write_u32::<LittleEndian>(e.atom_id)?;
                    buf.write_u8(e.flags)?;
                    if let Some(app_id) = e.appearance_id {
                        buf.write_u32::<LittleEndian>(app_id)?;
                    }
                    if let Some((x, y, z)) = e.location {
                        buf.write_u16::<LittleEndian>(x)?;
                        buf.write_u16::<LittleEndian>(y)?;
                        buf.write_u16::<LittleEndian>(z)?;
                    }
                    if let Some((px, py, pw, pz)) = e.pixel {
                        buf.write_i16::<LittleEndian>(px)?;
                        buf.write_i16::<LittleEndian>(py)?;
                        buf.write_i16::<LittleEndian>(pw)?;
                        buf.write_i16::<LittleEndian>(pz)?;
                    }
                    if let Some(gs) = e.glide_size {
                        buf.write_f32::<LittleEndian>(gs)?;
                    }
                    if let Some(ref vc) = e.vis_contents {
                        buf.write_u16::<LittleEndian>(vc.len() as u16)?;
                        for id in vc {
                            buf.write_u32::<LittleEndian>(*id)?;
                        }
                    }
                }
                self.write_frame_header(FrameType::MovableUpdate, &buf)
            }
            Frame::MovableDelete(atom_id) => {
                let mut buf = Vec::with_capacity(4);
                buf.write_u32::<LittleEndian>(*atom_id)?;
                self.write_frame_header(FrameType::MovableDelete, &buf)
            }
            Frame::Animation(anim) => {
                let mut buf = Vec::with_capacity(64);
                buf.write_u32::<LittleEndian>(anim.atom_id)?;
                buf.write_f32::<LittleEndian>(anim.start_time)?;
                buf.write_u8(anim.is_new as u8)?;
                buf.write_u16::<LittleEndian>(anim.sequence)?;
                buf.write_u16::<LittleEndian>(anim.keyframes.len() as u16)?;
                for kf in &anim.keyframes {
                    buf.write_u32::<LittleEndian>(kf.from_appearance_id)?;
                    buf.write_u32::<LittleEndian>(kf.to_appearance_id)?;
                    buf.write_f32::<LittleEndian>(kf.duration)?;
                    buf.write_i32::<LittleEndian>(kf.loop_count)?;
                    buf.write_u8(kf.easing)?;
                    buf.write_u8(kf.flags)?;
                    if let Some(ps) = kf.parallel_start {
                        buf.write_f32::<LittleEndian>(ps)?;
                    }
                }
                self.write_frame_header(FrameType::Animation, &buf)
            }
            Frame::Flick(flick) => {
                let mut buf = Vec::with_capacity(32);
                buf.write_u32::<LittleEndian>(flick.atom_id)?;
                let mut flags: u8 = 0;
                if flick.icon_id.is_some() {
                    flags |= 0x01;
                }
                if flick.icon_state.is_some() {
                    flags |= 0x02;
                }
                buf.write_u8(flags)?;
                if let Some(id) = flick.icon_id {
                    buf.write_u32::<LittleEndian>(id)?;
                }
                if let Some(ref state) = flick.icon_state {
                    write_string(&mut buf, state)?;
                }
                self.write_frame_header(FrameType::Flick, &buf)
            }
            Frame::IconMeta(icon) => {
                let mut buf = Vec::with_capacity(128);
                buf.write_u32::<LittleEndian>(icon.icon_id)?;
                buf.write_u32::<LittleEndian>(icon.resource_id)?;
                buf.write_u16::<LittleEndian>(icon.width)?;
                buf.write_u16::<LittleEndian>(icon.height)?;
                buf.write_u16::<LittleEndian>(icon.states.len() as u16)?;
                for state in &icon.states {
                    write_string(&mut buf, &state.name)?;
                    buf.write_u8(state.flags)?;
                    buf.write_u8(state.num_dirs)?;
                    buf.write_u16::<LittleEndian>(state.num_frames)?;
                    buf.write_u16::<LittleEndian>(state.loop_count)?;
                    if let Some(ref delays) = state.delays {
                        for d in delays {
                            buf.write_f32::<LittleEndian>(*d)?;
                        }
                    }
                    for dir_frames in &state.frame_indices {
                        for idx in dir_frames {
                            buf.write_u16::<LittleEndian>(*idx)?;
                        }
                    }
                }
                self.write_frame_header(FrameType::IconMeta, &buf)
            }
            Frame::EyeUpdate(eye) => {
                let mut buf = Vec::with_capacity(11);
                buf.write_f32::<LittleEndian>(eye.x)?;
                buf.write_f32::<LittleEndian>(eye.y)?;
                buf.write_i16::<LittleEndian>(eye.z)?;
                buf.write_u32::<LittleEndian>(eye.sight)?;
                buf.write_u8(eye.see_invisible)?;
                self.write_frame_header(FrameType::EyeUpdate, &buf)
            }
        }
    }

    pub fn write_eof(&mut self) -> io::Result<()> {
        self.inner.write_u32::<LittleEndian>(EOF_MARKER)
    }

    pub fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}
