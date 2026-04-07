//! Context area system for Hollowreach.
//!
//! Provides dynamic bounds-based entity collection for NPC AI.
//! Areas are XZ-axis-aligned rectangular regions. Every frame, all entities
//! with a `Transform` are tested against every `ContextArea`; the `InArea`
//! component on each entity is updated to reflect the area it currently
//! occupies.

use bevy::prelude::*;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

// ---------------------------------------------------------------------------
// Components
// ---------------------------------------------------------------------------

/// Attached to an area entity. Defines a rectangular region on the XZ plane.
#[derive(Component, Debug, Clone)]
pub struct ContextArea {
    /// Unique identifier for this area (e.g. `"courtyard"`).
    pub id: String,
    /// Human-readable name (e.g. `"Village Courtyard"`).
    pub label: String,
    /// Narrative description fed into LLM prompts.
    pub description: String,
    /// Minimum corner of the XZ bounding rectangle.
    pub min: Vec2,
    /// Maximum corner of the XZ bounding rectangle.
    pub max: Vec2,
    /// IDs of neighbouring areas (for NPC movement decisions).
    pub adjacent_areas: Vec<String>,
}

impl ContextArea {
    /// Returns `true` when the given world-space position falls inside the
    /// area bounds (XZ plane only; Y is ignored).
    pub fn contains(&self, pos: Vec3) -> bool {
        pos.x >= self.min.x && pos.x <= self.max.x && pos.z >= self.min.y && pos.z <= self.max.y
    }
}

/// Attached to any entity that can be tracked by the area system.
/// Holds the `id` of the `ContextArea` the entity is currently inside,
/// or `None` when the entity is outside every defined area.
#[derive(Component, Debug, Clone, Default)]
pub struct InArea(pub Option<String>);

// ---------------------------------------------------------------------------
// Snapshot types
// ---------------------------------------------------------------------------

/// Lightweight description of a single entity inside an area.
#[derive(Debug, Clone, Hash)]
pub struct EntitySnapshot {
    pub id: String,
    pub state: String,
    pub position: [i32; 3], // discretised for deterministic hashing
    pub interactions: Vec<String>,
}

/// Lightweight description of an NPC inside an area.
#[derive(Debug, Clone, Hash)]
pub struct NpcSnapshot {
    pub id: String,
    pub name: String,
    pub state: String,
    pub position: [i32; 3],
    pub current_action: String,
}

/// Information about the player relevant to the area context.
#[derive(Debug, Clone, Hash)]
pub struct PlayerSnapshot {
    pub position: [i32; 3],
    pub inventory: Vec<String>,
}

/// Complete snapshot of everything happening inside a single area at one
/// instant. Built on demand by [`build_area_context`] and consumed by the
/// NPC decision pipeline.
#[derive(Debug, Clone, Hash)]
pub struct AreaContext {
    pub area_id: String,
    pub area_label: String,
    pub area_description: String,
    pub entities: Vec<EntitySnapshot>,
    pub npcs: Vec<NpcSnapshot>,
    pub player: Option<PlayerSnapshot>,
}

// ---------------------------------------------------------------------------
// Dirty-detection component
// ---------------------------------------------------------------------------

/// Per-NPC component that stores the hash of the last `AreaContext` the NPC
/// observed. When the current hash differs from the stored one the context is
/// considered *dirty* and a new LLM query should be issued.
#[derive(Component, Debug, Clone, Default)]
pub struct NpcContextHash {
    pub last_context_hash: u64,
}

// ---------------------------------------------------------------------------
// Marker / state components referenced by the snapshot builder
// ---------------------------------------------------------------------------

/// Unique string identifier for a world entity (e.g. `"door_tavern"`).
#[derive(Component, Debug, Clone)]
pub struct EntityId(pub String);

/// Free-form state string (e.g. `"locked"`, `"open"`, `"idle"`).
#[derive(Component, Debug, Clone)]
pub struct EntityState(pub String);

/// List of interaction IDs currently available on an entity.
#[derive(Component, Debug, Clone)]
pub struct InteractionList(pub Vec<String>);

/// NPC personality — the `name` field is used in snapshots.
#[derive(Component, Debug, Clone)]
pub struct NpcPersonality {
    pub name: String,
    pub role: String,
    pub traits: Vec<String>,
    pub backstory: String,
    pub speech_style: String,
    pub knowledge: Vec<String>,
    pub goals: Vec<String>,
    pub likes: Vec<String>,
    pub dislikes: Vec<String>,
}

/// What the NPC is currently doing, serialised as a string for the snapshot.
#[derive(Component, Debug, Clone, Default)]
pub struct NpcCurrentAction(pub String);

/// The player's inventory, kept as simple item-ID strings.
#[derive(Component, Debug, Clone, Default)]
pub struct PlayerInventory(pub Vec<String>);

/// Marker that identifies the NPC tag (used in queries).
#[derive(Component, Debug, Clone)]
pub struct Npc;

// ---------------------------------------------------------------------------
// Systems
// ---------------------------------------------------------------------------

/// Each frame, iterate over every entity that has both a `Transform` and an
/// `InArea` component. Test its XZ position against all `ContextArea` bounds
/// and update `InArea` accordingly. First match wins (areas should not
/// overlap in practice).
pub fn update_area_membership(
    areas: Query<&ContextArea>,
    mut tracked: Query<(&Transform, &mut InArea)>,
) {
    for (transform, mut in_area) in tracked.iter_mut() {
        let pos = transform.translation;
        let mut found: Option<String> = None;
        for area in areas.iter() {
            if area.contains(pos) {
                found = Some(area.id.clone());
                break;
            }
        }
        if in_area.0 != found {
            in_area.0 = found;
        }
    }
}

// ---------------------------------------------------------------------------
// Context builder
// ---------------------------------------------------------------------------

/// Discretise a `Vec3` into an `[i32; 3]` so that floating-point jitter does
/// not cause spurious hash changes. Resolution: 10 cm.
fn discretise(v: Vec3) -> [i32; 3] {
    [
        (v.x * 10.0).round() as i32,
        (v.y * 10.0).round() as i32,
        (v.z * 10.0).round() as i32,
    ]
}

/// Build a full [`AreaContext`] snapshot for the given `area_id`.
///
/// This performs ECS queries to collect every entity, NPC, and the player
/// whose `InArea` matches the requested area.
///
/// Because Bevy queries require `&World` or system params, this function
/// takes pre-queried slices for maximum flexibility (works from both
/// exclusive systems and regular systems).
pub fn build_area_context(
    area_id: &str,
    areas: &Query<&ContextArea>,
    entities: &Query<
        (
            &EntityId,
            &EntityState,
            &Transform,
            &InArea,
            Option<&InteractionList>,
        ),
        Without<Npc>,
    >,
    npcs: &Query<
        (
            &EntityId,
            &EntityState,
            &Transform,
            &InArea,
            &NpcPersonality,
            Option<&NpcCurrentAction>,
        ),
        With<Npc>,
    >,
    player: &Query<(&Transform, &InArea, Option<&PlayerInventory>), With<super::Player>>,
) -> Option<AreaContext> {
    // Find the area definition.
    let area_def = areas.iter().find(|a| a.id == area_id)?;

    // --- Collect plain entities ---
    let mut entity_snapshots: Vec<EntitySnapshot> = Vec::new();
    for (eid, estate, tf, in_area, interactions) in entities.iter() {
        if in_area.0.as_deref() != Some(area_id) {
            continue;
        }
        entity_snapshots.push(EntitySnapshot {
            id: eid.0.clone(),
            state: estate.0.clone(),
            position: discretise(tf.translation),
            interactions: interactions.map(|i| i.0.clone()).unwrap_or_default(),
        });
    }
    // Sort for deterministic hashing.
    entity_snapshots.sort_by(|a, b| a.id.cmp(&b.id));

    // --- Collect NPCs ---
    let mut npc_snapshots: Vec<NpcSnapshot> = Vec::new();
    for (eid, estate, tf, in_area, personality, action) in npcs.iter() {
        if in_area.0.as_deref() != Some(area_id) {
            continue;
        }
        npc_snapshots.push(NpcSnapshot {
            id: eid.0.clone(),
            name: personality.name.clone(),
            state: estate.0.clone(),
            position: discretise(tf.translation),
            current_action: action
                .map(|a| a.0.clone())
                .unwrap_or_else(|| "idle".to_string()),
        });
    }
    npc_snapshots.sort_by(|a, b| a.id.cmp(&b.id));

    // --- Player ---
    let player_snapshot = player.iter().find_map(|(tf, in_area, inv)| {
        if in_area.0.as_deref() != Some(area_id) {
            return None;
        }
        Some(PlayerSnapshot {
            position: discretise(tf.translation),
            inventory: inv.map(|i| i.0.clone()).unwrap_or_default(),
        })
    });

    Some(AreaContext {
        area_id: area_def.id.clone(),
        area_label: area_def.label.clone(),
        area_description: area_def.description.clone(),
        entities: entity_snapshots,
        npcs: npc_snapshots,
        player: player_snapshot,
    })
}

// ---------------------------------------------------------------------------
// Context hash
// ---------------------------------------------------------------------------

/// Compute a deterministic 64-bit hash of an [`AreaContext`].
///
/// Used for dirty-detection: if two consecutive hashes are equal the context
/// has not changed and no LLM query is needed.
pub fn context_hash(ctx: &AreaContext) -> u64 {
    let mut hasher = DefaultHasher::new();
    ctx.hash(&mut hasher);
    hasher.finish()
}

// ---------------------------------------------------------------------------
// Plugin (optional convenience)
// ---------------------------------------------------------------------------

/// Bevy plugin that registers the area-membership update system.
pub struct ContextAreaPlugin;

impl Plugin for ContextAreaPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, update_area_membership);
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn area_contains_inside() {
        let area = ContextArea {
            id: "test".into(),
            label: "Test".into(),
            description: String::new(),
            min: Vec2::new(-5.0, -5.0),
            max: Vec2::new(5.0, 5.0),
            adjacent_areas: vec![],
        };
        assert!(area.contains(Vec3::new(0.0, 10.0, 0.0))); // Y ignored
        assert!(area.contains(Vec3::new(-5.0, 0.0, -5.0))); // on edge
        assert!(area.contains(Vec3::new(5.0, 0.0, 5.0))); // on edge
    }

    #[test]
    fn area_contains_outside() {
        let area = ContextArea {
            id: "test".into(),
            label: "Test".into(),
            description: String::new(),
            min: Vec2::new(-5.0, -5.0),
            max: Vec2::new(5.0, 5.0),
            adjacent_areas: vec![],
        };
        assert!(!area.contains(Vec3::new(5.1, 0.0, 0.0)));
        assert!(!area.contains(Vec3::new(0.0, 0.0, -5.1)));
    }

    #[test]
    fn discretise_deterministic() {
        assert_eq!(discretise(Vec3::new(1.0, 2.0, 3.0)), [10, 20, 30]);
        assert_eq!(discretise(Vec3::new(-0.55, 0.0, 1.24)), [-6, 0, 12]);
        // Same input always gives same output.
        let a = discretise(Vec3::new(3.14, 0.0, -2.71));
        let b = discretise(Vec3::new(3.14, 0.0, -2.71));
        assert_eq!(a, b);
    }

    #[test]
    fn context_hash_deterministic() {
        let ctx = AreaContext {
            area_id: "courtyard".into(),
            area_label: "Village Courtyard".into(),
            area_description: "A courtyard.".into(),
            entities: vec![EntitySnapshot {
                id: "barrel_01".into(),
                state: "intact".into(),
                position: [10, 0, 30],
                interactions: vec!["examine".into()],
            }],
            npcs: vec![],
            player: Some(PlayerSnapshot {
                position: [0, 10, 0],
                inventory: vec!["iron_key".into()],
            }),
        };
        let h1 = context_hash(&ctx);
        let h2 = context_hash(&ctx);
        assert_eq!(h1, h2);
    }

    #[test]
    fn context_hash_changes_on_mutation() {
        let ctx1 = AreaContext {
            area_id: "courtyard".into(),
            area_label: "Village Courtyard".into(),
            area_description: "A courtyard.".into(),
            entities: vec![EntitySnapshot {
                id: "barrel_01".into(),
                state: "intact".into(),
                position: [10, 0, 30],
                interactions: vec!["examine".into()],
            }],
            npcs: vec![],
            player: None,
        };
        let mut ctx2 = ctx1.clone();
        ctx2.entities[0].state = "broken".into();
        assert_ne!(context_hash(&ctx1), context_hash(&ctx2));
    }
}
