//! TTS integration via Chatterbox Python subprocess.
//!
//! Spawns a persistent Python worker process that keeps the Chatterbox model loaded.
//! Communication happens via JSON lines over stdin/stdout.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, Command, Stdio};
use std::sync::{mpsc, Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};

// ---------------------------------------------------------------------------
// Data types
// ---------------------------------------------------------------------------

/// Event sent when dialogue is shown and TTS audio should be generated.
#[derive(Message, Debug, Clone)]
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
    #[serde(default)]
    message: Option<String>,
    #[serde(default)]
    progress: Option<f32>,
}

// ---------------------------------------------------------------------------
// TtsEngine resource
// ---------------------------------------------------------------------------

/// Resource that manages the persistent Chatterbox Python subprocess.
#[derive(Resource)]
/// Loading progress info from the worker.
pub struct LoadingStatus {
    pub message: String,
    pub progress: f32,
}

#[derive(Resource)]
pub struct TtsEngine {
    request_tx: mpsc::Sender<(String, String, String, Entity)>,
    response_rx: Mutex<mpsc::Receiver<TtsResponse>>,
    loading_rx: Mutex<mpsc::Receiver<LoadingStatus>>,
    next_id: u64,
    pub ready: Arc<AtomicBool>,
}

impl TtsEngine {
    /// Spawn the TTS worker subprocess and communication threads.
    pub fn new() -> Self {
        let (request_tx, request_rx) = mpsc::channel::<(String, String, String, Entity)>();
        let (response_tx, response_rx) = mpsc::channel::<TtsResponse>();
        let (loading_tx, loading_rx) = mpsc::channel::<LoadingStatus>();
        let ready = Arc::new(AtomicBool::new(false));
        let ready_clone = ready.clone();

        std::thread::spawn(move || {
            Self::worker_thread(request_rx, response_tx, ready_clone, loading_tx);
        });

        Self {
            request_tx,
            response_rx: Mutex::new(response_rx),
            loading_rx: Mutex::new(loading_rx),
            next_id: 0,
            ready,
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

    pub fn poll(&self) -> Option<TtsResponse> {
        self.response_rx.lock().ok()?.try_recv().ok()
    }

    pub fn poll_loading(&self) -> Option<LoadingStatus> {
        self.loading_rx.lock().ok()?.try_recv().ok()
    }

    /// Background thread that owns the Python subprocess.
    fn worker_thread(
        request_rx: mpsc::Receiver<(String, String, String, Entity)>,
        response_tx: mpsc::Sender<TtsResponse>,
        ready_flag: Arc<AtomicBool>,
        loading_tx: mpsc::Sender<LoadingStatus>,
    ) {
        // Determine the path to the worker script
        // Look for tts_worker.py in scripts/ or next to executable
        let worker_script = ["scripts/tts_worker.py", "tts_worker.py"]
            .iter()
            .find(|p| std::path::Path::new(p).exists())
            .map(|s| s.to_string())
            .or_else(|| {
                std::env::current_exe().ok()
                    .and_then(|p| p.parent().map(|d| d.join("tts_worker.py")))
                    .filter(|p| p.exists())
                    .map(|p| p.to_string_lossy().to_string())
            })
            .unwrap_or_else(|| {
                error!("Could not find tts_worker.py");
                "scripts/tts_worker.py".to_string()
            });

        // Try venv python first, fall back to system python3
        let python_bin = [
            std::env::var("HOLLOWREACH_PYTHON").ok(),
            Some("chatterbox-venv/bin/python3".to_string()),
            dirs().map(|d| format!("{}/chatterbox-venv/bin/python3", d)),
            Some("python3".to_string()),
        ]
        .into_iter()
        .flatten()
        .find(|p| std::path::Path::new(p).exists())
        .unwrap_or_else(|| "python3".to_string());

        fn dirs() -> Option<String> {
            std::env::var("HOME").ok()
        }

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
                        if resp.status == "loading" {
                            let _ = loading_tx.send(LoadingStatus {
                                message: resp.message.unwrap_or_else(|| "Loading...".into()),
                                progress: resp.progress.unwrap_or(0.0),
                            });
                        }
                        if resp.status == "ready" {
                            let _ = loading_tx.send(LoadingStatus {
                                message: "Ready".into(),
                                progress: 1.0,
                            });
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

        ready_flag.store(true, Ordering::SeqCst);
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
    mut events: MessageReader<TtsRequest>,
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
    audio_settings: Res<crate::AudioSettings>,
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
        let speech_vol = audio_settings.effective_speech();
        commands.spawn((
            AudioPlayer::<AudioSource>(audio_handle),
            PlaybackSettings {
                volume: bevy::audio::Volume::Linear(speech_vol),
                ..PlaybackSettings::DESPAWN
            },
            TtsAudioPlayback,
        ));

        // Clean up the temp file
        let _ = std::fs::remove_file(&response.audio_path);
    }
}

// ---------------------------------------------------------------------------
// Plugin
// ---------------------------------------------------------------------------

/// Marker for the loading screen overlay.
#[derive(Component)]
struct TtsLoadingScreen;

/// Marker for the loading status text.
#[derive(Component)]
struct TtsLoadingStatusText;

/// Marker for the progress bar fill.
#[derive(Component)]
struct TtsProgressBar;

fn tts_loading_ui(mut commands: Commands) {
    commands
        .spawn((
            TtsLoadingScreen,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                right: Val::Px(0.0),
                top: Val::Px(0.0),
                bottom: Val::Px(0.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(16.0),
                ..default()
            },
            BackgroundColor(Color::srgb(0.0, 0.0, 0.0)),
            GlobalZIndex(500),
        ))
        .with_children(|parent| {
            // Status text
            parent.spawn((
                TtsLoadingStatusText,
                Text::new("Starting..."),
                TextFont { font_size: 16.0, ..default() },
                TextColor(Color::srgba(0.7, 0.7, 0.7, 0.9)),
            ));

            // Progress bar background
            parent
                .spawn((
                    Node {
                        width: Val::Px(300.0),
                        height: Val::Px(6.0),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.3, 0.3, 0.3, 0.5)),
                ))
                .with_children(|bar_bg| {
                    // Progress bar fill
                    bar_bg.spawn((
                        TtsProgressBar,
                        Node {
                            width: Val::Percent(0.0),
                            height: Val::Percent(100.0),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.95, 0.82, 0.4)),
                    ));
                });
        });
}

fn tts_loading_ui_update(
    mut commands: Commands,
    engine: Option<Res<TtsEngine>>,
    screen_q: Query<Entity, With<TtsLoadingScreen>>,
    mut text_q: Query<&mut Text, With<TtsLoadingStatusText>>,
    mut bar_q: Query<&mut Node, With<TtsProgressBar>>,
    mut next_state: ResMut<NextState<crate::GameState>>,
) {
    let Some(engine) = engine else { return };

    while let Some(status) = engine.poll_loading() {
        if let Ok(mut text) = text_q.single_mut() {
            **text = status.message;
        }
        if let Ok(mut node) = bar_q.single_mut() {
            node.width = Val::Percent(status.progress * 100.0);
        }
    }

    if engine.ready.load(Ordering::SeqCst) {
        for entity in &screen_q {
            commands.entity(entity).despawn();
        }
        next_state.set(crate::GameState::Playing);
    }
}

/// Run condition: true only when TTS engine is loaded and ready.
pub fn tts_ready(engine: Option<Res<TtsEngine>>) -> bool {
    engine.is_some_and(|e| e.ready.load(Ordering::SeqCst))
}

pub struct TtsPlugin;

impl Plugin for TtsPlugin {
    fn build(&self, app: &mut App) {
        // Start worker immediately during plugin build (before any Bevy systems run)
        let _ = std::fs::create_dir_all("/tmp/hollowreach_tts");
        let engine = TtsEngine::new();
        app.insert_resource(engine)
            .add_message::<TtsRequest>()
            .add_systems(Startup, tts_loading_ui)
            .add_systems(Update, (tts_request_system, tts_poll_system).chain())
            .add_systems(Update, tts_loading_ui_update);
    }
}
