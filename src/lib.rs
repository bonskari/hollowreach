pub use bevy::color::palettes::css;
pub use bevy::input::mouse::MouseMotion;
pub use bevy::prelude::*;
pub use bevy::window::CursorGrabMode;
use std::f32::consts::PI;

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

/// Marker for the dialogue text (center screen).
#[derive(Component)]
pub struct DialogueText;

/// Timer resource that tracks when dialogue should be hidden.
#[derive(Resource)]
pub struct DialogueTimer {
    pub timer: Timer,
    pub active: bool,
}

impl Default for DialogueTimer {
    fn default() -> Self {
        Self {
            timer: Timer::from_seconds(3.0, TimerMode::Once),
            active: false,
        }
    }
}

// --- Interaction cooldown ---

/// Resource that tracks a cooldown timer between interactions.
#[derive(Resource)]
pub struct InteractionCooldown(pub Timer);

impl Default for InteractionCooldown {
    fn default() -> Self {
        Self(Timer::from_seconds(1.0, TimerMode::Once))
    }
}

// --- Collision ---

/// Circle collider on the XZ plane. Attach to any entity that the player shouldn't walk through.
#[derive(Component)]
pub struct CircleCollider {
    pub radius: f32,
}

// --- Constants ---

pub const INTERACT_DISTANCE: f32 = 3.5;
pub const PLAYER_RADIUS: f32 = 0.4;

// --- Plugin ---

pub struct HollowreachPlugin;

impl Plugin for HollowreachPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MouseSensitivity>()
            .init_resource::<InteractionCooldown>()
            .init_resource::<DialogueTimer>()
            .init_resource::<AnimationSources>()
            .add_systems(Startup, (setup_scene, grab_cursor, setup_ui))
            .add_systems(
                Update,
                (
                    player_movement,
                    player_collision.after(player_movement),
                    player_look,
                    interact_system,
                    proximity_hint_system,
                    dialogue_fade_system,
                    start_npc_animations,
                    hide_unwanted_meshes,
                ),
            );
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

// --- Setup ---

pub fn setup_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    asset_server: Res<AssetServer>,
    mut anim_sources: ResMut<AnimationSources>,
) {
    // Helper to spawn a dungeon piece
    let d = |path: &str, server: &AssetServer| -> SceneRoot {
        SceneRoot(server.load(format!("kaykit_dungeon/{path}#Scene0")))
    };
    let ch = |path: &str, server: &AssetServer| -> SceneRoot {
        SceneRoot(server.load(format!("kaykit_characters/{path}#Scene0")))
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
        ))
        .with_children(|parent| {
            parent.spawn((
                PlayerCamera { pitch: 0.0, yaw: 0.0 },
                Camera3d::default(),
                Transform::from_xyz(0.0, 0.6, 0.0),
            ));
        });

    // Load character GLTFs as animation sources (animations merged into character GLBs)
    for char_file in ["Knight.glb", "Mage.glb", "Rogue_Hooded.glb", "Barbarian.glb", "Ranger.glb"] {
        let handle: Handle<Gltf> = asset_server.load(format!("kaykit_characters/{char_file}"));
        anim_sources.handles.push(handle);
    }

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

    // --- Tavern area (left/west side) ---
    commands.spawn((d("table_medium.gltf", &asset_server), Transform::from_xyz(-6.0, 0.0, 0.0), CircleCollider { radius: 1.2 }));
    commands.spawn((d("stool.gltf", &asset_server), Transform::from_xyz(-4.8, 0.0, 0.0)));
    commands.spawn((
        d("stool.gltf", &asset_server),
        Transform::from_xyz(-7.2, 0.0, 0.0).with_rotation(Quat::from_rotation_y(PI)),
    ));
    commands.spawn((d("stool.gltf", &asset_server), Transform::from_xyz(-6.0, 0.0, 1.5)));
    // Second table
    commands.spawn((d("table_small.gltf", &asset_server), Transform::from_xyz(-6.0, 0.0, -3.0), CircleCollider { radius: 0.7 }));
    commands.spawn((d("stool.gltf", &asset_server), Transform::from_xyz(-5.2, 0.0, -3.0)));
    commands.spawn((d("stool.gltf", &asset_server), Transform::from_xyz(-6.8, 0.0, -3.0)));

    // --- Storage area (right/east side) ---
    commands.spawn((d("barrel_large.gltf", &asset_server), Transform::from_xyz(7.0, 0.0, -4.0), CircleCollider { radius: 0.6 }));
    commands.spawn((d("barrel_large.gltf", &asset_server), Transform::from_xyz(8.2, 0.0, -5.5), CircleCollider { radius: 0.6 }));
    commands.spawn((d("barrel_small.gltf", &asset_server), Transform::from_xyz(8.0, 0.0, -3.0), CircleCollider { radius: 0.4 }));
    commands.spawn((d("box_large.gltf", &asset_server), Transform::from_xyz(7.0, 0.0, -7.0), CircleCollider { radius: 0.6 }));
    commands.spawn((d("box_small.gltf", &asset_server), Transform::from_xyz(8.2, 0.0, -7.0)));
    commands.spawn((d("box_small.gltf", &asset_server), Transform::from_xyz(7.0, 1.0, -7.0))); // stacked

    // --- Center feature: decorated pillar ---
    commands.spawn((d("pillar_decorated.gltf", &asset_server), Transform::from_xyz(0.0, 0.0, -1.0), CircleCollider { radius: 1.0 }));

    // Treasure chest near back wall
    commands.spawn((
        d("chest.gltf", &asset_server),
        Transform::from_xyz(-7.0, 0.0, -8.0),
        CircleCollider { radius: 0.5 },
        Interactable {
            name: "Old Chest".into(),
            dialogue: "You open the chest. Inside you find a tattered map of the Hollowreach.".into(),
            is_npc: false,
        },
    ));
    commands.spawn((d("chest_gold.gltf", &asset_server), Transform::from_xyz(7.0, 0.0, -8.5), CircleCollider { radius: 0.5 }));

    // Banners on back wall
    commands.spawn((d("banner_blue.gltf", &asset_server), Transform::from_xyz(-4.0, 0.0, -9.4)));
    commands.spawn((d("banner_red.gltf", &asset_server), Transform::from_xyz(0.0, 0.0, -9.4)));
    commands.spawn((d("banner_green.gltf", &asset_server), Transform::from_xyz(4.0, 0.0, -9.4)));

    // Torches along walls
    for &(x, z, rot) in &[
        (-9.4, -6.0, PI / 2.0), (-9.4, 0.0, PI / 2.0), (-9.4, 6.0, PI / 2.0),
        (9.4, -6.0, -PI / 2.0), (9.4, 0.0, -PI / 2.0), (9.4, 6.0, -PI / 2.0),
        (-4.0, -9.4, 0.0), (4.0, -9.4, 0.0),
    ] {
        commands.spawn((
            d("torch_mounted.gltf", &asset_server),
            Transform::from_xyz(x, 0.0, z).with_rotation(Quat::from_rotation_y(rot)),
        ));
    }

    // Shelves along left wall
    commands.spawn((
        d("shelf_large.gltf", &asset_server),
        Transform::from_xyz(-9.0, 0.0, -4.0).with_rotation(Quat::from_rotation_y(PI / 2.0)),
    ));

    // Beds in back-left corner (sleeping area)
    commands.spawn((d("bed_frame.gltf", &asset_server), Transform::from_xyz(-8.0, 0.0, -8.0)));
    commands.spawn((
        d("bed_frame.gltf", &asset_server),
        Transform::from_xyz(-8.0, 0.0, -6.0).with_rotation(Quat::from_rotation_y(PI)),
    ));

    // --- NPCs (KayKit Adventurer characters) ---

    // Knight — village guard near entrance (away from pillars)
    commands.spawn((
        ch("Knight.glb", &asset_server),
        Transform::from_xyz(4.0, 0.0, 8.0).with_rotation(Quat::from_rotation_y(PI)),
        CircleCollider { radius: 0.5 },
        Interactable {
            name: "Sir Roland".into(),
            dialogue: "\"Welcome to Hollowreach, traveler. Keep your wits about you — these walls hold more secrets than stone.\"".into(),
            is_npc: true,
        },
    ));

    // Mage — near tavern table (offset so not inside stool)
    commands.spawn((
        ch("Mage.glb", &asset_server),
        Transform::from_xyz(-4.0, 0.0, 1.5).with_rotation(Quat::from_rotation_y(PI / 2.0)),
        CircleCollider { radius: 0.5 },
        Interactable {
            name: "Elara the Wise".into(),
            dialogue: "\"The ley lines beneath this village... they pulse with an ancient energy. Something stirs below.\"".into(),
            is_npc: true,
        },
    ));

    // Rogue — lurking near storage (offset from barrels)
    commands.spawn((
        ch("Rogue_Hooded.glb", &asset_server),
        Transform::from_xyz(5.5, 0.0, -5.0).with_rotation(Quat::from_rotation_y(-PI / 3.0)),
        CircleCollider { radius: 0.5 },
        Interactable {
            name: "Whisper".into(),
            dialogue: "\"Psst... looking for something? I know passages the guards don't. For the right price, of course.\"".into(),
            is_npc: true,
        },
    ));

    // Barbarian — near the center pillar (offset from it)
    commands.spawn((
        ch("Barbarian.glb", &asset_server),
        Transform::from_xyz(2.0, 0.0, 0.5),
        CircleCollider { radius: 0.5 },
        Interactable {
            name: "Grok".into(),
            dialogue: "\"Grok not like this place. Too many walls. But the food is good.\"".into(),
            is_npc: true,
        },
    ));

    // Ranger — on lookout near back wall
    commands.spawn((
        ch("Ranger.glb", &asset_server),
        Transform::from_xyz(3.0, 0.0, -8.0).with_rotation(Quat::from_rotation_y(PI)),
        CircleCollider { radius: 0.5 },
        Interactable {
            name: "Sylva".into(),
            dialogue: "\"The forest beyond these walls grows darker each night. I've tracked something... unnatural.\"".into(),
            is_npc: true,
        },
    ));

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

        let mut mesh = Mesh::new(bevy::render::mesh::PrimitiveTopology::TriangleList, bevy::render::render_asset::RenderAssetUsages::default());
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
        mesh.insert_indices(bevy::render::mesh::Indices::U32(indices));

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

        let mut mesh = Mesh::new(bevy::render::mesh::PrimitiveTopology::TriangleList, bevy::render::render_asset::RenderAssetUsages::default());
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
        mesh.insert_indices(bevy::render::mesh::Indices::U32(indices));

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
    commands.insert_resource(AmbientLight {
        color: Color::srgb(0.9, 0.9, 1.0),
        brightness: 400.0,
    });

    // Sun
    commands.spawn((
        DirectionalLight {
            illuminance: 10000.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -PI / 3.5, PI / 5.0, 0.0)),
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

/// Marker for the dialogue box container (the dark panel).
#[derive(Component)]
pub struct DialogueBox;

/// Marker for the NPC name text inside the dialogue box.
#[derive(Component)]
pub struct DialogueNameText;

pub fn setup_ui(mut commands: Commands) {
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
            // --- Dialogue box (RPG-style bottom panel) ---
            parent
                .spawn((
                    DialogueBox,
                    Node {
                        position_type: PositionType::Absolute,
                        bottom: Val::Px(30.0),
                        left: Val::Percent(10.0),
                        right: Val::Percent(10.0),
                        padding: UiRect::all(Val::Px(20.0)),
                        flex_direction: FlexDirection::Column,
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.05, 0.05, 0.1, 0.85)),
                    BorderRadius::all(Val::Px(8.0)),
                    BorderColor(Color::srgba(0.6, 0.5, 0.3, 0.8)),
                    Outline::new(Val::Px(2.0), Val::ZERO, Color::srgba(0.6, 0.5, 0.3, 0.8)),
                    Visibility::Hidden,
                ))
                .with_children(|box_parent| {
                    // NPC name (gold colored, bold-ish)
                    box_parent.spawn((
                        DialogueNameText,
                        Text::new(""),
                        TextFont {
                            font_size: 22.0,
                            ..default()
                        },
                        TextColor(Color::srgb(0.9, 0.75, 0.3)),
                        Node {
                            margin: UiRect::bottom(Val::Px(8.0)),
                            ..default()
                        },
                    ));

                    // Dialogue text (white)
                    box_parent.spawn((
                        DialogueText,
                        Text::new(""),
                        TextFont {
                            font_size: 18.0,
                            ..default()
                        },
                        TextColor(Color::srgba(0.95, 0.95, 0.95, 1.0)),
                        TextLayout::new_with_justify(JustifyText::Left),
                    ));
                });

            // --- Proximity hint (bottom center, styled key prompt) ---
            parent
                .spawn((
                    ProximityHintText,
                    Node {
                        margin: UiRect::bottom(Val::Px(10.0)),
                        padding: UiRect::axes(Val::Px(16.0), Val::Px(8.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.5)),
                    BorderRadius::all(Val::Px(6.0)),
                    Visibility::Hidden,
                ))
                .with_children(|hint_parent| {
                    hint_parent.spawn((
                        Text::new(""),
                        TextFont {
                            font_size: 16.0,
                            ..default()
                        },
                        TextColor(Color::srgba(1.0, 1.0, 0.8, 1.0)),
                        TextLayout::new_with_justify(JustifyText::Center),
                    ));
                });

            // --- Crosshair (small dot in center) ---
            parent.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    top: Val::Percent(50.0),
                    left: Val::Percent(50.0),
                    width: Val::Px(4.0),
                    height: Val::Px(4.0),
                    margin: UiRect {
                        left: Val::Px(-2.0),
                        top: Val::Px(-2.0),
                        ..default()
                    },
                    ..default()
                },
                BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.5)),
                BorderRadius::all(Val::Px(2.0)),
            ));
        });
}

pub fn grab_cursor(mut windows: Query<&mut Window>) {
    let mut window = windows.single_mut();
    window.cursor_options.grab_mode = CursorGrabMode::Locked;
    window.cursor_options.visible = false;
}

// --- Player Systems ---

pub fn player_movement(
    keyboard: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    mut player_q: Query<&mut Transform, With<Player>>,
    camera_q: Query<&PlayerCamera>,
) {
    let mut transform = player_q.single_mut();
    let camera = camera_q.single();

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
    let mut transform = player_q.single_mut();
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
    mut mouse_motion: EventReader<MouseMotion>,
    sensitivity: Res<MouseSensitivity>,
    mut camera_q: Query<(&mut PlayerCamera, &mut Transform)>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut windows: Query<&mut Window>,
) {
    if keyboard.just_pressed(KeyCode::Escape) {
        let mut window = windows.single_mut();
        match window.cursor_options.grab_mode {
            CursorGrabMode::Locked => {
                window.cursor_options.grab_mode = CursorGrabMode::None;
                window.cursor_options.visible = true;
            }
            _ => {
                window.cursor_options.grab_mode = CursorGrabMode::Locked;
                window.cursor_options.visible = false;
            }
        }
    }

    let (mut camera, mut cam_transform) = camera_q.single_mut();

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
    player_q: Query<&Transform, With<Player>>,
    interactables: Query<(&Transform, &Interactable), Without<Player>>,
    mut dialogue_text_q: Query<&mut Text, With<DialogueText>>,
    mut dialogue_name_q: Query<&mut Text, (With<DialogueNameText>, Without<DialogueText>)>,
    mut dialogue_box_q: Query<&mut Visibility, With<DialogueBox>>,
    mut dialogue_timer: ResMut<DialogueTimer>,
) {
    cooldown.0.tick(time.delta());

    if !keyboard.just_pressed(KeyCode::KeyE) {
        return;
    }
    if !cooldown.0.finished() {
        return;
    }

    let player_tf = player_q.single();

    let mut closest: Option<(&Interactable, f32)> = None;
    for (tf, interactable) in &interactables {
        let dist = player_tf.translation.distance(tf.translation);
        if dist < INTERACT_DISTANCE {
            if closest.is_none() || dist < closest.unwrap().1 {
                closest = Some((interactable, dist));
            }
        }
    }

    if let Some((interactable, _)) = closest {
        // Set name
        let mut name_text = dialogue_name_q.single_mut();
        **name_text = interactable.name.clone();

        // Set dialogue
        let mut text = dialogue_text_q.single_mut();
        **text = interactable.dialogue.clone();

        // Show dialogue box
        let mut box_vis = dialogue_box_q.single_mut();
        *box_vis = Visibility::Visible;

        dialogue_timer.timer.reset();
        dialogue_timer.active = true;
        cooldown.0.reset();
    }
}

pub fn proximity_hint_system(
    player_q: Query<&Transform, With<Player>>,
    interactables: Query<(&Transform, &Interactable), Without<Player>>,
    mut hint_q: Query<(&mut Visibility, &Children), With<ProximityHintText>>,
    mut text_q: Query<&mut Text>,
) {
    let player_tf = player_q.single();

    let mut nearest: Option<(&Interactable, f32)> = None;
    for (tf, interactable) in &interactables {
        let dist = player_tf.translation.distance(tf.translation);
        if dist < INTERACT_DISTANCE {
            if nearest.is_none() || dist < nearest.unwrap().1 {
                nearest = Some((interactable, dist));
            }
        }
    }

    let (mut visibility, children) = hint_q.single_mut();
    if let Some((interactable, _)) = nearest {
        if let Some(&child) = children.first() {
            if let Ok(mut text) = text_q.get_mut(child) {
                **text = format!("[E]  {}", interactable.name);
            }
        }
        *visibility = Visibility::Visible;
    } else {
        *visibility = Visibility::Hidden;
    }
}

pub fn dialogue_fade_system(
    time: Res<Time>,
    mut dialogue_timer: ResMut<DialogueTimer>,
    mut box_q: Query<&mut Visibility, With<DialogueBox>>,
) {
    if !dialogue_timer.active {
        return;
    }

    dialogue_timer.timer.tick(time.delta());

    if dialogue_timer.timer.finished() {
        dialogue_timer.active = false;
        let mut visibility = box_q.single_mut();
        *visibility = Visibility::Hidden;
    }
}
