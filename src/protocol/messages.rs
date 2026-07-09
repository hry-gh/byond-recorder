/// BYOND message type constants (server → client).
///
/// These are the message types we care about for replay recording.
/// All other types are ignored by the recorder.

pub const MSG_VERSION: u16 = 1;
pub const MSG_KEY: u16 = 26;
pub const MSG_FLICK: u16 = 108;
pub const MSG_MOVABLE_SMALL: u16 = 118;
pub const MSG_ICON: u16 = 207;
pub const MSG_ANIMATION: u16 = 240;
pub const MSG_BROWSE: u16 = 243;
pub const MSG_MAP_CONFIG: u16 = 248;
pub const MSG_TURF_BLOCK: u16 = 249;
pub const MSG_TURF_PARTIAL: u16 = 250;
pub const MSG_MOVABLE_EYE: u16 = 251;
pub const MSG_APPEARANCE: u16 = 247;

pub fn is_visual_message(msg_type: u16) -> bool {
    matches!(
        msg_type,
        MSG_APPEARANCE
            | MSG_TURF_BLOCK
            | MSG_TURF_PARTIAL
            | MSG_MOVABLE_SMALL
            | MSG_MOVABLE_EYE
            | MSG_ICON
            | MSG_ANIMATION
            | MSG_FLICK
            | MSG_MAP_CONFIG
    )
}
