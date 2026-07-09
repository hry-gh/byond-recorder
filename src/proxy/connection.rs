use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::Result;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tracing::{debug, info, warn};

use crate::protocol::framing::{FrameReader, RawMessage};
use crate::protocol::handshake;
use crate::protocol::messages;
use crate::recorder::RoundRecorder;

enum ConnectionPhase {
    WaitingForClientHandshake,
    WaitingForServerHandshake { client_handshake_payload: Vec<u8> },
    Established,
}

pub async fn handle(
    mut client: TcpStream,
    client_addr: SocketAddr,
    upstream_addr: SocketAddr,
    recorder: Arc<RoundRecorder>,
) -> Result<()> {
    let mut server = TcpStream::connect(upstream_addr).await?;
    info!("{}: connected to upstream", client_addr);

    let mut client_buf = vec![0u8; 65536];
    let mut server_buf = vec![0u8; 65536];

    let mut phase = ConnectionPhase::WaitingForClientHandshake;
    let mut server_frame_reader = FrameReader::new();
    let mut client_frame_reader = FrameReader::new();
    let mut session_id: Option<String> = None;
    let mut session_started = false;

    loop {
        tokio::select! {
            result = client.read(&mut client_buf) => {
                let n = result?;
                if n == 0 {
                    info!("{}: client disconnected", client_addr);
                    break;
                }

                if let ConnectionPhase::WaitingForClientHandshake = &phase {
                    if n >= 4 {
                        let payload = client_buf[4..n].to_vec();
                        debug!("{}: client handshake raw ({} bytes): {:02x?}", client_addr, n, &client_buf[..n.min(64)]);
                        phase = ConnectionPhase::WaitingForServerHandshake {
                            client_handshake_payload: payload,
                        };
                    }
                } else if let ConnectionPhase::Established = &phase {
                    client_frame_reader.push_data(&client_buf[..n]);

                    match client_frame_reader.read_messages() {
                        Ok(msgs) => {
                            for msg in &msgs {
                                debug!("{}: client msg type={} len={}", client_addr, msg.msg_type, msg.payload.len());
                                if msg.msg_type == messages::MSG_KEY {
                                    debug!("{}: msg 26 payload ({} bytes): {:02x?}", client_addr, msg.payload.len(), &msg.payload[..msg.payload.len().min(80)]);
                                    let key = extract_null_terminated_string(&msg.payload);
                                    if !key.is_empty() && key != "-" {
                                        let ckey = byond_key_to_ckey(&key);
                                        info!("{}: identified player key: {} (ckey: {})", client_addr, key, ckey);
                                        session_id = Some(ckey);
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            warn!("{}: client frame parse error: {:#}", client_addr, e);
                        }
                    }
                }

                server.write_all(&client_buf[..n]).await?;
            }
            result = server.read(&mut server_buf) => {
                let n = result?;
                if n == 0 {
                    info!("{}: server disconnected", client_addr);
                    break;
                }

                match &phase {
                    ConnectionPhase::WaitingForServerHandshake { client_handshake_payload } => {
                        if n >= 4 {
                            let server_payload = server_buf[4..n].to_vec();
                            debug!("{}: server handshake raw ({} bytes): {:02x?}", client_addr, n, &server_buf[..n.min(64)]);

                            match handshake::derive_cipher_key(client_handshake_payload, &server_payload) {
                                Ok(result) => {
                                    info!(
                                        "{}: handshake complete, BYOND version {}, cipher key derived",
                                        client_addr, result.client_version
                                    );
                                    client_frame_reader.set_cipher(result.cipher.clone(), true);
                                    server_frame_reader.set_cipher(result.cipher, false);
                                    phase = ConnectionPhase::Established;
                                }
                                Err(e) => {
                                    warn!("{}: handshake parse failed: {:#}, continuing without decryption", client_addr, e);
                                    phase = ConnectionPhase::Established;
                                }
                            }
                        }
                    }
                    ConnectionPhase::Established => {
                        server_frame_reader.push_data(&server_buf[..n]);

                        match server_frame_reader.read_messages() {
                            Ok(raw_messages) => {
                                if !raw_messages.is_empty() {
                                    debug!(
                                        "{}: parsed {} server messages from {} bytes",
                                        client_addr, raw_messages.len(), n
                                    );
                                }
                                for msg in &raw_messages {
                                    if !session_started {
                                        if let Some(ref ckey) = session_id {
                                            match recorder.start_session(ckey) {
                                                Ok(()) => {
                                                    session_started = true;
                                                    info!("{}: recording session started for {}", client_addr, ckey);
                                                }
                                                Err(e) => warn!("{}: failed to start session: {:#}", client_addr, e),
                                            }
                                        }
                                    }

                                    if session_started {
                                        if let Some(ref ckey) = session_id {
                                            if messages::is_visual_message(msg.msg_type) {
                                                if let Err(e) = recorder.record_message(ckey, msg) {
                                                    warn!("{}: recording error: {:#}", client_addr, e);
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                warn!("{}: frame parse error: {:#}", client_addr, e);
                            }
                        }
                    }
                    _ => {}
                }

                client.write_all(&server_buf[..n]).await?;
            }
        }
    }

    if session_started {
        if let Some(ref ckey) = session_id {
            recorder.end_session(ckey)?;
        }
    }

    info!("{}: connection closed", client_addr);
    Ok(())
}

fn extract_null_terminated_string(data: &[u8]) -> String {
    let end = data.iter().position(|&b| b == 0).unwrap_or(data.len());
    String::from_utf8_lossy(&data[..end]).into_owned()
}

fn byond_key_to_ckey(key: &str) -> String {
    key.chars()
        .filter(|c| c.is_alphanumeric())
        .map(|c| c.to_ascii_lowercase())
        .collect()
}
