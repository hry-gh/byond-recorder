// Recorder integration hooks.
// Include this file in your game's .dme to enable replay recording.
//
// Configuration:
//   #define RECORDER_API_URL "http://localhost:6062"
//   #define RECORDER_ENABLED

#ifndef RECORDER_API_URL
#define RECORDER_API_URL "http://localhost:6062"
#endif

/proc/recorder_round_start(round_id)
#ifdef RECORDER_ENABLED
	world.Export("[RECORDER_API_URL]/round_start", "{\"round_id\": \"[round_id]\"}")
#endif

/proc/recorder_round_end()
#ifdef RECORDER_ENABLED
	world.Export("[RECORDER_API_URL]/round_end", "{}")
#endif
