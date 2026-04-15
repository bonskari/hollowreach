//! Persistent game session log.
//!
//! Writes every player action and NPC reply to `saves/session_<timestamp>.log`
//! so the user can hand the file to analysis.

use bevy::prelude::*;
use std::fs::{create_dir_all, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;

use crate::chat_log::PushChatMessage;
use crate::text_input::SayEvent;
use crate::{NpcPersonality, Player};

// ---------------------------------------------------------------------------
// Resource
// ---------------------------------------------------------------------------

/// Handle to the current session log file.
#[derive(Resource)]
pub struct GameLog {
    path: PathBuf,
    writer: Mutex<std::fs::File>,
}

impl GameLog {
    fn append(&self, line: &str) {
        if let Ok(mut w) = self.writer.lock() {
            let _ = writeln!(w, "{line}");
            let _ = w.flush();
        }
    }

    pub fn log_action(&self, category: &str, detail: &str) {
        let ts = chrono_like_now();
        self.append(&format!("[{ts}] {category}: {detail}"));
    }

    pub fn path(&self) -> &std::path::Path {
        &self.path
    }
}

// ---------------------------------------------------------------------------
// Plugin
// ---------------------------------------------------------------------------

pub struct GameLogPlugin;

impl Plugin for GameLogPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, init_game_log)
            .add_systems(
                Update,
                (log_say_events, log_chat_messages, log_player_interact),
            );
    }
}

// ---------------------------------------------------------------------------
// Startup
// ---------------------------------------------------------------------------

fn init_game_log(mut commands: Commands) {
    let dir = PathBuf::from("saves");
    let _ = create_dir_all(&dir);

    let ts = chrono_like_now().replace(':', "-").replace(' ', "_");
    let path = dir.join(format!("session_{ts}.log"));

    let file = match OpenOptions::new().create(true).append(true).open(&path) {
        Ok(f) => f,
        Err(e) => {
            error!("GameLog: failed to open {path:?}: {e}");
            return;
        }
    };

    let log = GameLog {
        path: path.clone(),
        writer: Mutex::new(file),
    };
    log.log_action("SESSION", "Hollowreach session started");
    info!("Session log: {}", path.display());
    commands.insert_resource(log);
}

// ---------------------------------------------------------------------------
// Logging systems
// ---------------------------------------------------------------------------

/// Log every SayEvent (player speech to NPC) and the target NPC name.
fn log_say_events(
    mut say_events: MessageReader<SayEvent>,
    personality_q: Query<&NpcPersonality>,
    log: Option<Res<GameLog>>,
) {
    let Some(log) = log else { return };
    for ev in say_events.read() {
        let npc_name = personality_q
            .get(ev.npc)
            .map(|p| p.name.clone())
            .unwrap_or_else(|_| format!("entity {:?}", ev.npc));
        log.log_action("SAY", &format!("player -> {}: {:?}", npc_name, ev.text));
    }
}

/// Log every chat message that appears on screen (player + NPC).
fn log_chat_messages(
    mut chat_events: MessageReader<PushChatMessage>,
    log: Option<Res<GameLog>>,
) {
    let Some(log) = log else { return };
    for ev in chat_events.read() {
        log.log_action("CHAT", &format!("{}: {:?}", ev.speaker, ev.text));
    }
}

/// Log when the player presses E to interact with an NPC.
fn log_player_interact(
    keyboard: Res<ButtonInput<KeyCode>>,
    player_q: Query<&Transform, With<Player>>,
    log: Option<Res<GameLog>>,
) {
    let Some(log) = log else { return };
    if keyboard.just_pressed(KeyCode::KeyE) {
        let pos = player_q
            .single()
            .map(|t| format!("{:?}", t.translation))
            .unwrap_or_else(|_| "?".into());
        log.log_action("INTERACT", &format!("player pressed E at {}", pos));
    }
}

// ---------------------------------------------------------------------------
// Timestamp (no extra crate — use SystemTime)
// ---------------------------------------------------------------------------

fn chrono_like_now() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let dur = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let secs = dur.as_secs();
    let ms = dur.subsec_millis();

    // HH:MM:SS.mmm based on seconds since epoch (UTC, no date).
    let h = (secs / 3600) % 24;
    let m = (secs / 60) % 60;
    let s = secs % 60;
    format!("{h:02}:{m:02}:{s:02}.{ms:03}")
}
