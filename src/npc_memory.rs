//! NPC memory system — records, retrieves, and persists NPC memories.
//!
//! Each NPC with an `NpcMemory` component accumulates short one-line summaries
//! of events (conversations, actions, observations). Memories are scored by
//! importance and recency so that only the most relevant entries are included
//! in LLM prompts, keeping token usage within budget.
//!
//! Memories persist across sessions via JSON files in the `saves/` directory.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::text_input::SayEvent;
use crate::{EntityId, NpcPersonality};

// ---------------------------------------------------------------------------
// Data types
// ---------------------------------------------------------------------------

/// A single memory entry from an NPC's perspective.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MemoryEntry {
    /// Short one-line summary from the NPC's perspective.
    /// e.g. "The wanderer asked me about the Abyss."
    pub text: String,
    /// Importance score 1–10. Higher = more likely to be included in prompts.
    pub importance: u8,
    /// Game time (seconds) when this memory was created.
    pub timestamp: f64,
    /// Related entity name, if any (e.g. "the wanderer", "Sir Roland").
    pub related_entity: Option<String>,
}

/// Persistent memory storage for a single NPC.
#[derive(Component, Debug, Clone, Serialize, Deserialize)]
pub struct NpcMemory {
    /// All recorded memories, newest last.
    pub entries: Vec<MemoryEntry>,
}

impl Default for NpcMemory {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
        }
    }
}

impl NpcMemory {
    /// Maximum entries before old low-importance memories are pruned.
    const MAX_ENTRIES: usize = 100;

    /// Add a new memory, pruning if over capacity.
    pub fn record(&mut self, text: String, importance: u8, timestamp: f64, related_entity: Option<String>) {
        self.entries.push(MemoryEntry {
            text,
            importance: importance.clamp(1, 10),
            timestamp,
            related_entity,
        });

        // Prune if over capacity: remove lowest-importance, oldest entries first.
        if self.entries.len() > Self::MAX_ENTRIES {
            // Sort by importance asc, then timestamp asc (oldest first).
            // Remove entries from the front (least important + oldest).
            self.entries.sort_by(|a, b| {
                a.importance.cmp(&b.importance).then(a.timestamp.partial_cmp(&b.timestamp).unwrap_or(std::cmp::Ordering::Equal))
            });
            let remove_count = self.entries.len() - Self::MAX_ENTRIES;
            self.entries.drain(..remove_count);
            // Re-sort by timestamp ascending (chronological order).
            self.entries.sort_by(|a, b| {
                a.timestamp.partial_cmp(&b.timestamp).unwrap_or(std::cmp::Ordering::Equal)
            });
        }
    }

    /// Return the most relevant memories for inclusion in an LLM prompt.
    /// Sorted by importance descending, then recency descending.
    /// Returns at most `limit` entries.
    pub fn top_memories(&self, limit: usize) -> Vec<&MemoryEntry> {
        let mut refs: Vec<&MemoryEntry> = self.entries.iter().collect();
        // Primary sort: importance descending. Secondary: timestamp descending (recent first).
        refs.sort_by(|a, b| {
            b.importance.cmp(&a.importance).then(
                b.timestamp
                    .partial_cmp(&a.timestamp)
                    .unwrap_or(std::cmp::Ordering::Equal),
            )
        });
        refs.truncate(limit);
        refs
    }

    /// Format the top memories as a prompt section string.
    /// Returns empty string if no memories exist.
    pub fn format_for_prompt(&self, limit: usize) -> String {
        let top = self.top_memories(limit);
        if top.is_empty() {
            return String::new();
        }

        let mut lines = Vec::with_capacity(top.len() + 1);
        lines.push("[MEMORY]".to_string());
        for entry in &top {
            lines.push(format!("- {}", entry.text));
        }
        lines.join("\n")
    }
}

// ---------------------------------------------------------------------------
// Events
// ---------------------------------------------------------------------------

/// Event fired to record a memory for a specific NPC.
#[derive(Message)]
pub struct RecordMemory {
    /// The NPC entity that should remember this.
    pub npc: Entity,
    /// Short summary text from the NPC's perspective.
    pub text: String,
    /// Importance 1–10.
    pub importance: u8,
    /// Related entity name (e.g. "the wanderer").
    pub related_entity: Option<String>,
}

// ---------------------------------------------------------------------------
// Plugin
// ---------------------------------------------------------------------------

pub struct NpcMemoryPlugin;

impl Plugin for NpcMemoryPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<RecordMemory>()
            .add_systems(Startup, load_memories_system)
            .add_systems(
                Update,
                (
                    attach_memory_to_npcs,
                    record_memory_system,
                    record_say_memory_system,
                    save_memories_system,
                )
                    .run_if(in_state(crate::GameState::Playing)),
            );
    }
}

// ---------------------------------------------------------------------------
// Systems
// ---------------------------------------------------------------------------

/// Ensures every NPC with a personality also has an NpcMemory component.
fn attach_memory_to_npcs(
    mut commands: Commands,
    npcs: Query<Entity, (With<NpcPersonality>, Without<NpcMemory>)>,
) {
    for entity in &npcs {
        commands.entity(entity).insert(NpcMemory::default());
    }
}

/// Processes RecordMemory events and adds entries to NPC memories.
fn record_memory_system(
    mut events: MessageReader<RecordMemory>,
    mut memories: Query<&mut NpcMemory>,
    time: Res<Time>,
) {
    for event in events.read() {
        if let Ok(mut memory) = memories.get_mut(event.npc) {
            let game_time = time.elapsed_secs_f64();
            memory.record(
                event.text.clone(),
                event.importance,
                game_time,
                event.related_entity.clone(),
            );
        }
    }
}

/// When the player says something to an NPC, record it as a memory for that NPC.
fn record_say_memory_system(
    mut say_events: MessageReader<SayEvent>,
    mut memories: Query<&mut NpcMemory>,
    time: Res<Time>,
) {
    for event in say_events.read() {
        if let Ok(mut memory) = memories.get_mut(event.npc) {
            let game_time = time.elapsed_secs_f64();
            // Summarize what the wanderer said — keep it short.
            let summary = summarize_player_speech(&event.text);
            memory.record(summary, 6, game_time, Some("the wanderer".to_string()));
        }
    }
}

/// Create a short NPC-perspective summary of what the player said.
fn summarize_player_speech(player_text: &str) -> String {
    let trimmed = player_text.trim();
    // If the player's text is short enough, quote it directly.
    if trimmed.len() <= 60 {
        format!("The wanderer said: \"{}\"", trimmed)
    } else {
        // Truncate long messages to keep memory entries concise.
        let short = &trimmed[..57];
        // Find last word boundary to avoid cutting mid-word.
        let end = short.rfind(' ').unwrap_or(57);
        format!("The wanderer said: \"{}...\"", &trimmed[..end])
    }
}

// ---------------------------------------------------------------------------
// Persistence
// ---------------------------------------------------------------------------

/// Directory where memory files are saved.
fn saves_dir() -> PathBuf {
    PathBuf::from("saves")
}

/// Build the file path for a given NPC's memory file.
fn memory_file_path(npc_id: &str) -> PathBuf {
    saves_dir().join(format!("{}_memory.json", npc_id))
}

/// On startup, load persisted memories for all NPCs that have an EntityId.
fn load_memories_system(
    mut commands: Commands,
    npcs: Query<(Entity, &EntityId), With<NpcPersonality>>,
) {
    let dir = saves_dir();
    if !dir.exists() {
        return;
    }

    for (entity, entity_id) in &npcs {
        let path = memory_file_path(&entity_id.0);
        if path.exists() {
            match std::fs::read_to_string(&path) {
                Ok(json) => match serde_json::from_str::<NpcMemory>(&json) {
                    Ok(memory) => {
                        info!("Loaded {} memories for NPC '{}'", memory.entries.len(), entity_id.0);
                        commands.entity(entity).insert(memory);
                    }
                    Err(e) => {
                        warn!("Failed to parse memory file for '{}': {}", entity_id.0, e);
                    }
                },
                Err(e) => {
                    warn!("Failed to read memory file for '{}': {}", entity_id.0, e);
                }
            }
        }
    }
}

/// Resource tracking when we last saved, to avoid saving every frame.
#[derive(Resource)]
struct MemorySaveTimer(Timer);

impl Default for MemorySaveTimer {
    fn default() -> Self {
        Self(Timer::from_seconds(30.0, TimerMode::Repeating))
    }
}

/// Periodically save all NPC memories to disk.
fn save_memories_system(
    time: Res<Time>,
    mut timer: Local<MemorySaveTimer>,
    npcs: Query<(&EntityId, &NpcMemory)>,
) {
    timer.0.tick(time.delta());
    if !timer.0.just_finished() {
        return;
    }

    let dir = saves_dir();
    if let Err(e) = std::fs::create_dir_all(&dir) {
        warn!("Failed to create saves directory: {}", e);
        return;
    }

    for (entity_id, memory) in &npcs {
        if memory.entries.is_empty() {
            continue;
        }
        let path = memory_file_path(&entity_id.0);
        match serde_json::to_string_pretty(memory) {
            Ok(json) => {
                if let Err(e) = std::fs::write(&path, json) {
                    warn!("Failed to write memory file for '{}': {}", entity_id.0, e);
                }
            }
            Err(e) => {
                warn!("Failed to serialize memory for '{}': {}", entity_id.0, e);
            }
        }
    }
}

/// Force-save all memories immediately (call on game exit or state transitions).
pub fn force_save_all_memories(
    npcs: Query<(&EntityId, &NpcMemory)>,
) {
    let dir = saves_dir();
    if let Err(e) = std::fs::create_dir_all(&dir) {
        warn!("Failed to create saves directory: {}", e);
        return;
    }

    for (entity_id, memory) in &npcs {
        if memory.entries.is_empty() {
            continue;
        }
        let path = memory_file_path(&entity_id.0);
        match serde_json::to_string_pretty(memory) {
            Ok(json) => {
                if let Err(e) = std::fs::write(&path, json) {
                    warn!("Failed to write memory file for '{}': {}", entity_id.0, e);
                }
            }
            Err(e) => {
                warn!("Failed to serialize memory for '{}': {}", entity_id.0, e);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers for external systems
// ---------------------------------------------------------------------------

/// Record that an NPC spoke a line (for their own memory of what they said).
pub fn record_npc_spoke(memory: &mut NpcMemory, npc_name: &str, text: &str, game_time: f64) {
    let summary = if text.len() <= 50 {
        format!("I said: \"{}\"", text)
    } else {
        let short = &text[..47];
        let end = short.rfind(' ').unwrap_or(47);
        format!("I said: \"{}...\"", &text[..end])
    };
    memory.record(summary, 3, game_time, Some(npc_name.to_string()));
}

/// Record that an NPC performed an action.
pub fn record_npc_action(memory: &mut NpcMemory, action_desc: &str, game_time: f64) {
    memory.record(action_desc.to_string(), 4, game_time, None);
}

/// Record that an NPC observed something about another entity.
pub fn record_npc_observation(
    memory: &mut NpcMemory,
    observation: &str,
    game_time: f64,
    related: Option<String>,
) {
    memory.record(observation.to_string(), 5, game_time, related);
}
