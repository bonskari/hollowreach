//! NPC decision loop, action execution, and simple pathfinding.
//!
//! This module implements the autonomous NPC behavior system described in
//! TECHNICAL_PLAN.md sections 3.2–3.4 and 6. NPCs take turns in a round-robin
//! queue; each turn the NPC scans its surroundings, decides on an action (for
//! now via placeholder random logic — LLM integration comes later), and then
//! executes that action over subsequent frames.

use bevy::prelude::*;
use std::collections::VecDeque;
use std::hash::{Hash, Hasher};

use crate::{
    npc_look_at::NpcLookAt, pause_menu, static_collision_aabbs, AnimationSources, CircleCollider,
    Interactable, Player,
};

// ---------------------------------------------------------------------------
// Data types
// ---------------------------------------------------------------------------

/// An action an NPC can decide to perform.
#[derive(Clone, Debug, PartialEq)]
pub enum NpcAction {
    /// Walk to and interact with `entity` using the named `interaction_id`.
    Interact {
        entity: Entity,
        interaction_id: String,
    },
    /// Walk toward a target entity.
    MoveTo(Entity),
    /// Speak a line aloud (no specific target).
    Speak(String),
    /// Speak a line directed at a specific entity.
    SpeakTo { target: Entity, text: String },
    /// Give an item to a target entity.
    Give { target: Entity, item_id: String },
    /// Do nothing.
    Idle,
}

// ---------------------------------------------------------------------------
// Components
// ---------------------------------------------------------------------------

/// Marks an entity as an autonomous NPC managed by the decision loop.
#[derive(Component)]
pub struct NpcBrain;

/// Per-NPC decision state.
#[derive(Component)]
pub struct NpcDecisionState {
    /// The action currently being executed, if any.
    pub current_action: Option<NpcAction>,
    /// Hash of the last context snapshot — used for dirty detection.
    pub last_context_hash: u64,
    /// Cooldown between decision ticks.
    pub cooldown: Timer,
}

impl Default for NpcDecisionState {
    fn default() -> Self {
        Self {
            current_action: None,
            last_context_hash: 0,
            cooldown: Timer::from_seconds(3.0, TimerMode::Once),
        }
    }
}

/// Marker: player is interacting with this NPC — stop all AI actions.
/// Removed when the interaction panel closes.
#[derive(Component)]
pub struct NpcInteracting;

/// Walk speed for NPC movement (units per second).
#[derive(Component)]
pub struct NpcWalkSpeed(pub f32);

/// Tracks which animation is currently playing on this NPC, to avoid re-setting every frame.
#[derive(Component, PartialEq, Eq, Clone, Copy)]
pub enum NpcCurrentAnim {
    Idle,
    Walk,
}

/// Cached animation graphs — built once when clips are found.
#[derive(Resource)]
pub struct NpcAnimGraphs {
    pub idle_graph: Handle<AnimationGraph>,
    pub idle_node: AnimationNodeIndex,
    pub walk_graph: Handle<AnimationGraph>,
    pub walk_node: AnimationNodeIndex,
}

impl Default for NpcWalkSpeed {
    fn default() -> Self {
        Self(2.0)
    }
}

// ---------------------------------------------------------------------------
// Resources
// ---------------------------------------------------------------------------

/// Round-robin queue that tracks which NPC should decide next.
#[derive(Resource, Default)]
pub struct NpcTurnQueue {
    /// Ordered queue of NPC entities.
    pub queue: VecDeque<Entity>,
    /// Index of the NPC whose turn it is.
    pub current_index: usize,
    /// Whether the queue has been populated from the world.
    pub initialized: bool,
}

impl NpcTurnQueue {
    /// Advance to the next NPC in the queue, wrapping around.
    pub fn advance(&mut self) {
        if !self.queue.is_empty() {
            self.current_index = (self.current_index + 1) % self.queue.len();
        }
    }

    /// The entity whose turn it currently is, if any.
    pub fn current(&self) -> Option<Entity> {
        self.queue.get(self.current_index).copied()
    }

    /// Remove a despawned entity from the queue.
    pub fn remove(&mut self, entity: Entity) {
        if let Some(pos) = self.queue.iter().position(|&e| e == entity) {
            self.queue.remove(pos);
            if self.current_index >= self.queue.len() && !self.queue.is_empty() {
                self.current_index = 0;
            }
        }
    }
}

/// Placeholder dialogue lines NPCs can randomly choose from.
const NPC_IDLE_LINES: &[&str] = &[
    "Hmm, quiet today...",
    "I wonder what lies beyond these walls.",
    "Something doesn't feel right.",
    "The shadows grow longer each evening.",
    "I could use a drink.",
    "These old stones hold many secrets.",
    "Did you hear that?",
    "Best stay alert.",
];

// ---------------------------------------------------------------------------
// Plugin
// ---------------------------------------------------------------------------

/// Registers all NPC AI systems and resources.
pub struct NpcAiPlugin;

impl Plugin for NpcAiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<NpcTurnQueue>().add_systems(
            Update,
            (
                npc_queue_init_system,
                npc_anim_graph_init_system,
                npc_decision_system
                    .after(npc_queue_init_system)
                    .run_if(pause_menu::game_not_paused),
                npc_execute_system
                    .after(npc_decision_system)
                    .run_if(pause_menu::game_not_paused),
                npc_pathfinding_system
                    .after(npc_execute_system)
                    .run_if(pause_menu::game_not_paused),
                npc_animation_system
                    .after(npc_execute_system)
                    .after(npc_anim_graph_init_system)
                    .run_if(pause_menu::game_not_paused),
            ),
        );
    }
}

// ---------------------------------------------------------------------------
// Systems
// ---------------------------------------------------------------------------

/// One-shot system that populates the turn queue the first time NPC brains
/// are detected.  Also inserts default components if missing.
pub fn npc_queue_init_system(
    mut commands: Commands,
    mut queue: ResMut<NpcTurnQueue>,
    new_npcs: Query<Entity, (With<NpcBrain>, Without<NpcDecisionState>)>,
) {
    for entity in &new_npcs {
        commands
            .entity(entity)
            .insert((NpcDecisionState::default(), NpcWalkSpeed::default()));
        queue.queue.push_back(entity);
    }
    if !queue.queue.is_empty() {
        queue.initialized = true;
    }
}

/// Build a crude context hash from the positions, states, and identities of
/// nearby entities so we can skip redundant decisions when nothing changed.
fn build_context_hash(
    npc_entity: Entity,
    npc_transform: &Transform,
    others: &Query<(Entity, &Transform, Option<&Interactable>), Without<Player>>,
    player_query: &Query<&Transform, With<Player>>,
) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    let mut hasher = DefaultHasher::new();

    // Player position (quantised to 0.5-unit grid to reduce noise).
    if let Ok(pt) = player_query.single() {
        let px = (pt.translation.x * 2.0).round() as i32;
        let pz = (pt.translation.z * 2.0).round() as i32;
        px.hash(&mut hasher);
        pz.hash(&mut hasher);
    }

    // All entities within a generous radius.
    let scan_radius_sq = 15.0_f32 * 15.0;
    for (entity, transform, interactable) in others.iter() {
        if entity == npc_entity {
            continue;
        }
        let dist_sq = npc_transform
            .translation
            .distance_squared(transform.translation);
        if dist_sq > scan_radius_sq {
            continue;
        }
        // Identity.
        entity.to_bits().hash(&mut hasher);
        // Quantised position.
        let qx = (transform.translation.x * 2.0).round() as i32;
        let qz = (transform.translation.z * 2.0).round() as i32;
        qx.hash(&mut hasher);
        qz.hash(&mut hasher);
        // Interactable name as a proxy for "state" until we have EntityState.
        if let Some(inter) = interactable {
            inter.name.hash(&mut hasher);
        }
    }

    let mut hasher_out = [0u8; 8];
    let h = hasher.finish();
    hasher_out.copy_from_slice(&h.to_le_bytes());
    u64::from_le_bytes(hasher_out)
}

/// Core decision system.  On each frame, ticks cooldowns and, when the
/// current NPC is idle and its cooldown expired, generates a new action.
///
/// Current logic is placeholder random — will be replaced by LLM queries.
pub fn npc_decision_system(
    time: Res<Time>,
    mut queue: ResMut<NpcTurnQueue>,
    mut npcs: Query<(Entity, &Transform, &mut NpcDecisionState, Has<NpcInteracting>), With<NpcBrain>>,
    others: Query<(Entity, &Transform, Option<&Interactable>), Without<Player>>,
    player_query: Query<&Transform, With<Player>>,
    interactable_query: Query<(Entity, &Interactable, &Transform)>,
) {
    if queue.queue.is_empty() {
        return;
    }

    let Some(current_entity) = queue.current() else {
        return;
    };

    // Tick cooldown for the current NPC.
    let Ok((npc_entity, npc_tf, mut state, is_interacting)) = npcs.get_mut(current_entity) else {
        // Entity may have despawned — remove and advance.
        queue.remove(current_entity);
        return;
    };

    // Player is interacting — skip this NPC's turn
    if is_interacting {
        queue.advance();
        return;
    }

    state.cooldown.tick(time.delta());

    // Only decide when idle and cooldown finished.
    if state.current_action.is_some() || !state.cooldown.is_finished() {
        return;
    }

    // --- Context dirty check ---
    let ctx_hash = build_context_hash(npc_entity, npc_tf, &others, &player_query);
    if ctx_hash == state.last_context_hash {
        // Nothing changed — stay idle, advance turn.
        queue.advance();
        state.cooldown.reset();
        return;
    }
    state.last_context_hash = ctx_hash;

    // --- Placeholder random decision ---
    // Deterministic-ish seed from entity + frame so behaviour varies.
    let seed = npc_entity
        .to_bits()
        .wrapping_add((time.elapsed_secs() * 100.0) as u64);
    let roll = seed % 100;

    let action = if roll < 50 {
        // 50 %: idle
        NpcAction::Idle
    } else if roll < 80 {
        // 30 %: move to a random interactable entity
        let candidates: Vec<(Entity, &Interactable, &Transform)> = interactable_query
            .iter()
            .filter(|(e, _, _)| *e != npc_entity)
            .collect();
        if candidates.is_empty() {
            NpcAction::Idle
        } else {
            let idx = (seed / 7) as usize % candidates.len();
            NpcAction::MoveTo(candidates[idx].0)
        }
    } else {
        // 20 %: speak a random line
        let idx = (seed / 13) as usize % NPC_IDLE_LINES.len();
        NpcAction::Speak(NPC_IDLE_LINES[idx].to_string())
    };

    state.current_action = Some(action);

    // Reset cooldown so the NPC won't re-decide immediately after
    // finishing this action.
    state.cooldown.reset();
    queue.advance();
}

/// Execute the current action.  For `Speak` actions the text is shown via
/// the panel system's dialogue content.
pub fn npc_execute_system(
    mut npcs: Query<
        (
            Entity,
            &mut NpcDecisionState,
            &Transform,
            Option<&Interactable>,
            Option<&mut NpcLookAt>,
            Has<NpcInteracting>,
        ),
        With<NpcBrain>,
    >,
    mut panel_commands: MessageWriter<crate::panel::PanelCommand>,
    target_transforms: Query<&Transform, Without<NpcBrain>>,
) {
    for (_entity, mut state, npc_tf, interactable, mut look_at, is_interacting) in &mut npcs {
        // Player is interacting with this NPC — freeze all actions
        if is_interacting {
            state.current_action = None;
            continue;
        }
        let Some(ref action) = state.current_action else {
            continue;
        };

        match action {
            NpcAction::Speak(text) | NpcAction::SpeakTo { text, .. } => {
                // Update look-at target when speaking to a specific entity.
                if let NpcAction::SpeakTo { target, .. } = action {
                    if let Some(ref mut la) = look_at {
                        la.target = Some(*target);
                    }
                }

                // Display dialogue using panel system.
                let speaker = interactable.map(|i| i.name.as_str()).unwrap_or("NPC");
                let full = format!("{speaker}: \"{text}\"");

                panel_commands.write(crate::panel::PanelCommand {
                    action: crate::panel::PanelAction::Open(crate::panel::PanelContent::Dialogue {
                        speaker: speaker.to_string(),
                        text: full,
                    }),
                });

                // Speak is instant — clear action.
                state.current_action = None;
            }

            NpcAction::MoveTo(target) => {
                // Check if we've arrived (handled by pathfinding system).
                if let Ok(target_tf) = target_transforms.get(*target) {
                    let dist = npc_tf.translation.xz().distance(target_tf.translation.xz());
                    if dist < 1.5 {
                        // Arrived — switch to idle.
                        state.current_action = None;
                    }
                } else {
                    // Target gone — abort.
                    state.current_action = None;
                }
            }

            NpcAction::Interact { entity, .. } => {
                // For now, interactions are not yet wired to the reaction
                // system.  Just walk to the entity; once arrived, go idle.
                if let Ok(target_tf) = target_transforms.get(*entity) {
                    let dist = npc_tf.translation.xz().distance(target_tf.translation.xz());
                    if dist < 1.5 {
                        state.current_action = None;
                    }
                } else {
                    state.current_action = None;
                }
            }

            NpcAction::Give { target, .. } => {
                // Inventory system not yet implemented — just clear.
                if let Ok(target_tf) = target_transforms.get(*target) {
                    let dist = npc_tf.translation.xz().distance(target_tf.translation.xz());
                    if dist < 1.5 {
                        state.current_action = None;
                    }
                } else {
                    state.current_action = None;
                }
            }

            NpcAction::Idle => {
                // Nothing to do — clear immediately.
                state.current_action = None;
            }
        }
    }
}

/// Simple direct-line pathfinding with wall collision avoidance.
///
/// Moves NPCs toward their target position when they have a `MoveTo`,
/// `Interact`, or `Give` action.  Uses the same static collision AABBs as the
/// player and also respects `CircleCollider` components on other entities.
///
/// No navmesh yet — NPCs walk in a straight line and slide along walls.
pub fn npc_pathfinding_system(
    time: Res<Time>,
    mut npcs: Query<(Entity, &mut Transform, &NpcDecisionState, &NpcWalkSpeed, Has<NpcInteracting>), With<NpcBrain>>,
    target_transforms: Query<&Transform, Without<NpcBrain>>,
    collider_query: Query<(Entity, &Transform, &CircleCollider), Without<NpcBrain>>,
) {
    let static_aabbs = static_collision_aabbs();
    let dt = time.delta_secs();

    for (npc_entity, mut npc_tf, state, speed, is_interacting) in &mut npcs {
        if is_interacting {
            continue;
        }
        let target_entity = match &state.current_action {
            Some(NpcAction::MoveTo(e)) => *e,
            Some(NpcAction::Interact { entity, .. }) => *entity,
            Some(NpcAction::Give { target, .. }) => *target,
            _ => continue,
        };

        let Ok(target_tf) = target_transforms.get(target_entity) else {
            continue;
        };

        let target_pos = target_tf.translation;
        let current_pos = npc_tf.translation;

        // Direction on XZ plane.
        let diff = Vec3::new(
            target_pos.x - current_pos.x,
            0.0,
            target_pos.z - current_pos.z,
        );
        let dist = diff.length();
        if dist < 0.1 {
            continue; // Close enough — execute system will clear action.
        }

        let dir = diff / dist;
        let step = speed.0 * dt;
        let move_dist = step.min(dist); // Don't overshoot.
        let mut new_pos = current_pos + dir * move_dist;

        // Keep Y unchanged (ground level).
        new_pos.y = current_pos.y;

        // --- Collision: static AABBs (walls) ---
        let npc_radius = 0.5;
        for aabb in &static_aabbs {
            new_pos = aabb.push_out_circle_xz(new_pos, npc_radius);
        }

        // --- Collision: dynamic circle colliders (other entities) ---
        for (col_entity, col_tf, collider) in &collider_query {
            if col_entity == npc_entity {
                continue;
            }
            let col_pos = col_tf.translation;
            let min_dist = collider.radius + npc_radius;
            let d = Vec2::new(new_pos.x - col_pos.x, new_pos.z - col_pos.z);
            let d_len = d.length();
            if d_len < min_dist && d_len > 0.001 {
                // Push out.
                let push = (min_dist - d_len) * d / d_len;
                new_pos.x += push.x;
                new_pos.z += push.y;
            }
        }

        // Face movement direction.
        let face_dir = (new_pos - current_pos).normalize_or_zero();
        if face_dir.length_squared() > 0.001 {
            let angle = face_dir.x.atan2(face_dir.z);
            npc_tf.rotation = Quat::from_rotation_y(angle);
        }

        npc_tf.translation = new_pos;
    }
}

/// One-shot system: builds NpcAnimGraphs resource once walk+idle clips are available.
pub fn npc_anim_graph_init_system(
    mut commands: Commands,
    existing: Option<Res<NpcAnimGraphs>>,
    gltf_assets: Res<Assets<Gltf>>,
    anim_sources: Res<AnimationSources>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
) {
    if existing.is_some() {
        return;
    }

    let mut walk_clip: Option<Handle<AnimationClip>> = None;
    let mut idle_clip: Option<Handle<AnimationClip>> = None;

    for handle in &anim_sources.handles {
        if let Some(gltf) = gltf_assets.get(handle) {
            if walk_clip.is_none() {
                if let Some(clip) = gltf.named_animations.get("Walk") {
                    walk_clip = Some(clip.clone());
                }
            }
            if idle_clip.is_none() {
                for name in &["Idle_A", "Idle_B", "Idle_Loop"] {
                    if let Some(clip) = gltf.named_animations.get(*name) {
                        idle_clip = Some(clip.clone());
                        break;
                    }
                }
            }
        }
    }

    let (Some(walk), Some(idle)) = (walk_clip, idle_clip) else { return };

    let (walk_graph, walk_node) = AnimationGraph::from_clip(walk);
    let (idle_graph, idle_node) = AnimationGraph::from_clip(idle);

    commands.insert_resource(NpcAnimGraphs {
        walk_graph: graphs.add(walk_graph),
        walk_node,
        idle_graph: graphs.add(idle_graph),
        idle_node,
    });
}

/// Switches NPC animations between Walk and Idle based on their current action.
/// Only changes animation when state transitions (not every frame).
pub fn npc_animation_system(
    anim_graphs: Option<Res<NpcAnimGraphs>>,
    mut npcs: Query<(Entity, &NpcDecisionState, &Children, Option<&NpcCurrentAnim>), With<NpcBrain>>,
    children_q: Query<&Children>,
    mut animation_players: Query<(Entity, &mut AnimationPlayer)>,
    mut commands: Commands,
) {
    let Some(graphs) = anim_graphs else { return };

    for (npc_entity, state, npc_children, current_anim) in &mut npcs {
        let is_walking = matches!(
            &state.current_action,
            Some(NpcAction::MoveTo(_))
                | Some(NpcAction::Interact { .. })
                | Some(NpcAction::Give { .. })
        );

        let desired = if is_walking { NpcCurrentAnim::Walk } else { NpcCurrentAnim::Idle };

        // Skip if already playing the right animation
        if current_anim == Some(&desired) {
            continue;
        }

        let (graph_handle, node_index) = match desired {
            NpcCurrentAnim::Walk => (&graphs.walk_graph, graphs.walk_node),
            NpcCurrentAnim::Idle => (&graphs.idle_graph, graphs.idle_node),
        };

        // Find AnimationPlayer in children/grandchildren
        fn find_anim_player(
            children: &Children,
            children_q: &Query<&Children>,
            players: &Query<(Entity, &mut AnimationPlayer)>,
        ) -> Option<Entity> {
            for child in children.iter() {
                if players.get(child).is_ok() {
                    return Some(child);
                }
                if let Ok(grandchildren) = children_q.get(child) {
                    for gc in grandchildren.iter() {
                        if players.get(gc).is_ok() {
                            return Some(gc);
                        }
                    }
                }
            }
            None
        }

        let Some(player_entity) = find_anim_player(npc_children, &children_q, &animation_players) else {
            continue;
        };

        // Set the graph handle on the entity and play the animation
        commands.entity(player_entity).insert(AnimationGraphHandle(graph_handle.clone()));
        if let Ok((_, mut player)) = animation_players.get_mut(player_entity) {
            let active = player.play(node_index);
            active.repeat();
        }

        // Mark which animation is playing
        commands.entity(npc_entity).insert(desired);
    }
}
