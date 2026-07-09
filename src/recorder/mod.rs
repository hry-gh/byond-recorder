use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::io::{BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Result, Context};
use tracing::{info, warn, debug};

use crate::format::appearance::Appearance;
use crate::format::replay::{Frame, ReplayHeader};
use crate::format::rsc;
use crate::format::writer::{AppearanceWriter, ReplayWriter};
use crate::protocol::deserialize::{self, DeserializedEvent};
use crate::protocol::framing::RawMessage;

struct SessionState {
    writer: ReplayWriter<BufWriter<File>>,
    maxx: u16,
    maxy: u16,
}

pub struct RoundRecorder {
    base_dir: PathBuf,
    game_dir: PathBuf,
    round_id: Mutex<String>,
    dir: Mutex<PathBuf>,
    icon_map: Mutex<HashMap<u32, String>>,
    appearances: Mutex<Option<AppearancesState>>,
    sessions: Mutex<HashMap<String, SessionState>>,
}

struct AppearancesState {
    writer: AppearanceWriter<BufWriter<File>>,
    seen: HashSet<u32>,
}

impl RoundRecorder {
    pub fn new(base_dir: &Path, game_dir: &Path, round_id: &str) -> Result<Self> {
        let dir = base_dir.join(round_id);
        fs::create_dir_all(&dir)
            .with_context(|| format!("creating recording directory: {}", dir.display()))?;

        let appearances = Self::create_appearances_writer(&dir)?;

        let icon_map = Self::load_icon_map(game_dir)?;

        info!("recording to {}", dir.display());

        Ok(Self {
            base_dir: base_dir.to_path_buf(),
            game_dir: game_dir.to_path_buf(),
            round_id: Mutex::new(round_id.to_string()),
            dir: Mutex::new(dir),
            icon_map: Mutex::new(icon_map),
            appearances: Mutex::new(Some(appearances)),
            sessions: Mutex::new(HashMap::new()),
        })
    }

    fn load_icon_map(game_dir: &Path) -> Result<HashMap<u32, String>> {
        let mut rsc_path = None;

        for entry in fs::read_dir(game_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("rsc") {
                rsc_path = Some(path);
                break;
            }
        }

        match rsc_path {
            Some(path) => {
                let map = rsc::parse_rsc(&path)?;
                info!("loaded {} resources from {}", map.len(), path.display());
                Ok(map)
            }
            None => {
                warn!("no .rsc file found in {}", game_dir.display());
                Ok(HashMap::new())
            }
        }
    }

    fn create_appearances_writer(dir: &Path) -> Result<AppearancesState> {
        let appearances_path = dir.join("appearances.dat");
        let appearances_file = BufWriter::new(
            File::create(&appearances_path)
                .with_context(|| format!("creating {}", appearances_path.display()))?,
        );
        let writer = AppearanceWriter::new(appearances_file)?;
        Ok(AppearancesState {
            writer,
            seen: HashSet::new(),
        })
    }

    pub fn write_icon_map(&self) -> Result<()> {
        let dir = self.dir.lock().unwrap().clone();
        let map = self.icon_map.lock().unwrap();
        let map_path = dir.join("icon_map.json");
        let json = serde_json::to_string_pretty(&*map)?;
        fs::write(&map_path, json)?;
        info!("wrote icon map ({} entries) to {}", map.len(), map_path.display());
        Ok(())
    }

    pub fn set_round_id(&self, round_id: &str) {
        let new_dir = self.base_dir.join(round_id);
        if let Err(e) = fs::create_dir_all(&new_dir) {
            warn!("failed to create directory for round {}: {:#}", round_id, e);
            return;
        }

        match Self::create_appearances_writer(&new_dir) {
            Ok(new_appearances) => {
                let mut appearances = self.appearances.lock().unwrap();
                *appearances = Some(new_appearances);
            }
            Err(e) => {
                warn!("failed to create appearances writer for round {}: {:#}", round_id, e);
                return;
            }
        }

        if let Ok(new_map) = Self::load_icon_map(&self.game_dir) {
            *self.icon_map.lock().unwrap() = new_map;
        }

        *self.round_id.lock().unwrap() = round_id.to_string();
        *self.dir.lock().unwrap() = new_dir.clone();
        info!("round switched to {}, recording to {}", round_id, new_dir.display());
    }

    pub fn record_appearance(&self, appearance: &Appearance) -> Result<()> {
        let mut state = self.appearances.lock().unwrap();
        if let Some(ref mut state) = *state {
            if state.seen.contains(&appearance.id) {
                return Ok(());
            }
            state.seen.insert(appearance.id);
            state.writer.write_appearance(appearance)?;
            debug!("recorded new appearance {}", appearance.id);
        }
        Ok(())
    }

    pub fn start_session(&self, session_id: &str) -> Result<()> {
        let dir = self.dir.lock().unwrap().clone();
        let round_id = self.round_id.lock().unwrap().clone();

        let replay_path = dir.join(format!("{}.replay", session_id));
        let file = BufWriter::new(
            File::create(&replay_path)
                .with_context(|| format!("creating {}", replay_path.display()))?,
        );

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        let header = ReplayHeader {
            byond_major: 516,
            byond_minor: 1680,
            round_id,
            ckey: session_id.to_string(),
            start_time: now,
            maxx: 0,
            maxy: 0,
            maxz: 0,
        };

        let writer = ReplayWriter::new(file, &header)?;

        let mut sessions = self.sessions.lock().unwrap();
        sessions.insert(session_id.to_string(), SessionState { writer, maxx: 0, maxy: 0 });

        info!("started recording session: {}", session_id);
        Ok(())
    }

    pub fn record_message(&self, session_id: &str, msg: &RawMessage) -> Result<()> {
        let (maxx, maxy) = {
            let sessions = self.sessions.lock().unwrap();
            match sessions.get(session_id) {
                Some(s) => (s.maxx, s.maxy),
                None => return Ok(()),
            }
        };

        let events = deserialize::deserialize_message(msg.msg_type, &msg.payload, maxx, maxy)
            .with_context(|| format!("deserializing msg type 0x{:02X}", msg.msg_type))?;

        for event in &events {
            match event {
                DeserializedEvent::Appearance(appearance) => {
                    self.record_appearance(appearance)?;
                }
                DeserializedEvent::Frame(frame) => {
                    if let Frame::MapResize { maxx: mx, maxy: my, .. } = frame {
                        let mut sessions = self.sessions.lock().unwrap();
                        if let Some(session) = sessions.get_mut(session_id) {
                            session.maxx = *mx;
                            session.maxy = *my;
                        }
                    }

                    let mut sessions = self.sessions.lock().unwrap();
                    if let Some(session) = sessions.get_mut(session_id) {
                        session.writer.write_frame(frame)?;
                    }
                }
            }
        }

        Ok(())
    }

    pub fn end_session(&self, session_id: &str) -> Result<()> {
        let mut sessions = self.sessions.lock().unwrap();
        if let Some(mut session) = sessions.remove(session_id) {
            session.writer.write_eof()?;
            session.writer.flush()?;
            info!("ended recording session: {}", session_id);
        }
        Ok(())
    }

    pub fn end_round(&self) -> Result<()> {
        let mut sessions = self.sessions.lock().unwrap();
        for (ckey, mut session) in sessions.drain() {
            if let Err(e) = session.writer.write_eof() {
                warn!("failed to write EOF for {}: {:#}", ckey, e);
            }
            if let Err(e) = session.writer.flush() {
                warn!("failed to flush {}: {:#}", ckey, e);
            }
        }
        drop(sessions);

        {
            let mut appearances = self.appearances.lock().unwrap();
            if let Some(ref mut state) = *appearances {
                state.writer.flush()?;
            }
            *appearances = None;
        }

        self.write_icon_map()?;

        let dir = self.dir.lock().unwrap().clone();
        self.compress_round(&dir)?;

        info!("round ended, files compressed in {}", dir.display());
        Ok(())
    }

    fn compress_round(&self, dir: &Path) -> Result<()> {
        let entries: Vec<_> = fs::read_dir(dir)?
            .filter_map(|e| e.ok())
            .filter(|e| {
                let path = e.path();
                let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
                ext == "replay" || path.file_name().map_or(false, |n| n == "appearances.dat")
            })
            .collect();

        for entry in entries {
            let path = entry.path();
            let zst_path = path.with_extension(
                format!("{}.zst", path.extension().and_then(|e| e.to_str()).unwrap_or(""))
            );

            let mut input = File::open(&path)?;
            let output = File::create(&zst_path)?;
            let mut encoder = zstd::Encoder::new(output, 3)?;

            let mut buf = vec![0u8; 65536];
            loop {
                let n = input.read(&mut buf)?;
                if n == 0 { break; }
                encoder.write_all(&buf[..n])?;
            }
            encoder.finish()?;

            fs::remove_file(&path)?;
            debug!("compressed {} -> {}", path.display(), zst_path.display());
        }

        Ok(())
    }

    pub fn flush_all(&self) -> Result<()> {
        let mut appearances = self.appearances.lock().unwrap();
        if let Some(ref mut state) = *appearances {
            state.writer.flush()?;
        }

        let mut sessions = self.sessions.lock().unwrap();
        for (_, session) in sessions.iter_mut() {
            session.writer.flush()?;
        }
        Ok(())
    }
}
