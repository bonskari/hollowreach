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
    llm, npc_look_at::NpcLookAt, pause_menu, static_collision_aabbs, AnimationSources,
    CircleCollider, Interactable, NpcPersonality, Player,
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
    /// Give an item to a target entity, with optional dialogue.
    Give { target: Entity, item_id: String, text: String },
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
}

impl Default for NpcDecisionState {
    fn default() -> Self {
        Self {
            current_action: None,
            last_context_hash: 0,
        }
    }
}

/// Marker: player is interacting with this NPC — stop all AI actions.
/// Removed when the interaction panel closes.
#[derive(Component)]
pub struct NpcInteracting;

/// Marker: this NPC has been activated by player interaction.
/// NPCs stay idle until the player interacts with them for the first time.
#[derive(Component)]
pub struct NpcActivated;

/// Marker: this NPC has a pending LLM decision request.
#[derive(Component)]
pub struct NpcPendingDecision;

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
                // All NPC AI only in Playing state
                npc_queue_init_system,
                npc_anim_graph_init_system,
                npc_decision_system
                    .after(npc_queue_init_system)
                    .run_if(pause_menu::game_not_paused),
                npc_decision_poll_system
                    .after(npc_decision_system)
                    .run_if(pause_menu::game_not_paused),
                npc_execute_system
                    .after(npc_decision_poll_system)
                    .run_if(pause_menu::game_not_paused),
                npc_pathfinding_system
                    .after(npc_execute_system)
                    .run_if(pause_menu::game_not_paused),
                npc_animation_system
                    .after(npc_execute_system)
                    .after(npc_anim_graph_init_system)
                    .run_if(pause_menu::game_not_paused),
            ).run_if(in_state(crate::GameState::Playing)),
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

    // Player position excluded from hash — player movement alone should
    // not trigger NPC re-decisions. NPCs react to entity state changes
    // and NPC movements, not to the player walking around.

    // Hash entity states — only changes to the world (not positions)
    // should trigger new NPC decisions.
    for (entity, _transform, interactable) in others.iter() {
        if entity == npc_entity {
            continue;
        }
        entity.to_bits().hash(&mut hasher);
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
/// current NPC is idle and its cooldown expired, sends an LLM decision request.
pub fn npc_decision_system(
    mut commands: Commands,
    mut queue: ResMut<NpcTurnQueue>,
    mut npcs: Query<
        (Entity, &Transform, &mut NpcDecisionState, Has<NpcInteracting>, Has<NpcPendingDecision>, Has<NpcActivated>),
        With<NpcBrain>,
    >,
    others: Query<(Entity, &Transform, Option<&Interactable>), Without<Player>>,
    player_query: Query<&Transform, With<Player>>,
    interactable_query: Query<(Entity, &Interactable, &Transform)>,
    personality_q: Query<&NpcPersonality>,
    npc_inv_q: Query<&crate::inventory::NpcInventory>,
    npc_mem_q: Query<&crate::npc_memory::NpcMemory>,
    llm_engine: Option<Res<llm::LlmEngine>>,
) {
    if queue.queue.is_empty() {
        return;
    }

    let Some(current_entity) = queue.current() else {
        return;
    };

    let Ok((npc_entity, npc_tf, mut state, is_interacting, has_pending, is_activated)) =
        npcs.get_mut(current_entity)
    else {
        queue.remove(current_entity);
        return;
    };

    if is_interacting || has_pending || !is_activated {
        queue.advance();
        return;
    }

    // Hash check: only request a new decision when the world has changed
    // since this NPC's last decision. This prevents spamming the same
    // Speak action endlessly when idle.
    let ctx_hash = build_context_hash(npc_entity, npc_tf, &others, &player_query);
    if ctx_hash == state.last_context_hash {
        // Nothing changed — if busy, keep doing current action; if idle, stay idle.
        queue.advance();
        return;
    }

    // Context changed — cancel current action and re-evaluate.
    state.current_action = None;
    state.last_context_hash = ctx_hash;

    // --- Send LLM decision request ---
    if let Some(ref engine) = llm_engine {
        if engine.ready.load(std::sync::atomic::Ordering::SeqCst) {
            let inv = npc_inv_q.get(npc_entity).map(|i| i.items.clone()).unwrap_or_default();
            let mem = npc_mem_q.get(npc_entity).map(|m| m.format_for_prompt(10)).unwrap_or_default();
            let npc_ctx = if let Ok(p) = personality_q.get(npc_entity) {
                llm::NpcContext {
                    name: p.name.clone(),
                    role: p.role.clone(),
                    traits: p.traits.clone(),
                    backstory: p.backstory.clone(),
                    speech_style: p.speech_style.clone(),
                    knowledge: p.knowledge.clone(),
                    goals: p.goals.clone(),
                    likes: p.likes.clone(),
                    dislikes: p.dislikes.clone(),
                    inventory: inv,
                    memories: mem,
                }
            } else {
                // No personality — skip
                queue.advance();
                return;
            };

            // Build surroundings info
            let npc_pos = npc_tf.translation;
            let scan_radius = 15.0_f32;

            let mut nearby = Vec::new();
            for (entity, interactable, tf) in &interactable_query {
                if entity == npc_entity {
                    continue;
                }
                let dist = npc_pos.distance(tf.translation);
                if dist < scan_radius {
                    nearby.push(llm::NearbyEntity {
                        name: interactable.name.clone(),
                        entity_type: if interactable.is_npc {
                            "npc".into()
                        } else {
                            "object".into()
                        },
                        distance: dist,
                        entity,
                    });
                }
            }

            let player_distance = player_query
                .single()
                .ok()
                .map(|pt| npc_pos.distance(pt.translation));

            engine.request_decision(llm::DecisionRequest {
                npc: npc_ctx,
                surroundings: llm::SurroundingsInfo {
                    nearby_entities: nearby,
                    player_distance,
                },
                npc_entity,
            });

            commands.entity(npc_entity).insert(NpcPendingDecision);
        }
    }

    queue.advance();
}

/// Polls LLM for completed NPC decisions and assigns the resulting actions.
pub fn npc_decision_poll_system(
    mut commands: Commands,
    llm_engine: Option<Res<llm::LlmEngine>>,
    mut npcs: Query<&mut NpcDecisionState, With<NpcBrain>>,
    interactable_q: Query<(Entity, &Interactable)>,
    player_q: Query<Entity, With<Player>>,
) {
    let Some(engine) = llm_engine else { return };

    while let Some(response) = engine.poll_decision() {
        commands
            .entity(response.npc_entity)
            .remove::<NpcPendingDecision>();

        let Ok(mut state) = npcs.get_mut(response.npc_entity) else {
            continue;
        };

        let action = match response.action {
            llm::LlmAction::Idle => NpcAction::Idle,
            llm::LlmAction::Speak(text) => NpcAction::Speak(text),
            llm::LlmAction::SpeakTo { target_name, text } => {
                // Find target entity by name
                if let Some((entity, _)) = interactable_q
                    .iter()
                    .find(|(_, i)| i.name.eq_ignore_ascii_case(&target_name))
                {
                    NpcAction::SpeakTo {
                        target: entity,
                        text,
                    }
                } else {
                    NpcAction::Speak(text)
                }
            }
            llm::LlmAction::MoveTo { target_name } => {
                if let Some((entity, _)) = interactable_q
                    .iter()
                    .find(|(_, i)| i.name.eq_ignore_ascii_case(&target_name))
                {
                    NpcAction::MoveTo(entity)
                } else {
                    NpcAction::Idle
                }
            }
            llm::LlmAction::Give { target_name, item_id, text } => {
                // "wanderer" or similar → give to player
                let target = if target_name.to_lowercase().contains("wanderer") {
                    player_q.single().ok()
                } else {
                    interactable_q
                        .iter()
                        .find(|(_, i)| i.name.eq_ignore_ascii_case(&target_name))
                        .map(|(e, _)| e)
                };
                if let Some(target) = target {
                    NpcAction::Give { target, item_id, text }
                } else {
                    NpcAction::Idle
                }
            }
        };

        state.current_action = Some(action);
    }
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
    mut chat_events: MessageWriter<crate::chat_log::PushChatMessage>,
    mut give_events: MessageWriter<crate::inventory::GiveItemEvent>,
    target_transforms: Query<&Transform, Without<NpcBrain>>,
    player_q: Query<Entity, With<Player>>,
) {
    for (npc_entity, mut state, npc_tf, interactable, mut look_at, is_interacting) in &mut npcs {
        // Player is interacting with this NPC — freeze all actions
        if is_interacting {
            state.current_action = None;
            continue;
        }
        let Some(ref action) = state.current_action else {
            continue;
        };

        match action {
            NpcAction::Speak(_text) => {
                // NPC talking to themselves — no chat output. Silent action.
                state.current_action = None;
            }

            NpcAction::SpeakTo { target, text } => {
                // Speaking TO someone — show in chat.
                if let Some(ref mut la) = look_at {
                    la.target = Some(*target);
                }
                let speaker = interactable.map(|i| i.name.as_str()).unwrap_or("NPC");
                chat_events.write(crate::chat_log::PushChatMessage {
                    speaker: speaker.to_string(),
                    text: text.clone(),
                });
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

            NpcAction::Give { target, item_id, .. } => {
                if let Ok(target_tf) = target_transforms.get(*target) {
                    let dist = npc_tf.translation.xz().distance(target_tf.translation.xz());
                    if dist < 1.5 {
                        give_events.write(crate::inventory::GiveItemEvent {
                            from: npc_entity,
                            to: *target,
                            item_id: item_id.clone(),
                        });

                        let speaker = interactable.map(|i| i.name.as_str()).unwrap_or("NPC");
                        if let NpcAction::Give { text, .. } = action {
                            if !text.is_empty() {
                                chat_events.write(crate::chat_log::PushChatMessage {
                                    speaker: speaker.to_string(),
                                    text: text.clone(),
                                });
                            }
                        }
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
    mut players_q: Query<(&mut AnimationPlayer, Option<&AnimationGraphHandle>)>,
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

        // Recursively find the first AnimationPlayer descendant.
        fn find_anim_player<'a>(
            start: &Children,
            children_q: &'a Query<&Children>,
            has_player: &dyn Fn(Entity) -> bool,
        ) -> Option<Entity> {
            let mut stack: Vec<Entity> = start.iter().collect();
            while let Some(e) = stack.pop() {
                if has_player(e) {
                    return Some(e);
                }
                if let Ok(grandchildren) = children_q.get(e) {
                    stack.extend(grandchildren.iter());
                }
            }
            None
        }

        let has_player = |e: Entity| players_q.get(e).is_ok();
        let Some(player_entity) = find_anim_player(npc_children, &children_q, &has_player) else {
            continue;
        };

        // Only play once the correct graph handle is on the player.
        // On the first call we insert the handle; on the next frame play() works.
        if let Ok((mut player, maybe_handle)) = players_q.get_mut(player_entity) {
            let handle_matches = maybe_handle
                .map(|h| h.0 == *graph_handle)
                .unwrap_or(false);

            if !handle_matches {
                commands.entity(player_entity)
                    .insert(AnimationGraphHandle(graph_handle.clone()));
                // Don't play yet — graph isn't attached this frame.
                continue;
            }

            player.stop_all();
            let active = player.play(node_index);
            active.repeat();
        }

        // Mark which animation is playing
        commands.entity(npc_entity).insert(desired);
    }
}
