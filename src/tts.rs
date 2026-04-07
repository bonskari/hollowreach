//! TTS integration via Chatterbox Python subprocess.
//!
//! Spawns a persistent Python worker process that keeps the Chatterbox model loaded.
//! Communication happens via JSON lines over stdin/stdout.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, Command, Stdio};
use std::sync::{mpsc, Mutex};

// ---------------------------------------------------------------------------
// Data types
// ---------------------------------------------------------------------------

/// Event sent when dialogue is shown and TTS audio should be generated.
#[derive(Event, Debug, Clone)]
pub struct TtsRequest {
    pub text: String,
    pub voice_profile: String,
    pub npc_entity: Entity,
}

/// Internal response from the TTS worker.
#[derive(Debug, Clone)]
pub struct TtsResponse {
    pub audio_path: String,
    pub npc_entity: Entity,
}

/// Marker component for an entity currently playing TTS audio.
#[derive(Component)]
pub struct TtsAudioPlayback;

/// JSON payload sent to the Python worker.
#[derive(Serialize)]
struct WorkerRequest {
    text: String,
    voice_profile: String,
    output_path: String,
}

/// JSON payload received from the Python worker.
#[derive(Deserialize, Debug)]
struct WorkerResponse {
    status: String,
    #[serde(default)]
    path: Option<String>,
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    device: Option<String>,
}

// ---------------------------------------------------------------------------
// TtsEngine resource
// ---------------------------------------------------------------------------

/// Resource that manages the persistent Chatterbox Python subprocess.
#[derive(Resource)]
pub struct TtsEngine {
    /// Channel sender for requests to the background thread.
    request_tx: mpsc::Sender<(String, String, String, Entity)>,
    /// Channel receiver for completed responses (wrapped in Mutex for Sync).
    response_rx: Mutex<mpsc::Receiver<TtsResponse>>,
    /// Counter for generating unique filenames.
    next_id: u64,
    /// Whether the engine is ready (model loaded).
    pub ready: bool,
}

impl TtsEngine {
    /// Spawn the TTS worker subprocess and communication threads.
    pub fn new() -> Self {
        let (request_tx, request_rx) = mpsc::channel::<(String, String, String, Entity)>();
        let (response_tx, response_rx) = mpsc::channel::<TtsResponse>();

        // Spawn background thread that manages the Python process
        std::thread::spawn(move || {
            Self::worker_thread(request_rx, response_tx);
        });

        Self {
            request_tx,
            response_rx: Mutex::new(response_rx),
            next_id: 0,
            ready: false,
        }
    }

    /// Send a TTS request to the worker.
    pub fn request(&mut self, text: String, voice_profile: String, npc_entity: Entity) {
        let output_path = format!("/tmp/hollowreach_tts/tts_{}.wav", self.next_id);
        self.next_id += 1;

        if let Err(e) = self.request_tx.send((text, voice_profile, output_path, npc_entity)) {
            warn!("TTS request send failed: {}", e);
        }
    }

    /// Poll for completed TTS responses (non-blocking).
    pub fn poll(&self) -> Option<TtsResponse> {
        self.response_rx.lock().ok()?.try_recv().ok()
    }

    /// Background thread that owns the Python subprocess.
    fn worker_thread(
        request_rx: mpsc::Receiver<(String, String, String, Entity)>,
        response_tx: mpsc::Sender<TtsResponse>,
    ) {
        // Determine the path to the worker script
        let worker_script = if std::path::Path::new("assets/scripts/tts_worker.py").exists() {
            "assets/scripts/tts_worker.py".to_string()
        } else {
            // Fallback: look relative to the executable
            let exe_dir = std::env::current_exe()
                .ok()
                .and_then(|p| p.parent().map(|p| p.to_path_buf()));
            if let Some(dir) = exe_dir {
                dir.join("assets/scripts/tts_worker.py")
                    .to_string_lossy()
                    .to_string()
            } else {
                error!("Could not find tts_worker.py");
                return;
            }
        };

        let python_bin = "/home/b1s/chatterbox-venv/bin/python3";

        info!("TTS: spawning worker subprocess: {} {}", python_bin, worker_script);

        let mut child: Child = match Command::new(python_bin)
            .arg(&worker_script)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
        {
            Ok(c) => c,
            Err(e) => {
                error!("TTS: failed to spawn Python worker: {}", e);
                return;
            }
        };

        let mut stdin = child.stdin.take().expect("Failed to open worker stdin");
        let stdout = child.stdout.take().expect("Failed to open worker stdout");
        let mut reader = BufReader::new(stdout);

        // Wait for the worker to signal ready
        loop {
            let mut line = String::new();
            match reader.read_line(&mut line) {
                Ok(0) => {
                    error!("TTS: worker process exited during startup");
                    return;
                }
                Ok(_) => {
                    if let Ok(resp) = serde_json::from_str::<WorkerResponse>(&line) {
                        info!("TTS worker status: {} (device: {:?})", resp.status, resp.device);
                        if resp.status == "ready" {
                            break;
                        }
                        if resp.status == "error" {
                            error!("TTS worker error during startup: {:?}", resp.error);
                            return;
                        }
                    }
                }
                Err(e) => {
                    error!("TTS: error reading from worker: {}", e);
                    return;
                }
            }
        }

        info!("TTS: worker ready, processing requests");

        // Process requests
        for (text, voice_profile, output_path, npc_entity) in request_rx {
            let req = WorkerRequest {
                text,
                voice_profile,
                output_path: output_path.clone(),
            };

            let req_json = serde_json::to_string(&req).unwrap();

            if let Err(e) = writeln!(stdin, "{}", req_json) {
                error!("TTS: failed to write to worker stdin: {}", e);
                break;
            }
            if let Err(e) = stdin.flush() {
                error!("TTS: failed to flush worker stdin: {}", e);
                break;
            }

            // Read response
            let mut line = String::new();
            match reader.read_line(&mut line) {
                Ok(0) => {
                    error!("TTS: worker process exited unexpectedly");
                    break;
                }
                Ok(_) => {
                    match serde_json::from_str::<WorkerResponse>(&line) {
                        Ok(resp) => {
                            if resp.status == "done" {
                                if let Some(path) = resp.path {
                                    let _ = response_tx.send(TtsResponse {
                                        audio_path: path,
                                        npc_entity,
                                    });
                                }
                            } else if resp.status == "error" {
                                warn!("TTS generation error: {:?}", resp.error);
                            }
                        }
                        Err(e) => {
                            warn!("TTS: failed to parse worker response: {} (line: {})", e, line.trim());
                        }
                    }
                }
                Err(e) => {
                    error!("TTS: error reading worker response: {}", e);
                    break;
                }
            }
        }

        // Clean up
        let _ = child.kill();
    }
}

// ---------------------------------------------------------------------------
// Systems
// ---------------------------------------------------------------------------

/// Startup system: initialize the TTS engine.
fn tts_startup(mut commands: Commands) {
    // Create output directory
    let _ = std::fs::create_dir_all("/tmp/hollowreach_tts");

    info!("TTS: initializing engine");
    commands.insert_resource(TtsEngine::new());
}

/// System that listens for TtsRequest events and sends them to the engine.
fn tts_request_system(
    mut events: EventReader<TtsRequest>,
    mut engine: ResMut<TtsEngine>,
) {
    for req in events.read() {
        info!("TTS: requesting speech for entity {:?}: \"{}\"", req.npc_entity, req.text);
        engine.request(req.text.clone(), req.voice_profile.clone(), req.npc_entity);
    }
}

/// System that polls for completed TTS audio and plays it.
fn tts_poll_system(
    mut commands: Commands,
    engine: Res<TtsEngine>,
    asset_server: Res<AssetServer>,
) {
    while let Some(response) = engine.poll() {
        info!("TTS: audio ready at {}", response.audio_path);

        // Load the generated WAV file as a Bevy audio source.
        // The file is in /tmp, so we need to copy it to an assets-accessible location
        // or use an absolute path. Bevy's asset server loads from the assets/ directory,
        // so we copy the file there.
        let dest_filename = std::path::Path::new(&response.audio_path)
            .file_name()
            .unwrap()
            .to_string_lossy()
            .to_string();
        let dest_path = format!("assets/audio/tts/{}", dest_filename);

        // Ensure the tts audio directory exists
        let _ = std::fs::create_dir_all("assets/audio/tts");

        // Copy the generated WAV to the assets directory
        if let Err(e) = std::fs::copy(&response.audio_path, &dest_path) {
            warn!("TTS: failed to copy audio file: {}", e);
            continue;
        }

        // Load via Bevy asset server
        let audio_handle: Handle<AudioSource> = asset_server.load(format!("audio/tts/{}", dest_filename));

        // Spawn a one-shot audio player entity
        // TODO: In the future, attach this as spatial audio to the NPC entity
        commands.spawn((
            AudioPlayer::<AudioSource>(audio_handle),
            PlaybackSettings::DESPAWN,
            TtsAudioPlayback,
        ));

        // Clean up the temp file
        let _ = std::fs::remove_file(&response.audio_path);
    }
}

// ---------------------------------------------------------------------------
// Plugin
// ---------------------------------------------------------------------------

pub struct TtsPlugin;

impl Plugin for TtsPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<TtsRequest>()
            .add_systems(Startup, tts_startup)
            .add_systems(Update, (tts_request_system, tts_poll_system).chain());
    }
}
