pub use bevy::color::palettes::css;
pub use bevy::input::mouse::MouseMotion;
pub use bevy::prelude::*;
pub use bevy::ui::widget::NodeImageMode;
pub use bevy::window::CursorGrabMode;
use bevy::window::CursorOptions;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::asset::RenderAssetUsages;
use serde::Deserialize;
use std::collections::HashMap;
use std::f32::consts::PI;

pub mod chat_log;
pub mod context;
pub mod debug_overlay;
pub mod interactions;
pub mod inventory;
pub mod llm;
pub mod npc_ai;
pub mod npc_look_at;
pub mod npc_memory;
pub mod panel;
pub mod pause_menu;
pub mod text_input;
pub mod tts;
pub mod ui_helpers;

// --- Components ---

#[derive(Component)]
pub struct Player;

#[derive(Component)]
pub struct PlayerCamera {
    pub pitch: f32,
    pub yaw: f32,
}

#[derive(Component)]
pub struct Interactable {
    pub name: String,
    pub dialogue: String,
    pub is_npc: bool,
}

// --- JSON Config Structs ---

#[derive(Debug, Clone, Deserialize)]
pub struct ColliderConfig {
    #[serde(rename = "type")]
    pub collider_type: String,
    pub radius: f32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Condition {
    #[serde(rename = "type")]
    pub condition_type: String,
    #[serde(default)]
    pub state: Option<String>,
    #[serde(default)]
    pub item: Option<String>,
    #[serde(default)]
    pub flag: Option<String>,
    #[serde(default)]
    pub entity: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ReactionEffect {
    #[serde(rename = "type")]
    pub effect_type: String,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub asset: Option<String>,
    #[serde(default)]
    pub anim: Option<String>,
    #[serde(default)]
    pub new_state: Option<String>,
    #[serde(default)]
    pub item: Option<String>,
    #[serde(default)]
    pub flag: Option<String>,
    #[serde(default)]
    pub prompt: Option<String>,
    #[serde(default)]
    pub enabled: Option<bool>,
    #[serde(default)]
    pub entity: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Interaction {
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub conditions: Vec<Condition>,
    pub reaction: serde_json::Value,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UsePositionConfig {
    pub id: String,
    pub offset: [f32; 3],
    pub rotation_y: f32,
    pub actor_animation: String,
    #[serde(default)]
    pub enter_animation: Option<String>,
    #[serde(default)]
    pub exit_animation: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PersonalityConfig {
    pub name: String,
    pub role: String,
    #[serde(default)]
    pub traits: Vec<String>,
    #[serde(default)]
    pub backstory: String,
    #[serde(default)]
    pub speech_style: String,
    #[serde(default)]
    pub voice_profile: String,
    #[serde(default)]
    pub knowledge: Vec<String>,
    #[serde(default)]
    pub goals: Vec<String>,
    #[serde(default)]
    pub likes: Vec<String>,
    #[serde(default)]
    pub dislikes: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct EntityConfig {
    pub id: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(rename = "type")]
    pub entity_type: String,
    pub model: String,
    #[serde(default)]
    pub position: Option<[f32; 3]>,
    #[serde(default)]
    pub rotation_y: Option<f32>,
    #[serde(default)]
    pub state: Option<String>,
    #[serde(default)]
    pub collider: Option<ColliderConfig>,
    #[serde(default)]
    pub interactions: Vec<Interaction>,
    #[serde(default)]
    pub use_positions: Vec<UsePositionConfig>,
    // NPC-specific fields
    #[serde(default)]
    pub personality: Option<PersonalityConfig>,
    #[serde(default)]
    pub inventory: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AreaBounds {
    pub min: [f32; 2],
    pub max: [f32; 2],
}

#[derive(Debug, Clone, Deserialize)]
pub struct AreaConfig {
    pub id: String,
    pub label: String,
    pub bounds: AreaBounds,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub adjacent_areas: Vec<String>,
    #[serde(default)]
    pub ambient_sound: Option<String>,
}

// --- New ECS Components ---

/// Unique identifier for a data-driven entity.
#[derive(Component, Debug, Clone)]
pub struct EntityId(pub String);

/// Current state of a data-driven entity (e.g. "locked", "open", "idle").
#[derive(Component, Debug, Clone)]
pub struct EntityState(pub String);

/// List of interactions loaded from JSON config.
#[derive(Component, Debug, Clone)]
pub struct InteractionList(pub Vec<Interaction>);

/// NPC personality data loaded from JSON.
#[derive(Component, Debug, Clone)]
pub struct NpcPersonality {
    pub name: String,
    pub role: String,
    pub traits: Vec<String>,
    pub backstory: String,
    pub speech_style: String,
    pub voice_profile: String,
    pub knowledge: Vec<String>,
    pub goals: Vec<String>,
    pub likes: Vec<String>,
    pub dislikes: Vec<String>,
}

// --- Config Resources ---

/// All loaded entity configs, keyed by ID.
#[derive(Resource, Default)]
pub struct EntityConfigs(pub HashMap<String, EntityConfig>);

/// All loaded area configs, keyed by ID.
#[derive(Resource, Default)]
pub struct AreaConfigs(pub HashMap<String, AreaConfig>);

#[derive(Resource)]
pub struct MouseSensitivity(pub f32);

impl Default for MouseSensitivity {
    fn default() -> Self {
        Self(0.003)
    }
}

// --- Collision types ---

/// Simple axis-aligned bounding box for collision detection.
pub struct Aabb {
    pub min: Vec3,
    pub max: Vec3,
}

impl Aabb {
    /// Create an AABB from a center position and half-extents.
    pub fn from_center_half_extents(center: Vec3, half_extents: Vec3) -> Self {
        Self {
            min: center - half_extents,
            max: center + half_extents,
        }
    }

    /// Check if a point (on the XZ plane) is inside this AABB, treating the player as a
    /// small circle of the given radius.
    pub fn overlaps_circle_xz(&self, pos: Vec3, radius: f32) -> bool {
        pos.x + radius > self.min.x
            && pos.x - radius < self.max.x
            && pos.z + radius > self.min.z
            && pos.z - radius < self.max.z
    }

    /// Push a circle (pos + radius) out of this AABB along the axis of least penetration.
    pub fn push_out_circle_xz(&self, pos: Vec3, radius: f32) -> Vec3 {
        if !self.overlaps_circle_xz(pos, radius) {
            return pos;
        }

        // Compute penetration depth on each axis
        let pen_pos_x = (self.max.x + radius) - pos.x; // push in +x direction
        let pen_neg_x = pos.x - (self.min.x - radius); // push in -x direction
        let pen_pos_z = (self.max.z + radius) - pos.z; // push in +z direction
        let pen_neg_z = pos.z - (self.min.z - radius); // push in -z direction

        let min_pen_x = pen_pos_x.min(pen_neg_x);
        let min_pen_z = pen_pos_z.min(pen_neg_z);

        let mut result = pos;
        if min_pen_x < min_pen_z {
            if pen_pos_x < pen_neg_x {
                result.x = self.max.x + radius;
            } else {
                result.x = self.min.x - radius;
            }
        } else if pen_pos_z < pen_neg_z {
            result.z = self.max.z + radius;
        } else {
            result.z = self.min.z - radius;
        }

        result
    }
}

/// Returns the list of static collision AABBs in the scene.
pub fn static_collision_aabbs() -> Vec<Aabb> {
    vec![
        // Back wall (north)
        Aabb::from_center_half_extents(Vec3::new(0.0, 2.0, -10.0), Vec3::new(10.5, 2.0, 0.5)),
        // Left wall (west)
        Aabb::from_center_half_extents(Vec3::new(-10.0, 2.0, 0.0), Vec3::new(0.5, 2.0, 10.5)),
        // Right wall (east)
        Aabb::from_center_half_extents(Vec3::new(10.0, 2.0, 0.0), Vec3::new(0.5, 2.0, 10.5)),
        // South wall left section
        Aabb::from_center_half_extents(Vec3::new(-6.0, 2.0, 10.0), Vec3::new(4.5, 2.0, 0.5)),
        // South wall right section
        Aabb::from_center_half_extents(Vec3::new(6.0, 2.0, 10.0), Vec3::new(4.5, 2.0, 0.5)),
        // Center decorated pillar
        Aabb::from_center_half_extents(Vec3::new(0.0, 2.0, -1.0), Vec3::new(0.75, 2.0, 0.75)),
    ]
}

// --- UI Components ---

/// Marker for the proximity hint text (bottom center).
#[derive(Component)]
pub struct ProximityHintText;

/// Set to true when a panel close system consumes the Escape key this frame.
/// Prevents toggle_pause from also firing on the same Escape press.
#[derive(Resource, Default)]
pub struct EscapeConsumed(pub bool);

// --- Reusable UI animation components ---

/// Fade in: opacity goes from 0 to 1 over duration. Optional delay before starting.
#[derive(Component)]
pub struct UiFadeIn {
    pub elapsed: f32,
    pub delay: f32,
    pub duration: f32,
}

/// Fade out: opacity goes from 1 to 0. Optionally despawn when done.
#[derive(Component)]
pub struct UiFadeOut {
    pub elapsed: f32,
    pub delay: f32,
    pub duration: f32,
    pub despawn: bool,
}

/// Slide-up bounce animation for UI elements.
#[derive(Component)]
pub struct UiSlideIn {
    pub elapsed: f32,
    pub duration: f32,
    pub start_offset: f32,
}

/// Horizontal scale-in animation: width scales from 0% to 100%.
#[derive(Component)]
pub struct UiScaleIn {
    pub elapsed: f32,
    pub duration: f32,
}

/// Horizontal scale-out animation: width scales from 100% to 0%.
/// When finished, the parent AnimatedPanel is set to Hidden.
#[derive(Component)]
pub struct UiScaleOut {
    pub elapsed: f32,
    pub duration: f32,
}

/// Marker added to an AnimatedPanel to request a dismiss transition.
/// The panel_appear_animation_system will start UiScaleOut, then hide
/// the panel once the animation completes.
#[derive(Component)]
pub struct DismissPanel;

/// Cinematic intro screen — "This is" then "Hollowreach" with fade in/out.
#[derive(Resource)]
#[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum GameState {
    #[default]
    Loading,
    Menu,
    Playing,
}

#[derive(Resource)]
pub struct IntroSequence {
    pub elapsed: f32,
    pub active: bool,
}

impl Default for IntroSequence {
    fn default() -> Self {
        Self { elapsed: 0.0, active: true }
    }
}

// Intro timing (seconds):
// 0.0-0.5: black
// 0.5-1.5: "This is" fades in
// 1.2-2.2: "Hollowreach" fades in
// 2.5-3.5: hold
// 3.5-4.5: both fade out
// 4.5-5.0: black fades out to gameplay
// 5.0: done

/// Marker for the intro overlay (fullscreen black).
#[derive(Component)]
pub struct IntroOverlay;

/// Marker for "This is" text.
#[derive(Component)]
pub struct IntroTextTop;

/// Marker for "Hollowreach" text.
#[derive(Component)]
pub struct IntroTextTitle;

// --- Audio ---

/// Global audio volume settings.
/// All audio spawns multiply their base volume by the relevant category × master.
#[derive(Resource, Clone, Debug)]
pub struct AudioSettings {
    /// Master volume multiplier (0.0–1.0).
    pub master_volume: f32,
    /// Music / ambient volume (0.0–1.0).
    pub music_volume: f32,
    /// Sound effects volume (0.0–1.0) — footsteps, impacts, interaction sounds.
    pub sfx_volume: f32,
    /// TTS / NPC speech volume (0.0–1.0).
    pub speech_volume: f32,
}

impl Default for AudioSettings {
    fn default() -> Self {
        Self {
            master_volume: 0.8,
            music_volume: 0.5,
            sfx_volume: 0.7,
            speech_volume: 1.0,
        }
    }
}

impl AudioSettings {
    /// Effective SFX volume (sfx × master).
    pub fn effective_sfx(&self) -> f32 {
        self.sfx_volume * self.master_volume
    }
    /// Effective music volume (music × master).
    pub fn effective_music(&self) -> f32 {
        self.music_volume * self.master_volume
    }
    /// Effective speech volume (speech × master).
    pub fn effective_speech(&self) -> f32 {
        self.speech_volume * self.master_volume
    }
}

/// Resource that tracks the ambient background audio entity.
#[derive(Resource, Default)]
pub struct AmbientAudio {
    pub entity: Option<Entity>,
}

/// Marker for the cinematic intro sound effect.
#[derive(Component)]
pub struct IntroSfx;

/// Tracks whether the intro sound has been triggered.
#[derive(Resource)]
pub struct IntroSfxState {
    pub played: bool,
    pub sound: Option<Handle<AudioSource>>,
}

impl Default for IntroSfxState {
    fn default() -> Self {
        Self { played: false, sound: None }
    }
}

// --- Interaction cooldown ---

/// Resource that tracks a cooldown timer between interactions.
#[derive(Resource)]
pub struct InteractionCooldown(pub Timer);

impl Default for InteractionCooldown {
    fn default() -> Self {
        let mut t = Timer::from_seconds(1.0, TimerMode::Once);
        // Start finished so the first interaction works immediately
        t.tick(std::time::Duration::from_secs(2));
        Self(t)
    }
}

// --- Collision ---

/// Circle collider on the XZ plane. Attach to any entity that the player shouldn't walk through.
#[derive(Component)]
pub struct CircleCollider {
    pub radius: f32,
}

// --- Footstep Audio ---

/// Resource holding footstep sound handles per surface type.
#[derive(Resource)]
pub struct FootstepAudio {
    pub stone: Vec<Handle<AudioSource>>,
    pub grass: Vec<Handle<AudioSource>>,
    pub wood: Vec<Handle<AudioSource>>,
    pub dirt: Vec<Handle<AudioSource>>,
    pub timer: Timer,
    pub last_index: usize,
}

// --- Constants ---

pub const INTERACT_DISTANCE: f32 = 3.5;
/// Max angle (radians) between look direction and entity to count as "looking at".
/// ~25 degrees — tight enough that you must actually aim at the target.
pub const INTERACT_ANGLE: f32 = 0.44;
pub const PLAYER_RADIUS: f32 = 0.4;

/// Returns the entity the player is looking at (within distance and angle), if any.
/// Uses camera yaw + pitch to build a look vector, then checks dot product against
/// each interactable entity's direction from the player.
/// Entity positions are adjusted upward (+0.8m) to approximate center-of-mass
/// since model origins are typically at ground level.
pub fn find_looked_at_entity(
    player_pos: Vec3,
    camera: &PlayerCamera,
    candidates: impl Iterator<Item = (Entity, Vec3)>,
) -> Option<(Entity, f32)> {
    // Camera world position (player + camera offset)
    let cam_pos = player_pos + Vec3::new(0.0, 0.6, 0.0);

    // Build the camera's world-space forward vector from yaw + pitch
    let yaw_rot = Quat::from_rotation_y(camera.yaw);
    let pitch_rot = Quat::from_rotation_x(camera.pitch);
    let look_dir = (yaw_rot * pitch_rot * Vec3::NEG_Z).normalize();

    let mut best: Option<(Entity, f32, f32)> = None; // (entity, dist, angle)

    for (entity, entity_pos) in candidates {
        // Aim at approximate center of entity, not ground-level origin
        let target_pos = entity_pos + Vec3::new(0.0, 0.8, 0.0);
        let to_entity = target_pos - cam_pos;
        let dist = to_entity.length();
        if dist < 0.1 || dist > INTERACT_DISTANCE {
            continue;
        }
        let dir_to_entity = to_entity / dist;
        let angle = look_dir.dot(dir_to_entity).clamp(-1.0, 1.0).acos();
        if angle > INTERACT_ANGLE {
            continue;
        }
        // Prefer closer + more centered
        if best.is_none() || angle < best.unwrap().2 || (angle - best.unwrap().2).abs() < 0.05 && dist < best.unwrap().1 {
            best = Some((entity, dist, angle));
        }
    }

    best.map(|(e, d, _)| (e, d))
}

// --- Plugin ---

pub struct HollowreachPlugin;

impl Plugin for HollowreachPlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<GameState>()
            .init_resource::<AudioSettings>()
            .init_resource::<MouseSensitivity>()
            .init_resource::<InteractionCooldown>()
            .init_resource::<AnimationSources>()
            .init_resource::<IntroSequence>()
            .init_resource::<AmbientAudio>()
            .init_resource::<IntroSfxState>()
            .init_resource::<EntityConfigs>()
            .init_resource::<AreaConfigs>()
            .init_resource::<EscapeConsumed>()
            .add_systems(First, |mut esc: ResMut<EscapeConsumed>| { esc.0 = false; })
            .add_systems(Startup, (
                ui_helpers::setup_ui_assets,
                load_entity_configs,
                setup_scene,
                grab_cursor,
                setup_ui,
                setup_intro,
                load_footstep_audio,
            ).chain())
            .add_systems(Startup, spawn_from_configs.after(load_entity_configs).after(setup_scene))
            // All gameplay systems only run in Playing state
            .add_systems(
                Update,
                (
                    player_movement
                        .run_if(pause_menu::game_not_paused)
                        .run_if(text_input::text_input_not_active)
                        .run_if(panel::panel_not_blocking_gameplay),
                    player_collision
                        .after(player_movement)
                        .run_if(pause_menu::game_not_paused),
                    player_look
                        .run_if(pause_menu::game_not_paused)
                        .run_if(text_input::text_input_not_active)
                        .run_if(panel::panel_not_blocking_gameplay),
                    interact_system
                        .run_if(pause_menu::game_not_paused)
                        .run_if(text_input::text_input_not_active)
                        .run_if(panel::panel_not_blocking_gameplay),
                    proximity_hint_system.run_if(pause_menu::game_not_paused),
                    interaction_scroll_system
                        .run_if(pause_menu::game_not_paused)
                        .run_if(text_input::text_input_not_active)
                        .run_if(panel::panel_not_blocking_gameplay),
                    footstep_sound_system
                        .run_if(pause_menu::game_not_paused)
                        .run_if(text_input::text_input_not_active)
                        .run_if(panel::panel_not_blocking_gameplay),
                    start_npc_animations.run_if(pause_menu::game_not_paused),
                    hide_unwanted_meshes,
                ).run_if(in_state(GameState::Playing)),
            )
            .add_systems(
                Update,
                (
                    ui_slide_in_system,
                    ui_fade_in_system,
                    ui_fade_out_system,
                    intro_system.run_if(pause_menu::game_not_paused),
                    intro_sfx_system.run_if(pause_menu::game_not_paused),
                    handle_say_event.run_if(pause_menu::game_not_paused),
                    llm_dialogue_poll_system.run_if(pause_menu::game_not_paused),
                    show_text_event_system.run_if(pause_menu::game_not_paused),
                    ui_helpers::button_hover_system,
                ).run_if(in_state(GameState::Playing)),
            )
            .add_plugins(debug_overlay::DebugOverlayPlugin)
            .add_plugins(inventory::InventoryPlugin)
            .add_plugins(npc_ai::NpcAiPlugin)
            .add_plugins(text_input::TextInputPlugin)
            .add_plugins(pause_menu::PauseMenuPlugin)
            .add_plugins(npc_look_at::NpcLookAtPlugin)
            .add_plugins(tts::TtsPlugin)
            .add_plugins(llm::LlmPlugin)
            .add_plugins(npc_memory::NpcMemoryPlugin)
            .add_plugins(chat_log::ChatLogPlugin)
            .add_plugins(interactions::InteractionPlugin)
            .add_plugins(context::ContextAreaPlugin)
            .add_plugins(panel::PanelPlugin)
            .add_systems(Update, (
                panel_appear_animation_system
                    .after(panel::panel_command_system)
                    .after(panel::panel_transition_system),
                panel_dismiss_animation_system
                    .after(panel::panel_command_system),
                ui_scale_in_system,
                ui_scale_out_system,
            ))
            .add_systems(Startup, spawn_context_areas.after(load_entity_configs))
            .add_systems(Startup, populate_entity_id_map.after(spawn_from_configs));
    }
}

/// Hides any entity whose name starts with `_hidden`.
/// Use this naming convention in Blender to mark meshes that should not render in-game.
#[derive(Component)]
pub struct MeshHidden;

pub fn hide_unwanted_meshes(
    mut commands: Commands,
    named_entities: Query<(Entity, &Name), Without<MeshHidden>>,
) {
    for (entity, name) in &named_entities {
        if name.as_str().starts_with("_hidden") {
            commands.entity(entity).insert((Visibility::Hidden, MeshHidden));
        }
    }
}


/// Marker to track that we've already started animations on this entity.
#[derive(Component)]
pub struct AnimationStarted;

/// Stores GLTF handles for animation lookup.
#[derive(Resource, Default)]
pub struct AnimationSources {
    pub handles: Vec<Handle<Gltf>>,
}

/// System that finds newly loaded AnimationPlayers and starts an idle animation.
/// Searches for "Idle_A", "Idle_B", or "Idle_Loop" in any loaded GLTF.
pub fn start_npc_animations(
    mut commands: Commands,
    mut animation_players: Query<(Entity, &mut AnimationPlayer), Without<AnimationStarted>>,
    gltf_assets: Res<Assets<Gltf>>,
    anim_sources: Res<AnimationSources>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
) {
    if animation_players.is_empty() {
        return;
    }

    let idle_names = ["Idle_A", "Idle_B", "Idle_Loop"];
    let mut idle_clip_handle = None;
    for handle in &anim_sources.handles {
        if let Some(gltf) = gltf_assets.get(handle) {
            for name in &idle_names {
                if let Some(clip) = gltf.named_animations.get(*name) {
                    idle_clip_handle = Some(clip.clone());
                    break;
                }
            }
            if idle_clip_handle.is_some() {
                break;
            }
        }
    }

    let Some(clip) = idle_clip_handle else { return };

    let (graph, idle_index) = AnimationGraph::from_clip(clip);
    let graph_handle = graphs.add(graph);

    for (entity, mut player) in &mut animation_players {
        commands.entity(entity).insert((
            AnimationStarted,
            AnimationGraphHandle(graph_handle.clone()),
        ));
        let active = player.play(idle_index);
        active.repeat();
        active.set_speed(1.0);
    }
}

// --- Config Loading ---

/// Reads all JSON files from assets/data/entities/, assets/data/npcs/, and assets/data/areas/
/// and stores them in EntityConfigs and AreaConfigs resources.
pub fn load_entity_configs(
    mut entity_configs: ResMut<EntityConfigs>,
    mut area_configs: ResMut<AreaConfigs>,
) {
    let data_dirs = [
        ("assets/data/entities", false),
        ("assets/data/npcs", false),
        ("assets/data/areas", true),
    ];

    for (dir_path, is_area) in &data_dirs {
        let dir = match std::fs::read_dir(dir_path) {
            Ok(d) => d,
            Err(e) => {
                warn!("Could not read directory {}: {}", dir_path, e);
                continue;
            }
        };

        for entry in dir {
            let entry = match entry {
                Ok(e) => e,
                Err(_) => continue,
            };
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }

            let contents = match std::fs::read_to_string(&path) {
                Ok(c) => c,
                Err(e) => {
                    warn!("Could not read {}: {}", path.display(), e);
                    continue;
                }
            };

            if *is_area {
                match serde_json::from_str::<AreaConfig>(&contents) {
                    Ok(config) => {
                        info!("Loaded area config: {}", config.id);
                        area_configs.0.insert(config.id.clone(), config);
                    }
                    Err(e) => {
                        warn!("Failed to parse area config {}: {}", path.display(), e);
                    }
                }
            } else {
                match serde_json::from_str::<EntityConfig>(&contents) {
                    Ok(config) => {
                        info!("Loaded entity config: {}", config.id);
                        entity_configs.0.insert(config.id.clone(), config);
                    }
                    Err(e) => {
                        warn!("Failed to parse entity config {}: {}", path.display(), e);
                    }
                }
            }
        }
    }

    info!(
        "Loaded {} entity configs, {} area configs",
        entity_configs.0.len(),
        area_configs.0.len()
    );
}

/// Spawns Bevy entities from all loaded EntityConfigs.
pub fn spawn_from_configs(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    entity_configs: Res<EntityConfigs>,
    mut anim_sources: ResMut<AnimationSources>,
) {
    // Track which character GLBs we've already registered as animation sources
    let mut registered_anim_sources: std::collections::HashSet<String> = std::collections::HashSet::new();

    for config in entity_configs.0.values() {
        let pos = config.position.unwrap_or([0.0, 0.0, 0.0]);
        let rot_y = config.rotation_y.unwrap_or(0.0);
        let rot_y_rad = rot_y.to_radians();

        // Load the GLTF scene
        let scene_path = format!("{}#Scene0", config.model);
        let scene_root = SceneRoot(asset_server.load(&scene_path));

        let transform = Transform::from_xyz(pos[0], pos[1], pos[2])
            .with_rotation(Quat::from_rotation_y(rot_y_rad));

        let mut entity_cmd = commands.spawn((
            scene_root,
            transform,
            EntityId(config.id.clone()),
            EntityState(config.state.clone().unwrap_or_else(|| "default".to_string())),
            InteractionList(config.interactions.clone()),
            context::InArea::default(),
        ));

        // Add collider if present
        if let Some(ref collider) = config.collider {
            entity_cmd.insert(CircleCollider { radius: collider.radius });
        }

        // Build an Interactable for backward compatibility with the existing interact/proximity systems
        let is_npc = config.entity_type == "npc";
        if is_npc {
            let personality = config.personality.as_ref();
            let name = personality
                .map(|p| p.name.clone())
                .unwrap_or_else(|| config.id.clone());

            // Build a default dialogue from the first talk interaction's prompt, or a generic one
            let dialogue = config
                .interactions
                .iter()
                .find(|i| i.id == "talk")
                .and_then(|i| {
                    // reaction can be an array or object
                    if let Some(arr) = i.reaction.as_array() {
                        arr.iter()
                            .find(|r| r.get("type").and_then(|t| t.as_str()) == Some("dialogue_prompt"))
                            .and_then(|r| r.get("prompt").and_then(|p| p.as_str()))
                            .map(|s| s.to_string())
                    } else if let Some(obj) = i.reaction.as_object() {
                        obj.get("dialogue_prompt")
                            .and_then(|p| p.as_str())
                            .map(|s| s.to_string())
                    } else {
                        None
                    }
                })
                .unwrap_or_else(|| format!("{} has nothing to say.", name));

            entity_cmd.insert(Interactable {
                name: name.clone(),
                dialogue,
                is_npc: true,
            });

            // Add NPC-specific components
            if let Some(ref p) = config.personality {
                entity_cmd.insert((
                    NpcPersonality {
                        name: p.name.clone(),
                        role: p.role.clone(),
                        traits: p.traits.clone(),
                        backstory: p.backstory.clone(),
                        speech_style: p.speech_style.clone(),
                        voice_profile: p.voice_profile.clone(),
                        knowledge: p.knowledge.clone(),
                        goals: p.goals.clone(),
                        likes: p.likes.clone(),
                        dislikes: p.dislikes.clone(),
                    },
                    npc_look_at::NpcLookAt::default(),
                ));
            }

            entity_cmd.insert((
                inventory::NpcInventory { items: config.inventory.clone() },
                npc_ai::NpcBrain,
            ));

            // Register this character GLB as an animation source if not already done
            if !registered_anim_sources.contains(&config.model) {
                registered_anim_sources.insert(config.model.clone());
                let handle: Handle<Gltf> = asset_server.load(&config.model);
                anim_sources.handles.push(handle);
            }
        } else {
            // Non-NPC entities: add Interactable if they have interactions with info_text
            let first_interaction_text = config
                .interactions
                .iter()
                .find(|i| i.id == "examine" || i.id == "open")
                .and_then(|i| {
                    if let Some(arr) = i.reaction.as_array() {
                        arr.iter()
                            .find(|r| r.get("type").and_then(|t| t.as_str()) == Some("info_text"))
                            .and_then(|r| r.get("text").and_then(|t| t.as_str()))
                            .map(|s| s.to_string())
                    } else {
                        None
                    }
                });

            if let Some(text) = first_interaction_text {
                let display_name = config.name.clone()
                    .unwrap_or_else(|| config.id.replace('_', " "));

                entity_cmd.insert(Interactable {
                    name: display_name,
                    dialogue: text,
                    is_npc: false,
                });
            }
        }
    }

    info!("Spawned {} entities from configs", entity_configs.0.len());
}

/// Spawns ContextArea entities from all loaded AreaConfigs.
pub fn spawn_context_areas(
    mut commands: Commands,
    area_configs: Res<AreaConfigs>,
) {
    for config in area_configs.0.values() {
        commands.spawn(context::ContextArea {
            id: config.id.clone(),
            label: config.label.clone(),
            description: config.description.clone(),
            min: Vec2::new(config.bounds.min[0], config.bounds.min[1]),
            max: Vec2::new(config.bounds.max[0], config.bounds.max[1]),
            adjacent_areas: config.adjacent_areas.clone(),
        });
        info!("Spawned context area: {}", config.id);
    }
}

/// Populates the EntityIdMap resource so cross-entity effects can find targets.
pub fn populate_entity_id_map(
    query: Query<(Entity, &EntityId)>,
    mut entity_id_map: ResMut<interactions::EntityIdMap>,
) {
    for (entity, eid) in &query {
        entity_id_map.0.insert(eid.0.clone(), entity);
    }
    info!("EntityIdMap populated with {} entries", entity_id_map.0.len());
}

// --- Setup ---

pub fn setup_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    asset_server: Res<AssetServer>,
) {
    // Helper to spawn a dungeon piece
    let d = |path: &str, server: &AssetServer| -> SceneRoot {
        SceneRoot(server.load(format!("kaykit_dungeon/{path}#Scene0")))
    };

    // --- Ground ---
    let ground_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.35, 0.5, 0.25),
        perceptual_roughness: 0.95,
        ..default()
    });
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(60.0, 60.0).build())),
        MeshMaterial3d(ground_mat),
    ));

    // --- Player ---
    commands
        .spawn((
            Player,
            Transform::from_xyz(0.0, 1.0, 5.0),
            Visibility::default(),
            inventory::PlayerInventory::default(),
            context::InArea::default(),
        ))
        .with_children(|parent| {
            parent.spawn((
                PlayerCamera { pitch: 0.0, yaw: 0.0 },
                Camera3d::default(),
                Transform::from_xyz(0.0, 0.6, 0.0),
            ));
        });

    // =========================================================
    // VILLAGE LAYOUT — open courtyard with walls on three sides
    // Wall: 4w x 4h x 1d, Floor large: 4x4, Character: ~0.9h
    // Area: 20x20 (-10 to 10 on X and Z)
    // =========================================================

    // Back wall (north) — 5 segments covering 20 units
    for i in 0..5 {
        let x = -8.0 + i as f32 * 4.0;
        commands.spawn((d("wall.gltf", &asset_server), Transform::from_xyz(x, 0.0, -10.0)));
    }

    // Left wall (west) — 5 segments, rotated
    for i in 0..5 {
        let z = -8.0 + i as f32 * 4.0;
        commands.spawn((
            d("wall.gltf", &asset_server),
            Transform::from_xyz(-10.0, 0.0, z).with_rotation(Quat::from_rotation_y(PI / 2.0)),
        ));
    }

    // Right wall (east)
    for i in 0..5 {
        let z = -8.0 + i as f32 * 4.0;
        commands.spawn((
            d("wall.gltf", &asset_server),
            Transform::from_xyz(10.0, 0.0, z).with_rotation(Quat::from_rotation_y(-PI / 2.0)),
        ));
    }

    // South wall with doorway opening — walls on sides, gap in middle
    for i in 0..2 {
        let x = -8.0 + i as f32 * 4.0;
        commands.spawn((d("wall.gltf", &asset_server), Transform::from_xyz(x, 0.0, 10.0)));
    }
    for i in 3..5 {
        let x = -8.0 + i as f32 * 4.0;
        commands.spawn((d("wall.gltf", &asset_server), Transform::from_xyz(x, 0.0, 10.0)));
    }

    // Entrance pillars flanking the gap
    commands.spawn((d("pillar.gltf", &asset_server), Transform::from_xyz(-2.0, 0.0, 10.0)));
    commands.spawn((d("pillar.gltf", &asset_server), Transform::from_xyz(2.0, 0.0, 10.0)));

    // --- Floor: large tiles (4x4) covering the courtyard ---
    for xi in 0..5 {
        for zi in 0..5 {
            let x = -8.0 + xi as f32 * 4.0;
            let z = -8.0 + zi as f32 * 4.0;
            commands.spawn((
                d("floor_tile_large.gltf", &asset_server),
                Transform::from_xyz(x, 0.0, z),
            ));
        }
    }

    // --- Props, furniture, NPCs are now spawned from JSON configs ---
    // See spawn_from_configs system

    // --- Gabled roof (harjakatto) ---
    // Ridge runs along X axis (east-west), peaks at center
    // Building is 20 wide (X: -10..10) x 20 deep (Z: -10..10)
    // Walls are 4 units tall, ridge at ~7 units
    let roof_y_base = 4.0; // top of walls
    let roof_y_peak = 7.0; // ridge height
    let roof_half_w = 10.5; // slight overhang past walls
    let roof_len = 21.0; // slight overhang on ends
    let roof_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.45, 0.25, 0.15), // dark wood/thatch color
        perceptual_roughness: 0.95,
        cull_mode: None, // visible from both sides
        ..default()
    });

    // Build two sloped planes for the roof (procedural mesh)
    // South slope: from back wall ridge down to south wall top
    // North slope: from back wall ridge down to north wall top
    for side in [-1.0_f32, 1.0] {
        let mut positions = Vec::new();
        let mut normals = Vec::new();
        let mut uvs = Vec::new();
        let mut indices = Vec::new();

        // Quad: 4 vertices
        // Bottom edge (at wall top)
        let z_bottom = side * roof_half_w;
        let z_top = 0.0; // ridge at center
        positions.push([-roof_len / 2.0, roof_y_base, z_bottom]); // bottom-left
        positions.push([roof_len / 2.0, roof_y_base, z_bottom]);  // bottom-right
        positions.push([roof_len / 2.0, roof_y_peak, z_top]);     // top-right (ridge)
        positions.push([-roof_len / 2.0, roof_y_peak, z_top]);    // top-left (ridge)

        // Normal: perpendicular to the slope
        let edge_horizontal = Vec3::new(1.0, 0.0, 0.0);
        let edge_slope = Vec3::new(0.0, roof_y_peak - roof_y_base, z_top - z_bottom).normalize();
        let normal = edge_slope.cross(edge_horizontal).normalize();
        for _ in 0..4 {
            normals.push([normal.x, normal.y, normal.z]);
        }

        uvs.push([0.0, 0.0]);
        uvs.push([5.0, 0.0]);
        uvs.push([5.0, 2.0]);
        uvs.push([0.0, 2.0]);

        if side > 0.0 {
            indices.extend_from_slice(&[0, 1, 2, 0, 2, 3]);
        } else {
            indices.extend_from_slice(&[0, 2, 1, 0, 3, 2]);
        }

        let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
        mesh.insert_indices(Indices::U32(indices));

        commands.spawn((
            Mesh3d(meshes.add(mesh)),
            MeshMaterial3d(roof_mat.clone()),
        ));
    }

    // Triangular gable ends (east and west walls) — fill the gap between wall top and roof peak
    // Ridge runs along Z axis, gables are on X ends
    // Wait — actually ridge runs along X. Gables are at Z=±roof_half_w
    // The roof slopes from Z=±roof_half_w (at wall height) up to Z=0 (ridge at peak)
    // So gable ends are at X=±roof_len/2 — east and west ends
    let gable_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.5, 0.35, 0.2),
        perceptual_roughness: 0.9,
        cull_mode: None,
        ..default()
    });
    for &x in &[-roof_len / 2.0, roof_len / 2.0] {
        // Triangle: bottom-left at south wall, bottom-right at north wall, peak at ridge
        let positions = vec![
            [x, roof_y_base, -roof_half_w], // bottom south
            [x, roof_y_base, roof_half_w],  // bottom north
            [x, roof_y_peak, 0.0],          // peak (ridge)
        ];
        let normal = if x > 0.0 { [1.0, 0.0, 0.0] } else { [-1.0, 0.0, 0.0] };
        let normals = vec![normal; 3];
        let uvs = vec![[0.0, 0.0], [1.0, 0.0], [0.5, 1.0]];
        let indices = if x > 0.0 { vec![0u32, 2, 1] } else { vec![0u32, 1, 2] };

        let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
        mesh.insert_indices(Indices::U32(indices));

        commands.spawn((
            Mesh3d(meshes.add(mesh)),
            MeshMaterial3d(gable_mat.clone()),
        ));
    }

    // Ridge beam (wooden beam along the peak)
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(21.0, 0.3, 0.3))),
        MeshMaterial3d(gable_mat.clone()),
        Transform::from_xyz(0.0, roof_y_peak, 0.0),
    ));

    // Support beams (rafters visible from inside)
    let rafter_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.4, 0.25, 0.12),
        perceptual_roughness: 0.9,
        ..default()
    });
    for x in (-8..=8).step_by(4) {
        for &side in &[-1.0_f32, 1.0] {
            let z_bottom = side * roof_half_w;
            let rafter_len = ((roof_y_peak - roof_y_base).powi(2) + roof_half_w.powi(2)).sqrt();
            let angle = (roof_y_peak - roof_y_base).atan2(roof_half_w) * side;
            commands.spawn((
                Mesh3d(meshes.add(Cuboid::new(0.15, 0.15, rafter_len))),
                MeshMaterial3d(rafter_mat.clone()),
                Transform::from_xyz(
                    x as f32,
                    (roof_y_base + roof_y_peak) / 2.0,
                    z_bottom / 2.0,
                )
                .with_rotation(Quat::from_rotation_x(angle)),
            ));
        }
    }

    // --- Sky color ---
    commands.insert_resource(ClearColor(Color::srgb(0.45, 0.55, 0.75)));

    // --- Lighting ---
    commands.spawn(AmbientLight {
        color: Color::srgb(0.5, 0.55, 0.7),
        brightness: 80.0,
        affects_lightmapped_meshes: true,
    });

    // Sun
    commands.spawn((
        DirectionalLight {
            illuminance: 6000.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -PI / 3.5, PI / 5.0, 0.0)),
    ));

    // Interior warm light with shadows (hanging from ceiling)
    commands.spawn((
        PointLight {
            color: Color::srgb(1.0, 0.85, 0.6),
            intensity: 80000.0,
            range: 20.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(0.0, 5.5, 0.0),
    ));

    // Warm torch lights along walls
    for &(x, z) in &[
        (-9.4, -6.0), (-9.4, 0.0), (-9.4, 6.0),
        (9.4, -6.0), (9.4, 0.0), (9.4, 6.0),
        (-4.0, -9.4), (4.0, -9.4),
    ] {
        commands.spawn((
            PointLight {
                color: Color::from(css::ORANGE),
                intensity: 30000.0,
                range: 8.0,
                shadows_enabled: false,
                ..default()
            },
            Transform::from_xyz(x, 3.0, z),
        ));
    }

    println!("=== HOLLOWREACH ===");
    println!("Controls:");
    println!("  WASD  - Move");
    println!("  Mouse - Look around");
    println!("  E     - Interact");
    println!("  Esc   - Release cursor");
    println!("==================");
}

pub fn setup_ui(mut commands: Commands, ui: Res<ui_helpers::UiAssets>) {
    use ui_helpers::*;

    // Root UI container
    commands
        .spawn(Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            justify_content: JustifyContent::FlexEnd,
            align_items: AlignItems::Center,
            ..default()
        })
        .with_children(|parent| {
            // --- Proximity hint ---
            parent
                .spawn((
                    ProximityHintText,
                    Node {
                        position_type: PositionType::Absolute,
                        top: Val::Percent(55.0),
                        left: Val::Percent(0.0),
                        right: Val::Percent(0.0),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    Visibility::Hidden,
                ))
                .with_children(|hint_parent| {
                    hint_parent
                        .spawn((
                            panel_image_node(&ui),
                            Node {
                                padding: UiRect::axes(Val::Px(28.0), Val::Px(16.0)),
                                ..default()
                            },
                        ))
                        .with_children(|bg| {
                            bg.spawn((
                                Text::new(""),
                                TextFont { font_size: 15.0, ..default() },
                                TextColor(COLOR_GOLD),
                                TextLayout::new_with_justify(Justify::Center),
                            ));
                        });
                });
        });
}

pub fn setup_intro(mut commands: Commands, asset_server: Res<AssetServer>, mut sfx_state: ResMut<IntroSfxState>) {
    // Preload intro sound so it plays instantly
    sfx_state.sound = Some(asset_server.load("audio/cinematic/intro_impact.wav"));
    // Intro text container (transparent, over gameplay)
    commands
        .spawn((
            IntroOverlay,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                flex_direction: FlexDirection::Column,
                ..default()
            },
            GlobalZIndex(100),
        ))
        .with_children(|parent| {
            // "This is" — with shadow (dark text behind, offset 2px)
            parent
                .spawn(Node {
                    margin: UiRect::bottom(Val::Px(8.0)),
                    ..default()
                })
                .with_children(|wrapper| {
                    // Shadow
                    wrapper.spawn((
                        Text::new("This is"),
                        TextFont { font_size: 28.0, ..default() },
                        TextColor(Color::srgba(0.0, 0.0, 0.0, 0.0)),
                        Node {
                            position_type: PositionType::Absolute,
                            left: Val::Px(2.0),
                            top: Val::Px(2.0),
                            ..default()
                        },
                        UiFadeIn { elapsed: 0.0, delay: 0.5, duration: 1.0 },
                    ));
                    // Foreground
                    wrapper.spawn((
                        IntroTextTop,
                        Text::new("This is"),
                        TextFont { font_size: 28.0, ..default() },
                        TextColor(Color::srgba(0.9, 0.9, 0.9, 0.0)),
                        UiFadeIn { elapsed: 0.0, delay: 0.5, duration: 1.0 },
                    ));
                });

            // "Hollowreach" — instant appear at 1.5s (with impact sound)
            parent
                .spawn(Node::default())
                .with_children(|wrapper| {
                    // Shadow
                    wrapper.spawn((
                        Text::new("Hollowreach"),
                        TextFont { font_size: 56.0, ..default() },
                        TextColor(Color::srgba(0.0, 0.0, 0.0, 0.0)),
                        Node {
                            position_type: PositionType::Absolute,
                            left: Val::Px(3.0),
                            top: Val::Px(3.0),
                            ..default()
                        },
                        UiFadeIn { elapsed: 0.0, delay: 1.5, duration: 0.01 },
                    ));
                    // Foreground
                    wrapper.spawn((
                        IntroTextTitle,
                        Text::new("Hollowreach"),
                        TextFont { font_size: 56.0, ..default() },
                        TextColor(Color::srgba(0.95, 0.82, 0.4, 0.0)),
                        UiFadeIn { elapsed: 0.0, delay: 1.5, duration: 0.01 },
                    ));
                });
        });
}

pub fn intro_system(
    time: Res<Time>,
    mut intro: ResMut<IntroSequence>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut ambient_audio: ResMut<AmbientAudio>,
    overlay_q: Query<Entity, With<IntroOverlay>>,
    _text_q: Query<Entity, Or<(With<IntroTextTop>, With<IntroTextTitle>)>>,
    // Also fade shadows — all Text children of the overlay that have UiFadeIn
    fadeable_q: Query<Entity, With<UiFadeIn>>,
    audio_settings: Res<AudioSettings>,
) {
    if !intro.active {
        return;
    }
    intro.elapsed += time.delta_secs();

    // At 3.5s: fade out ALL text (including shadows) by switching UiFadeIn → UiFadeOut
    if intro.elapsed > 3.5 && intro.elapsed - time.delta_secs() <= 3.5 {
        for entity in &fadeable_q {
            commands.entity(entity).remove::<UiFadeIn>().insert(
                UiFadeOut { elapsed: 0.0, delay: 0.0, duration: 1.0, despawn: false },
            );
        }
    }

    // At 4.8s: despawn entire overlay (removes all children), start ambient audio
    if intro.elapsed > 4.8 && intro.elapsed - time.delta_secs() <= 4.8 {
        if let Ok(overlay) = overlay_q.single() {
            commands.entity(overlay).despawn();
        }

        // Start looping ambient audio (fire crackling in the tavern)
        let music_vol = audio_settings.effective_music();
        let ambient_entity = commands.spawn((
            AudioPlayer::<AudioSource>(asset_server.load("audio/ambient/village_ambient.wav")),
            PlaybackSettings {
                volume: bevy::audio::Volume::Linear(music_vol),
                ..PlaybackSettings::LOOP
            },
        )).id();
        ambient_audio.entity = Some(ambient_entity);

        intro.active = false;
    }
}

/// Plays a cinematic impact sound when "Hollowreach" title appears (at ~1.2s into the intro).
pub fn intro_sfx_system(
    intro: Res<IntroSequence>,
    mut sfx_state: ResMut<IntroSfxState>,
    mut commands: Commands,
    audio_settings: Res<AudioSettings>,
) {
    if !intro.active || sfx_state.played {
        return;
    }

    // Trigger the impact sound exactly when "Hollowreach" appears (1.5s)
    if intro.elapsed >= 1.5 {
        if let Some(sound) = sfx_state.sound.take() {
            let vol = audio_settings.effective_sfx();
            commands.spawn((
                IntroSfx,
                AudioPlayer::<AudioSource>(sound),
                PlaybackSettings {
                    volume: bevy::audio::Volume::Linear(vol),
                    ..PlaybackSettings::DESPAWN
                },
            ));
        }
        sfx_state.played = true;
    }
}

pub fn grab_cursor(mut cursor_q: Query<&mut CursorOptions>) {
    let mut cursor_opts = cursor_q.single_mut().unwrap();
    cursor_opts.grab_mode = CursorGrabMode::Locked;
    cursor_opts.visible = false;
}

// --- Player Systems ---

pub fn player_movement(
    keyboard: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    mut player_q: Query<&mut Transform, With<Player>>,
    camera_q: Query<&PlayerCamera>,
) {
    let mut transform = player_q.single_mut().unwrap();
    let camera = camera_q.single().unwrap();

    let speed = 5.0;
    let mut direction = Vec3::ZERO;

    let yaw_rot = Quat::from_rotation_y(camera.yaw);
    let forward = yaw_rot * Vec3::NEG_Z;
    let right = yaw_rot * Vec3::X;

    if keyboard.pressed(KeyCode::KeyW) {
        direction += forward;
    }
    if keyboard.pressed(KeyCode::KeyS) {
        direction -= forward;
    }
    if keyboard.pressed(KeyCode::KeyA) {
        direction -= right;
    }
    if keyboard.pressed(KeyCode::KeyD) {
        direction += right;
    }

    direction.y = 0.0;
    if direction.length_squared() > 0.0 {
        direction = direction.normalize();
    }

    transform.translation += direction * speed * time.delta_secs();
    transform.rotation = Quat::from_rotation_y(camera.yaw);
}

/// Collision system that runs after player_movement. Prevents the player from walking
/// through walls, collider entities, and clamps position to world boundaries.
pub fn player_collision(
    mut player_q: Query<&mut Transform, With<Player>>,
    colliders: Query<(&Transform, &CircleCollider), Without<Player>>,
) {
    let mut transform = player_q.single_mut().unwrap();
    let mut pos = transform.translation;

    // 1. Static collision against walls
    let aabbs = static_collision_aabbs();
    for aabb in &aabbs {
        pos = aabb.push_out_circle_xz(pos, PLAYER_RADIUS);
    }

    // 2. Dynamic circle colliders (NPCs, props, etc.)
    for (col_tf, collider) in &colliders {
        let col_pos = col_tf.translation;
        let min_dist = PLAYER_RADIUS + collider.radius;
        let dx = pos.x - col_pos.x;
        let dz = pos.z - col_pos.z;
        let dist_sq = dx * dx + dz * dz;
        if dist_sq < min_dist * min_dist && dist_sq > 0.0 {
            let dist = dist_sq.sqrt();
            let overlap = min_dist - dist;
            let nx = dx / dist;
            let nz = dz / dist;
            pos.x += nx * overlap;
            pos.z += nz * overlap;
        }
    }

    // 3. World boundary clamping
    pos.x = pos.x.clamp(-24.0, 24.0);
    pos.z = pos.z.clamp(-24.0, 24.0);

    transform.translation = pos;
}

pub fn player_look(
    mut mouse_motion: MessageReader<MouseMotion>,
    sensitivity: Res<MouseSensitivity>,
    mut camera_q: Query<(&mut PlayerCamera, &mut Transform)>,
) {

    let (mut camera, mut cam_transform) = camera_q.single_mut().unwrap();

    for ev in mouse_motion.read() {
        camera.yaw -= ev.delta.x * sensitivity.0;
        camera.pitch -= ev.delta.y * sensitivity.0;
        camera.pitch = camera.pitch.clamp(-PI / 2.0 + 0.05, PI / 2.0 - 0.05);
    }

    cam_transform.rotation = Quat::from_rotation_x(camera.pitch);
}

// --- Interaction ---

pub fn interact_system(
    keyboard: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    mut cooldown: ResMut<InteractionCooldown>,
    player_q: Query<(Entity, &Transform, Option<&inventory::PlayerInventory>), With<Player>>,
    camera_q: Query<&PlayerCamera>,
    interactable_q: Query<
        (Entity, &Transform, Option<&Interactable>, Option<&InteractionList>, Option<&EntityState>, Option<&EntityId>, Option<&NpcPersonality>),
        Without<Player>,
    >,
    mut look_at_q: Query<&mut npc_look_at::NpcLookAt>,
    mut commands: Commands,
    global_flags: Res<interactions::GlobalFlags>,
    mut panel_commands: MessageWriter<panel::PanelCommand>,
    mut tts_requests: MessageWriter<tts::TtsRequest>,
    llm_engine: Option<Res<llm::LlmEngine>>,
    npc_inv_q: Query<&inventory::NpcInventory>,
    npc_mem_q: Query<&npc_memory::NpcMemory>,
) {
    cooldown.0.tick(time.delta());

    if !keyboard.just_pressed(KeyCode::KeyE) {
        return;
    }
    if !cooldown.0.is_finished() {
        return;
    }

    let (player_entity, player_tf, player_inv) = player_q.single().unwrap();
    let camera = camera_q.single().unwrap();

    // Find the entity the player is looking at (distance + angle check)
    let candidates = interactable_q.iter().filter_map(|(entity, tf, opt_i, opt_l, _, _, _)| {
        if opt_i.is_some() || opt_l.is_some() { Some((entity, tf.translation)) } else { None }
    });
    let Some((target_entity, _)) = find_looked_at_entity(player_tf.translation, camera, candidates) else { return };

    let Ok((_, _, opt_interactable, opt_interaction_list, opt_entity_state, opt_entity_id, opt_personality)) =
        interactable_q.get(target_entity)
    else {
        return;
    };

    // Check if this is an NPC — if so, open the NPC interaction panel
    let is_npc = opt_interactable.is_some_and(|i| i.is_npc) || opt_personality.is_some();
    if is_npc {
        // Stop NPC movement and make them look at player
        commands.entity(target_entity).insert((npc_ai::NpcInteracting, npc_ai::NpcActivated));
        if let Ok(mut look_at) = look_at_q.get_mut(target_entity) {
            look_at.target = Some(player_entity);
        }

        let npc_name = opt_personality
            .map(|p| p.name.clone())
            .or_else(|| opt_interactable.map(|i| i.name.clone()))
            .unwrap_or_else(|| "Unknown".to_string());
        let npc_role = opt_personality
            .map(|p| p.role.clone())
            .unwrap_or_default();

        // No automatic greeting — NPC speaks only when player uses Say.
        let greeting = "";

        panel_commands.write(panel::PanelCommand {
            action: panel::PanelAction::Open(panel::PanelContent::NpcMenu {
                npc: target_entity,
                name: npc_name,
                role: npc_role,
                greeting: greeting.to_string(),
            }),
        });

        cooldown.0.reset();
        return;
    }

    // --- Non-NPC path: props and other interactables ---

    // Try the new InteractionList path first
    if let Some(interaction_list) = opt_interaction_list {
        let runtime_interactions = interactions::convert_interaction_list(interaction_list);
        let entity_state_str = opt_entity_state
            .map(|s| s.0.as_str())
            .unwrap_or("default");
        let actor_inventory: Vec<String> = player_inv
            .map(|inv| inv.items.clone())
            .unwrap_or_default();

        let all_entity_states: HashMap<String, String> = interactable_q
            .iter()
            .filter_map(|(_, _, _, _, opt_s, opt_eid, _)| {
                match (opt_eid, opt_s) {
                    (Some(eid), Some(state)) => Some((eid.0.clone(), state.0.clone())),
                    _ => None,
                }
            })
            .collect();

        let available = interactions::get_available_interactions(
            &runtime_interactions,
            entity_state_str,
            &actor_inventory,
            &global_flags.0,
            Some(&all_entity_states),
        );

        if !available.is_empty() {
            let prop_name = opt_interactable
                .map(|i| i.name.clone())
                .or_else(|| opt_entity_id.map(|eid| eid.0.clone()))
                .unwrap_or_else(|| "Object".to_string());
            let prop_subtitle = opt_entity_state
                .map(|s| s.0.clone())
                .unwrap_or_default();

            panel_commands.write(panel::PanelCommand {
                action: panel::PanelAction::Open(panel::PanelContent::PropMenu {
                    prop: target_entity,
                    name: prop_name,
                    subtitle: prop_subtitle,
                    interactions: available,
                }),
            });

            cooldown.0.reset();
            return;
        }
    }

    // Legacy fallback: Interactable entities without InteractionList
    if let Some(interactable) = opt_interactable {
        let legacy_interaction = interactions::Interaction {
            id: "examine".to_string(),
            label: "Examine".to_string(),
            conditions: vec![],
            reaction: vec![interactions::ReactionEffect {
                effect_type: interactions::ReactionEffectType::InfoText,
                text: Some(interactable.dialogue.clone()),
                anim: None,
                asset: None,
                new_state: None,
                entity: None,
                item: None,
                flag: None,
                prompt: None,
                enabled: None,
            }],
        };

        panel_commands.write(panel::PanelCommand {
            action: panel::PanelAction::Open(panel::PanelContent::PropMenu {
                prop: target_entity,
                name: interactable.name.clone(),
                subtitle: String::new(),
                interactions: vec![legacy_interaction],
            }),
        });

        cooldown.0.reset();
    }
}

/// Handles SayEvent — sends player text to LLM for NPC response.
pub fn handle_say_event(
    mut say_events: MessageReader<text_input::SayEvent>,
    mut chat_events: MessageWriter<chat_log::PushChatMessage>,
    llm_engine: Option<Res<llm::LlmEngine>>,
    personality_q: Query<&NpcPersonality>,
    npc_inv_q: Query<&inventory::NpcInventory>,
    npc_mem_q: Query<&npc_memory::NpcMemory>,
) {
    for event in say_events.read() {
        // Push player's text into chat log
        chat_events.write(chat_log::PushChatMessage {
            speaker: "You".to_string(),
            text: event.text.clone(),
        });

        // Send to LLM for NPC response
        if let Some(ref engine) = llm_engine {
            let inv = npc_inv_q.get(event.npc).map(|i| i.items.clone()).unwrap_or_default();
            let mem = npc_mem_q.get(event.npc).map(|m| m.format_for_prompt(10)).unwrap_or_default();
            let npc_ctx = if let Ok(p) = personality_q.get(event.npc) {
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
                    inventory: inv.clone(),
                    memories: mem.clone(),
                }
            } else {
                llm::NpcContext {
                    name: "NPC".into(),
                    role: "Villager".into(),
                    traits: vec![],
                    backstory: String::new(),
                    speech_style: String::new(),
                    knowledge: vec![],
                    goals: vec![],
                    likes: vec![],
                    dislikes: vec![],
                    inventory: inv,
                    memories: mem,
                }
            };

            engine.request_dialogue(llm::DialogueRequest {
                npc: npc_ctx,
                player_text: event.text.clone(),
                history: vec![],
                npc_entity: event.npc,
            });
        }
    }
}

/// Polls LLM for dialogue responses and shows them in the chat log + triggers TTS.
pub fn llm_dialogue_poll_system(
    llm_engine: Option<Res<llm::LlmEngine>>,
    mut chat_events: MessageWriter<chat_log::PushChatMessage>,
    mut tts_events: MessageWriter<tts::TtsRequest>,
    personality_q: Query<&NpcPersonality>,
) {
    let Some(engine) = llm_engine else { return };

    while let Some(response) = engine.poll_dialogue() {
        if response.text.is_empty() {
            continue;
        }

        let speaker = personality_q
            .get(response.npc_entity)
            .map(|p| p.name.clone())
            .unwrap_or_else(|_| "NPC".into());

        let voice = personality_q
            .get(response.npc_entity)
            .map(|p| p.voice_profile.clone())
            .unwrap_or_default();

        // Push into chat log (top of screen)
        chat_events.write(chat_log::PushChatMessage {
            speaker: speaker.clone(),
            text: response.text.clone(),
        });

        // Trigger TTS
        if !voice.is_empty() {
            tts_events.write(tts::TtsRequest {
                text: response.text,
                voice_profile: voice,
                npc_entity: response.npc_entity,
            });
        }
    }
}

/// Shows text from interaction effects (info_text, dialogue_prompt) via the panel system.
pub fn show_text_event_system(
    mut events: MessageReader<interactions::ShowTextEvent>,
    mut panel_commands: MessageWriter<panel::PanelCommand>,
) {
    for event in events.read() {
        panel_commands.write(panel::PanelCommand {
            action: panel::PanelAction::Open(panel::PanelContent::Dialogue {
                speaker: String::new(),
                text: event.text.clone(),
            }),
        });
    }
}

/// System that lets the player scroll through available interactions using
/// number keys (1-9) or mouse wheel when near an entity with multiple options.
pub fn interaction_scroll_system(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut scroll_events: MessageReader<bevy::input::mouse::MouseWheel>,
    mut selected: ResMut<interactions::SelectedInteraction>,
    player_q: Query<(Entity, &Transform, Option<&inventory::PlayerInventory>), With<Player>>,
    camera_q: Query<&PlayerCamera>,
    interactable_q: Query<
        (Entity, &Transform, Option<&InteractionList>, Option<&EntityState>, Option<&EntityId>),
        Without<Player>,
    >,
    global_flags: Res<interactions::GlobalFlags>,
) {
    let (_player_entity, player_tf, player_inv) = player_q.single().unwrap();
    let camera = camera_q.single().unwrap();

    // Find entity the player is looking at
    let candidates = interactable_q.iter().filter_map(|(entity, tf, list, _, _)| {
        list.map(|_| (entity, tf.translation))
    });
    let looked_at = find_looked_at_entity(player_tf.translation, camera, candidates);

    let Some((target_entity, _)) = looked_at else {
        if selected.target.is_some() {
            selected.target = None;
            selected.index = 0;
        }
        for _ in scroll_events.read() {}
        return;
    };

    if selected.target != Some(target_entity) {
        selected.target = Some(target_entity);
        selected.index = 0;
    }

    let available_count = if let Ok((_, _, Some(interaction_list), opt_state, _)) =
        interactable_q.get(target_entity)
    {
        let runtime = interactions::convert_interaction_list(interaction_list);
        let entity_state_str = opt_state.map(|s| s.0.as_str()).unwrap_or("default");
        let actor_inventory: Vec<String> = player_inv
            .map(|inv| inv.items.clone())
            .unwrap_or_default();
        let all_entity_states: HashMap<String, String> = interactable_q
            .iter()
            .filter_map(|(_, _, _, opt_s, opt_eid)| {
                match (opt_eid, opt_s) {
                    (Some(eid), Some(state)) => Some((eid.0.clone(), state.0.clone())),
                    _ => None,
                }
            })
            .collect();
        interactions::get_available_interactions(
            &runtime,
            entity_state_str,
            &actor_inventory,
            &global_flags.0,
            Some(&all_entity_states),
        )
        .len()
    } else {
        0
    };

    if available_count == 0 {
        for _ in scroll_events.read() {}
        return;
    }

    let number_keys = [
        KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3,
        KeyCode::Digit4, KeyCode::Digit5, KeyCode::Digit6,
        KeyCode::Digit7, KeyCode::Digit8, KeyCode::Digit9,
    ];
    for (i, key) in number_keys.iter().enumerate() {
        if keyboard.just_pressed(*key) && i < available_count {
            selected.index = i;
            return;
        }
    }

    for ev in scroll_events.read() {
        if ev.y > 0.0 {
            if selected.index > 0 {
                selected.index -= 1;
            }
        } else if ev.y < 0.0 {
            if selected.index + 1 < available_count {
                selected.index += 1;
            }
        }
    }
}

pub fn proximity_hint_system(
    player_q: Query<&Transform, With<Player>>,
    camera_q: Query<&PlayerCamera>,
    interactable_q: Query<
        (Entity, &Transform, Option<&Interactable>, Option<&InteractionList>, Option<&NpcPersonality>),
        Without<Player>,
    >,
    mut hint_q: Query<(&mut Visibility, &Children), With<ProximityHintText>>,
    children_q: Query<&Children>,
    mut text_q: Query<&mut Text>,
    panel_state: Res<panel::PanelState>,
) {
    // Hide hint while any panel content is showing or animating
    if panel_state.visual != panel::PanelVisual::Hidden {
        let (mut visibility, _) = hint_q.single_mut().unwrap();
        *visibility = Visibility::Hidden;
        return;
    }
    let player_tf = player_q.single().unwrap();
    let camera = camera_q.single().unwrap();

    // Find entity the player is looking at (distance + angle)
    let candidates = interactable_q.iter().filter_map(|(entity, tf, opt_i, opt_l, _)| {
        if opt_i.is_some() || opt_l.is_some() { Some((entity, tf.translation)) } else { None }
    });
    let looked_at = find_looked_at_entity(player_tf.translation, camera, candidates);

    let (mut visibility, children) = hint_q.single_mut().unwrap();

    if let Some((nearest_entity, _)) = looked_at {
        let Ok((_, _, opt_interactable, _, opt_personality)) = interactable_q.get(nearest_entity) else {
            *visibility = Visibility::Hidden;
            return;
        };

        let entity_name = opt_personality
            .map(|p| p.name.clone())
            .or_else(|| opt_interactable.map(|i| i.name.clone()))
            .unwrap_or_default();

        let hint_text = if entity_name.is_empty() {
            "Interact [E]".to_string()
        } else {
            format!("{}\nInteract [E]", entity_name)
        };

        {
            fn find_text(entity: Entity, children_q: &Query<&Children>, text_q: &mut Query<&mut Text>) -> Option<Entity> {
                if text_q.get(entity).is_ok() { return Some(entity); }
                if let Ok(kids) = children_q.get(entity) {
                    for kid in kids.iter() {
                        if let Some(found) = find_text(kid, children_q, text_q) { return Some(found); }
                    }
                }
                None
            }
            if let Some(&child) = children.first() {
                if let Some(text_entity) = find_text(child, &children_q, &mut text_q) {
                    if let Ok(mut text) = text_q.get_mut(text_entity) {
                        **text = hint_text;
                    }
                }
            }
        }
        *visibility = Visibility::Visible;
    } else {
        *visibility = Visibility::Hidden;
    }
}

/// Drives UiFadeIn components — sets opacity on BackgroundColor or TextColor.
pub fn ui_fade_in_system(
    time: Res<Time>,
    mut bg_query: Query<(&mut UiFadeIn, &mut BackgroundColor), Without<Text>>,
    mut text_query: Query<(&mut UiFadeIn, &mut TextColor), With<Text>>,
) {
    for (mut fade, mut bg) in &mut bg_query {
        fade.elapsed += time.delta_secs();
        let t = ((fade.elapsed - fade.delay) / fade.duration).clamp(0.0, 1.0);
        let alpha = t * t * (3.0 - 2.0 * t); // smoothstep
        bg.0 = bg.0.with_alpha(alpha);
    }
    for (mut fade, mut tc) in &mut text_query {
        fade.elapsed += time.delta_secs();
        let t = ((fade.elapsed - fade.delay) / fade.duration).clamp(0.0, 1.0);
        let alpha = t * t * (3.0 - 2.0 * t);
        tc.0 = tc.0.with_alpha(alpha);
    }
}

/// Drives UiFadeOut components.
pub fn ui_fade_out_system(
    time: Res<Time>,
    mut commands: Commands,
    mut bg_query: Query<(Entity, &mut UiFadeOut, &mut BackgroundColor), Without<Text>>,
    mut text_query: Query<(Entity, &mut UiFadeOut, &mut TextColor), With<Text>>,
) {
    for (entity, mut fade, mut bg) in &mut bg_query {
        fade.elapsed += time.delta_secs();
        let t = ((fade.elapsed - fade.delay) / fade.duration).clamp(0.0, 1.0);
        let alpha = 1.0 - t * t * (3.0 - 2.0 * t);
        bg.0 = bg.0.with_alpha(alpha);
        if t >= 1.0 {
            commands.entity(entity).remove::<UiFadeOut>();
            if fade.despawn {
                commands.entity(entity).despawn();
            }
        }
    }
    for (entity, mut fade, mut tc) in &mut text_query {
        fade.elapsed += time.delta_secs();
        let t = ((fade.elapsed - fade.delay) / fade.duration).clamp(0.0, 1.0);
        let alpha = 1.0 - t * t * (3.0 - 2.0 * t);
        tc.0 = tc.0.with_alpha(alpha);
        if t >= 1.0 {
            commands.entity(entity).remove::<UiFadeOut>();
            if fade.despawn {
                commands.entity(entity).despawn();
            }
        }
    }
}

/// Animates UI elements sliding up with a bounce easing.
pub fn ui_slide_in_system(
    time: Res<Time>,
    mut query: Query<(&mut UiSlideIn, &mut Node)>,
) {
    for (mut slide, mut node) in &mut query {
        slide.elapsed += time.delta_secs();
        let t = (slide.elapsed / slide.duration).clamp(0.0, 1.0);
        // Bounce-out easing
        let bounce = if t < 0.6 {
            // Ease out
            let t2 = t / 0.6;
            1.0 - (1.0 - t2) * (1.0 - t2)
        } else if t < 0.8 {
            // Small overshoot
            let t2 = (t - 0.6) / 0.2;
            1.0 + 0.08 * (1.0 - (t2 * 2.0 - 1.0).powi(2))
        } else {
            // Settle back
            let t2 = (t - 0.8) / 0.2;
            1.0 + 0.08 * (1.0 - t2) * (1.0 - t2) * (1.0 - t2)
        };
        let offset = slide.start_offset * (1.0 - bounce);
        node.bottom = Val::Px(30.0 - offset);
    }
}

/// Horizontal scale-in: animates max_width from 0px upward with overflow clipping.
/// The clip naturally reveals content as the panel grows.
pub fn ui_scale_in_system(
    time: Res<Time>,
    mut commands: Commands,
    mut query: Query<(Entity, &mut UiScaleIn, &mut Node)>,
) {
    for (entity, mut scale, mut node) in &mut query {
        scale.elapsed += time.delta_secs();
        let t = (scale.elapsed / scale.duration).clamp(0.0, 1.0);
        // Ease-out: fast start, smooth end
        let eased = 1.0 - (1.0 - t) * (1.0 - t);
        // Animate max_width from 0 to a large value; overflow clips the content
        node.max_width = Val::Px(eased * 1200.0);
        node.overflow = Overflow::clip();
        if t >= 1.0 {
            node.max_width = Val::Auto;
            node.overflow = Overflow::visible();
            commands.entity(entity).remove::<UiScaleIn>();
        }
    }
}

/// Horizontal scale-out: animates max_width from current to 0px.
/// When finished, hides the parent panel and cleans up.
pub fn ui_scale_out_system(
    time: Res<Time>,
    mut commands: Commands,
    mut query: Query<(Entity, &mut UiScaleOut, &mut Node)>,
    parent_q: Query<&ChildOf>,
) {
    for (entity, mut scale, mut node) in &mut query {
        scale.elapsed += time.delta_secs();
        let t = (scale.elapsed / scale.duration).clamp(0.0, 1.0);
        // Ease-in: slow start, fast end
        let eased = t * t;
        node.max_width = Val::Px((1.0 - eased) * 1200.0);
        node.overflow = Overflow::clip();
        if t >= 1.0 {
            node.max_width = Val::Px(0.0);
            commands.entity(entity).remove::<UiScaleOut>();
            // Hide the parent (AnimatedPanel wrapper)
            if let Ok(child_of) = parent_q.get(entity) {
                commands.entity(child_of.parent()).remove::<DismissPanel>();
                commands.entity(child_of.parent()).insert(Visibility::Hidden);
            }
        }
    }
}

/// Watches for panels becoming visible or being dismissed.
/// On visible: adds UiScaleIn to first child.
/// On DismissPanel: starts UiScaleOut on first child.
pub fn panel_appear_animation_system(
    mut commands: Commands,
    panels: Query<(&Visibility, &Children), (With<ui_helpers::AnimatedPanel>, Changed<Visibility>)>,
    mut node_q: Query<&mut Node>,
) {
    for (vis, children) in &panels {
        if *vis == Visibility::Visible {
            if let Some(panel_entity) = children.iter().next() {
                // Set max_width to 0 immediately to prevent 1-frame flash
                if let Ok(mut node) = node_q.get_mut(panel_entity) {
                    node.max_width = Val::Px(0.0);
                    node.overflow = Overflow::clip();
                }
                commands.entity(panel_entity).insert(UiScaleIn {
                    elapsed: 0.0,
                    duration: 0.35,
                });
            }
        }
    }
}

/// Starts the dismiss animation on panels marked with DismissPanel.
pub fn panel_dismiss_animation_system(
    mut commands: Commands,
    panels: Query<(Entity, &Children), (With<ui_helpers::AnimatedPanel>, With<DismissPanel>, Without<UiScaleOut>)>,
    child_q: Query<Option<&UiScaleOut>>,
) {
    for (_panel_entity, children) in &panels {
        if let Some(inner) = children.iter().next() {
            // Don't start if already animating out
            if child_q.get(inner).is_ok_and(|o| o.is_none()) {
                commands.entity(inner).remove::<UiScaleIn>();
                commands.entity(inner).insert(UiScaleOut {
                    elapsed: 0.0,
                    duration: 0.25,
                });
            }
        }
    }
}

// --- Footstep systems ---

/// Startup system: load footstep audio files and insert the FootstepAudio resource.
pub fn load_footstep_audio(mut commands: Commands, asset_server: Res<AssetServer>) {
    let load_glob = |prefix: &str| -> Vec<Handle<AudioSource>> {
        let dir = std::path::Path::new("assets/audio/footsteps");
        let mut handles = Vec::new();
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.starts_with(prefix) && name.ends_with(".wav") && !name.contains("Stop") {
                    handles.push(asset_server.load(format!("audio/footsteps/{name}")));
                }
            }
        }
        handles
    };
    commands.insert_resource(FootstepAudio {
        stone: load_glob("Foot_Conc_BootWalk"),
        grass: load_glob("Foot_Grass_Walk"),
        wood: load_glob("Foot_HdWood_BootWalk"),
        dirt: load_glob("Foot_Dirt1_BootWalk"),
        timer: Timer::from_seconds(0.4, TimerMode::Repeating),
        last_index: usize::MAX,
    });
}

/// Update system: play footstep sounds at regular intervals while the player is walking.
/// Does not play during the intro sequence.
pub fn footstep_sound_system(
    mut commands: Commands,
    time: Res<Time>,
    keyboard: Res<ButtonInput<KeyCode>>,
    intro: Res<IntroSequence>,
    mut footsteps: ResMut<FootstepAudio>,
    player_q: Query<&Transform, With<Player>>,
    audio_settings: Res<AudioSettings>,
) {
    // Don't play footsteps during the intro
    if intro.active {
        return;
    }

    // Check if the player is pressing any movement key
    let moving = keyboard.pressed(KeyCode::KeyW)
        || keyboard.pressed(KeyCode::KeyS)
        || keyboard.pressed(KeyCode::KeyA)
        || keyboard.pressed(KeyCode::KeyD);

    if !moving {
        footsteps.timer.reset();
        return;
    }

    footsteps.timer.tick(time.delta());

    if footsteps.timer.just_finished() {
        // Determine surface type based on player position
        // Floor tiles cover -8..8, outside is grass
        let player_pos = player_q.single().unwrap();
        let on_tiles = player_pos.translation.x.abs() < 9.0 && player_pos.translation.z.abs() < 9.0;

        // TODO: audio environment presets with proper reverb DSP
        let sounds: Vec<_> = if on_tiles {
            footsteps.stone.clone()
        } else {
            footsteps.grass.clone()
        };
        let last = footsteps.last_index;

        if sounds.len() > 1 {
            let mut idx = (time.elapsed_secs() * 7919.0) as usize % sounds.len();
            if idx == last {
                idx = (idx + 1) % sounds.len();
            }
            footsteps.last_index = idx;

            let pitch = 0.93 + ((time.elapsed_secs() * 3571.0) % 1.0) * 0.14;
            let vol = 0.5 * audio_settings.effective_sfx();
            commands.spawn((
                AudioPlayer(sounds[idx].clone()),
                PlaybackSettings {
                    speed: pitch,
                    volume: bevy::audio::Volume::Linear(vol),
                    ..PlaybackSettings::DESPAWN
                },
            ));
        } else if !sounds.is_empty() {
            let vol = 0.5 * audio_settings.effective_sfx();
            commands.spawn((
                AudioPlayer(sounds[0].clone()),
                PlaybackSettings {
                    volume: bevy::audio::Volume::Linear(vol),
                    ..PlaybackSettings::DESPAWN
                },
            ));
        }
    }
}
