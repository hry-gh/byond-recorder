# BYOND Replay Format Specification

**Version:** 1 (draft)

This document specifies the binary format used by the BYOND replay recorder and viewer. The format is designed to capture per-client visual state from a running DreamDaemon instance for later playback.

## Overview

A recording for a single round consists of the following files:

```
recordings/{round_id}/
    appearances.dat          Shared appearance definition table
    icon_map.json            Resource ID to .dmi file path mapping
    icons/                   .dmi sprite files keyed by resource ID
        {resource_id}.dmi
        ...
    {ckey}.replay            Per-client visual state stream
    {ckey}.replay            ...
```

## Conventions

All multi-byte integers are **little-endian** unless otherwise noted.

### Primitive Types

| Type     | Size    | Description                                  |
|----------|---------|----------------------------------------------|
| `u8`     | 1 byte  | Unsigned 8-bit integer                       |
| `u16`    | 2 bytes | Unsigned 16-bit integer, little-endian       |
| `u32`    | 4 bytes | Unsigned 32-bit integer, little-endian       |
| `i16`    | 2 bytes | Signed 16-bit integer, little-endian         |
| `i32`    | 4 bytes | Signed 32-bit integer, little-endian         |
| `f32`    | 4 bytes | IEEE 754 single-precision float              |
| `u64`    | 8 bytes | Unsigned 64-bit integer, little-endian       |
| `string` | varies  | Length-prefixed UTF-8: `u16 length` followed by `[u8; length]`. No null terminator. |

---

## `appearances.dat`

Shared appearance definitions referenced by all `.replay` files in the same round. Appearances are deduplicated: each unique appearance ID appears exactly once.

### Header

| Field     | Type      | Value  | Description                    |
|-----------|-----------|--------|--------------------------------|
| `magic`   | `[u8; 4]` | `BYAP` | File identifier                |
| `version` | `u8`      | `1`    | Format version                 |

### Entries

Entries are appended sequentially after the header. Read until EOF.

| Field            | Type   | Description                          |
|------------------|--------|--------------------------------------|
| `appearance_id`  | `u32`  | Unique appearance identifier         |
| `data_length`    | `u32`  | Length of the following data blob    |
| `data`           | `[u8; data_length]` | Serialized appearance (see below) |

### Appearance Data

Each appearance blob contains the following fields in order:

| Field              | Type     | Description                              |
|--------------------|----------|------------------------------------------|
| `icon_id`          | `u32`    | References an icon in the icon table     |
| `icon_state`       | `string` | Icon state name                          |
| `name`             | `string` | Appearance name                          |
| `dir`              | `u8`     | Direction (BYOND dir constants)          |
| `pixel_x`          | `i16`    | Pixel offset X                           |
| `pixel_y`          | `i16`    | Pixel offset Y                           |
| `pixel_w`          | `i16`    | Pixel offset W                           |
| `pixel_z`          | `i16`    | Pixel offset Z                           |
| `layer`            | `f32`    | Render layer                             |
| `plane`            | `i16`    | Render plane                             |
| `alpha`            | `u8`     | Opacity (0-255)                          |
| `color_r`          | `u8`     | Red channel                              |
| `color_g`          | `u8`     | Green channel                            |
| `color_b`          | `u8`     | Blue channel                             |
| `blend_mode`       | `u8`     | Blend mode                               |
| `appearance_flags` | `u32`    | BYOND appearance flags                   |
| `glide_size`       | `f32`    | Glide size for movement interpolation    |
| `invisibility`     | `u8`     | Invisibility level                       |
| `mouse_opacity`    | `u8`     | Mouse opacity mode                       |
| `presence`         | `u16`    | Bitfield indicating optional fields      |

#### Presence Bitfield

| Bit  | Constant           | Description                       |
|------|--------------------|-----------------------------------|
| 0x01 | `HAS_TRANSFORM`    | 6-element transform matrix        |
| 0x02 | `HAS_COLOR_MATRIX` | 20-element color matrix           |
| 0x04 | `HAS_OVERLAYS`     | Overlay appearance list           |
| 0x08 | `HAS_UNDERLAYS`    | Underlay appearance list          |
| 0x10 | `HAS_SCREEN_LOC`   | Screen location string            |

#### Optional Fields (in order of presence bits, lowest first)

**HAS_TRANSFORM (0x01):**

| Field       | Type       | Description                |
|-------------|------------|----------------------------|
| `transform` | `[f32; 6]` | Affine transform matrix: `[a, b, c, d, e, f]` |

**HAS_COLOR_MATRIX (0x02):**

| Field          | Type        | Description                |
|----------------|-------------|----------------------------|
| `color_matrix` | `[f32; 20]` | 5x4 color transformation matrix |

**HAS_OVERLAYS (0x04):**

| Field           | Type              | Description                       |
|-----------------|-------------------|-----------------------------------|
| `overlay_count` | `u16`             | Number of overlay appearance IDs  |
| `overlay_ids`   | `[u32; count]`    | Appearance IDs of overlays        |

**HAS_UNDERLAYS (0x08):**

| Field            | Type              | Description                        |
|------------------|-------------------|------------------------------------|
| `underlay_count` | `u16`             | Number of underlay appearance IDs  |
| `underlay_ids`   | `[u32; count]`    | Appearance IDs of underlays        |

**HAS_SCREEN_LOC (0x10):**

| Field        | Type     | Description                  |
|--------------|----------|------------------------------|
| `screen_loc` | `string` | Screen location descriptor   |

---

## `{ckey}.replay`

Per-client visual state stream. Contains a header followed by a sequence of timestamped frames.

### Header

| Field          | Type       | Value    | Description                          |
|----------------|------------|----------|--------------------------------------|
| `magic`        | `[u8; 4]`  | `BRCR`   | File identifier                      |
| `version`      | `u8`       | `1`      | Format version                       |
| `byond_major`  | `u16`      |          | BYOND major version (e.g. 516)       |
| `byond_minor`  | `u16`      |          | BYOND minor version (e.g. 1680)      |
| `round_id`     | `string`   |          | Round identifier                     |
| `ckey`         | `string`   |          | Player ckey                          |
| `start_time`   | `u64`      |          | Recording start time (Unix ms)       |
| `maxx`         | `u16`      |          | Initial map width                    |
| `maxy`         | `u16`      |          | Initial map height                   |
| `maxz`         | `u16`      |          | Initial map depth                    |

### Frames

Frames are read sequentially until the EOF marker.

| Field         | Type   | Description                                          |
|---------------|--------|------------------------------------------------------|
| `delta_ms`    | `u32`  | Milliseconds since previous frame. `0xFFFFFFFF` = EOF |
| `frame_type`  | `u8`   | Type of frame (see below)                            |
| `data_length` | `u32`  | Length of frame data                                 |
| `data`        | `[u8; data_length]` | Frame payload                           |

### Frame Types

#### `0x01` - MAP_RESIZE

Map dimensions changed.

| Field  | Type  | Description     |
|--------|-------|-----------------|
| `maxx` | `u16` | New map width   |
| `maxy` | `u16` | New map height  |
| `maxz` | `u16` | New map depth   |

#### `0x02` - TURF_UPDATE

Batch of turf appearance changes. Coordinates are absolute.

| Field   | Type  | Description              |
|---------|-------|--------------------------|
| `count` | `u16` | Number of turf updates   |

Followed by `count` entries:

| Field           | Type  | Description                            |
|-----------------|-------|----------------------------------------|
| `x`             | `u16` | Absolute X coordinate                  |
| `y`             | `u16` | Absolute Y coordinate                  |
| `z`             | `u16` | Absolute Z coordinate                  |
| `appearance_id` | `u32` | Appearance ID (references appearances.dat) |

#### `0x03` - MOVABLE_UPDATE

Batch of movable atom state changes.

| Field   | Type  | Description                |
|---------|-------|----------------------------|
| `count` | `u16` | Number of atom updates     |

Followed by `count` entries:

| Field     | Type  | Description                          |
|-----------|-------|--------------------------------------|
| `atom_id` | `u32` | Atom identifier                      |
| `flags`   | `u8`  | Bitfield indicating which fields follow |

**Update flags:**

| Bit  | Constant         | Description                    |
|------|------------------|--------------------------------|
| 0x01 | `UPD_APPEARANCE` | Appearance changed             |
| 0x02 | `UPD_LOCATION`   | Location changed               |
| 0x04 | `UPD_PIXEL`      | Pixel offsets changed          |
| 0x08 | `UPD_GLIDE`      | Glide size changed             |
| 0x10 | `UPD_VIS`        | Vis contents changed           |

**Optional fields (in flag order):**

`UPD_APPEARANCE (0x01):`

| Field           | Type  | Description                            |
|-----------------|-------|----------------------------------------|
| `appearance_id` | `u32` | Appearance ID (references appearances.dat) |

`UPD_LOCATION (0x02):`

| Field | Type  | Description          |
|-------|-------|----------------------|
| `x`   | `u16` | Absolute X coordinate |
| `y`   | `u16` | Absolute Y coordinate |
| `z`   | `u16` | Absolute Z coordinate |

`UPD_PIXEL (0x04):`

| Field     | Type  | Description    |
|-----------|-------|----------------|
| `pixel_x` | `i16` | Pixel offset X |
| `pixel_y` | `i16` | Pixel offset Y |
| `pixel_w` | `i16` | Pixel offset W |
| `pixel_z` | `i16` | Pixel offset Z |

`UPD_GLIDE (0x08):`

| Field       | Type  | Description |
|-------------|-------|-------------|
| `glide_size` | `f32` | Glide size  |

`UPD_VIS (0x10):`

| Field      | Type           | Description                     |
|------------|----------------|---------------------------------|
| `vc_count` | `u16`          | Number of vis_contents entries  |
| `atom_ids` | `[u32; count]` | Atom IDs in vis_contents        |

#### `0x04` - MOVABLE_DELETE

An atom is no longer visible / has been removed.

| Field     | Type  | Description    |
|-----------|-------|----------------|
| `atom_id` | `u32` | Atom to remove |

#### `0x05` - ANIMATION

Smooth animation applied to an atom.

| Field         | Type  | Description                          |
|---------------|-------|--------------------------------------|
| `atom_id`     | `u32` | Target atom                          |
| `start_time`  | `f32` | Animation start time (server time)   |
| `is_new`      | `u8`  | Whether this replaces existing anim  |
| `sequence`    | `u16` | Sequence ID for chaining             |
| `frame_count` | `u16` | Number of keyframes                  |

Followed by `frame_count` keyframes:

| Field                | Type  | Description                            |
|----------------------|-------|----------------------------------------|
| `from_appearance_id` | `u32` | Starting appearance                    |
| `to_appearance_id`   | `u32` | Target appearance                      |
| `duration`           | `f32` | Frame duration in server time units    |
| `loop`               | `i32` | Loop count (-1 = infinite, 0 = stop)   |
| `easing`             | `u8`  | Easing function                        |
| `flags`              | `u8`  | Animation flags (bit 2 = parallel)     |

If `flags & 0x04` (parallel animation):

| Field            | Type  | Description           |
|------------------|-------|-----------------------|
| `parallel_start` | `f32` | Parallel start offset |

#### `0x06` - FLICK

Temporary icon state override on an atom.

| Field     | Type  | Description                        |
|-----------|-------|------------------------------------|
| `atom_id` | `u32` | Target atom                        |
| `flags`   | `u8`  | Bitfield: 0x01 = has icon, 0x02 = has state |

If `flags & 0x01`:

| Field     | Type  | Description      |
|-----------|-------|------------------|
| `icon_id` | `u32` | Override icon ID |

If `flags & 0x02`:

| Field        | Type     | Description           |
|--------------|----------|-----------------------|
| `icon_state` | `string` | Override icon state   |

#### `0x07` - ICON_META

Icon metadata (dimensions, states, frame timing). The actual pixel data is in `icons/{resource_id}.dmi`.

| Field         | Type  | Description                          |
|---------------|-------|--------------------------------------|
| `icon_id`     | `u32` | Icon identifier (referenced by appearances) |
| `resource_id` | `u32` | Maps to `icons/{resource_id}.dmi`    |
| `width`       | `u16` | Icon width in pixels                 |
| `height`      | `u16` | Icon height in pixels                |
| `state_count` | `u16` | Number of icon states                |

Followed by `state_count` icon states:

| Field        | Type     | Description                    |
|--------------|----------|--------------------------------|
| `name`       | `string` | State name (empty = default)   |
| `flags`      | `u8`     | State flags (bit 0 = rewind)   |
| `num_dirs`   | `u8`     | Number of directions (1/4/8)   |
| `num_frames` | `u16`    | Number of animation frames     |
| `loop`       | `u16`    | Loop count                     |

If `num_frames > 1`:

| Field    | Type               | Description             |
|----------|--------------------|-------------------------|
| `delays` | `[f32; num_frames]` | Per-frame delay values  |

Then for each direction:

| Field         | Type                | Description                  |
|---------------|---------------------|------------------------------|
| `frame_indices` | `[u16; num_frames]` | Spritesheet frame indices  |

#### `0x08` - EYE_UPDATE

Client viewport/camera state change.

| Field            | Type  | Description                    |
|------------------|-------|--------------------------------|
| `x`              | `f32` | Eye X position                 |
| `y`              | `f32` | Eye Y position                 |
| `z`              | `i16` | Eye Z level                    |
| `sight`          | `u32` | Sight flags                    |
| `see_invisible`  | `u8`  | See invisible level            |

---

## `icon_map.json`

JSON object mapping resource IDs (as strings) to `.dmi` file paths relative to the game's source tree.

```json
{
    "1": "icons/mob/human.dmi",
    "2": "icons/obj/items.dmi",
    "3": "icons/turf/floors.dmi"
}
```

The recorder copies the referenced `.dmi` files into `icons/{resource_id}.dmi` at round start using this mapping.

---

## Reading a Recording

1. Load `appearances.dat` into an `id -> Appearance` lookup table.
2. Load `icon_map.json` and the `icons/` directory for sprite data.
3. Open `{ckey}.replay`, read the header, initialize map dimensions.
4. Process frames sequentially:
   - Apply `delta_ms` timing for playback speed.
   - `ICON_META` frames populate the icon metadata table.
   - `TURF_UPDATE` frames set turf appearances at absolute coordinates.
   - `MOVABLE_UPDATE` frames create/update atom state.
   - `MOVABLE_DELETE` frames remove atoms.
   - `ANIMATION` and `FLICK` frames apply to existing atoms.
   - `EYE_UPDATE` frames move the camera.
   - `MAP_RESIZE` frames resize the world grid.
5. Render using the appearance table, icon metadata, and `.dmi` sprites.

## Versioning

Both `appearances.dat` and `.replay` files contain a `version` field. Readers should reject files with a version higher than they support. When the format changes, increment the version and document the differences.
