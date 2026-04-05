use bevy::color::palettes::css;
use bevy::input::mouse::MouseMotion;
use bevy::prelude::*;
use bevy::window::CursorGrabMode;
use std::f32::consts::PI;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Hollowreach".into(),
                resolution: (1280.0, 720.0).into(),
                ..default()
            }),
            ..default()
        }))
        .init_resource::<MouseSensitivity>()
        .add_systems(Startup, (setup_scene, grab_cursor))
        .add_systems(
            Update,
            (
                player_movement,
                player_look,
                interact_system,
                proximity_hint_system,
            ),
        )
        .run();
}

// --- Components ---

#[derive(Component)]
struct Player;

#[derive(Component)]
struct PlayerCamera {
    pitch: f32,
    yaw: f32,
}

#[derive(Component)]
struct Interactable {
    name: String,
    dialogue: String,
    is_npc: bool,
}

#[derive(Resource)]
struct MouseSensitivity(f32);

impl Default for MouseSensitivity {
    fn default() -> Self {
        Self(0.003)
    }
}

// --- Setup ---

fn setup_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Ground plane
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(50.0, 50.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::from(css::DARK_GREEN),
            ..default()
        })),
    ));

    // Player (invisible body + camera)
    commands
        .spawn((
            Player,
            Transform::from_xyz(0.0, 1.0, 5.0),
            Visibility::default(),
        ))
        .with_children(|parent| {
            parent.spawn((
                PlayerCamera {
                    pitch: 0.0,
                    yaw: 0.0,
                },
                Camera3d::default(),
                Transform::from_xyz(0.0, 0.6, 0.0),
            ));
        });

    // --- Environment objects ---

    let wall_material = materials.add(StandardMaterial {
        base_color: Color::from(css::DIM_GRAY),
        ..default()
    });

    // Back wall
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(20.0, 4.0, 0.5))),
        MeshMaterial3d(wall_material.clone()),
        Transform::from_xyz(0.0, 2.0, -10.0),
    ));

    // Left wall
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(0.5, 4.0, 20.0))),
        MeshMaterial3d(wall_material.clone()),
        Transform::from_xyz(-10.0, 2.0, 0.0),
    ));

    // Right wall
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(0.5, 4.0, 20.0))),
        MeshMaterial3d(wall_material.clone()),
        Transform::from_xyz(10.0, 2.0, 0.0),
    ));

    // Pillar
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(1.0, 5.0, 1.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::from(css::SLATE_GRAY),
            ..default()
        })),
        Transform::from_xyz(-4.0, 2.5, -3.0),
    ));

    // --- Interactable objects ---

    // Glowing orb
    commands.spawn((
        Mesh3d(meshes.add(Sphere::new(0.3))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::from(css::GOLD),
            emissive: LinearRgba::new(5.0, 4.0, 0.0, 1.0),
            ..default()
        })),
        Transform::from_xyz(3.0, 1.2, -2.0),
        Interactable {
            name: "Mysterious Orb".into(),
            dialogue: "The orb hums with a faint energy. It feels warm to the touch.".into(),
            is_npc: false,
        },
    ));

    // Chest
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(0.8, 0.5, 0.5))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::from(css::SADDLE_BROWN),
            ..default()
        })),
        Transform::from_xyz(-6.0, 0.25, 2.0),
        Interactable {
            name: "Old Chest".into(),
            dialogue: "You open the chest. Inside you find a tattered map of the Hollowreach."
                .into(),
            is_npc: false,
        },
    ));

    // --- NPCs ---

    let npc_body = meshes.add(Capsule3d::new(0.3, 1.2));

    // NPC 1 - The Wanderer
    commands
        .spawn((
            Mesh3d(npc_body.clone()),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: Color::from(css::ROYAL_BLUE),
                ..default()
            })),
            Transform::from_xyz(5.0, 0.9, -5.0),
            Interactable {
                name: "The Wanderer".into(),
                dialogue: "\"Ah, a new face in the Hollowreach. Be careful — not everything here is as it seems. The stones remember things we've long forgotten.\"".into(),
                is_npc: true,
            },
        ))
        .with_children(|parent| {
            parent.spawn((
                Mesh3d(meshes.add(Sphere::new(0.25))),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color: Color::from(css::PEACH_PUFF),
                    ..default()
                })),
                Transform::from_xyz(0.0, 0.85, 0.0),
            ));
        });

    // NPC 2 - The Keeper
    commands
        .spawn((
            Mesh3d(npc_body),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: Color::from(css::DARK_RED),
                ..default()
            })),
            Transform::from_xyz(-3.0, 0.9, -7.0),
            Interactable {
                name: "The Keeper".into(),
                dialogue: "\"I guard what remains. The Hollowreach was once a thriving settlement, before the ground itself swallowed it whole. Take the orb if you must — but know its cost.\"".into(),
                is_npc: true,
            },
        ))
        .with_children(|parent| {
            parent.spawn((
                Mesh3d(meshes.add(Sphere::new(0.25))),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color: Color::from(css::PEACH_PUFF),
                    ..default()
                })),
                Transform::from_xyz(0.0, 0.85, 0.0),
            ));
        });

    // --- Lighting ---

    commands.insert_resource(AmbientLight {
        color: Color::from(css::LIGHT_BLUE),
        brightness: 200.0,
    });

    commands.spawn((
        DirectionalLight {
            illuminance: 5000.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -PI / 4.0, PI / 6.0, 0.0)),
    ));

    commands.spawn((
        PointLight {
            color: Color::from(css::GOLD),
            intensity: 50000.0,
            range: 10.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(3.0, 2.0, -2.0),
    ));

    println!("=== HOLLOWREACH ===");
    println!("Controls:");
    println!("  WASD  - Move");
    println!("  Mouse - Look around");
    println!("  E     - Interact");
    println!("  Esc   - Release cursor");
    println!("==================");
}

fn grab_cursor(mut windows: Query<&mut Window>) {
    if let Ok(mut window) = windows.single_mut() {
        window.cursor_options.grab_mode = CursorGrabMode::Locked;
        window.cursor_options.visible = false;
    }
}

// --- Player Systems ---

fn player_movement(
    keyboard: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    mut player_q: Query<&mut Transform, With<Player>>,
    camera_q: Query<&PlayerCamera>,
) {
    let Ok(mut transform) = player_q.single_mut() else {
        return;
    };
    let Ok(camera) = camera_q.single() else {
        return;
    };

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

fn player_look(
    mut mouse_motion: EventReader<MouseMotion>,
    sensitivity: Res<MouseSensitivity>,
    mut camera_q: Query<(&mut PlayerCamera, &mut Transform)>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut windows: Query<&mut Window>,
) {
    if keyboard.just_pressed(KeyCode::Escape) {
        if let Ok(mut window) = windows.single_mut() {
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
    }

    let Ok((mut camera, mut cam_transform)) = camera_q.single_mut() else {
        return;
    };

    for ev in mouse_motion.read() {
        camera.yaw -= ev.delta.x * sensitivity.0;
        camera.pitch -= ev.delta.y * sensitivity.0;
        camera.pitch = camera.pitch.clamp(-PI / 2.0 + 0.05, PI / 2.0 - 0.05);
    }

    cam_transform.rotation =
        Quat::from_rotation_y(camera.yaw) * Quat::from_rotation_x(camera.pitch);
}

// --- Interaction ---

const INTERACT_DISTANCE: f32 = 3.5;

fn interact_system(
    keyboard: Res<ButtonInput<KeyCode>>,
    player_q: Query<&Transform, With<Player>>,
    interactables: Query<(&Transform, &Interactable), Without<Player>>,
) {
    if !keyboard.just_pressed(KeyCode::KeyE) {
        return;
    }

    let Ok(player_tf) = player_q.single() else {
        return;
    };

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
        if interactable.is_npc {
            println!("\n[{}] says:", interactable.name);
        } else {
            println!("\n[{}]:", interactable.name);
        }
        println!("  {}\n", interactable.dialogue);
    }
}

fn proximity_hint_system(
    player_q: Query<&Transform, With<Player>>,
    interactables: Query<(&Transform, &Interactable), Without<Player>>,
    mut last_hint: Local<Option<String>>,
) {
    let Ok(player_tf) = player_q.single() else {
        return;
    };

    let mut nearest: Option<(&Interactable, f32)> = None;

    for (tf, interactable) in &interactables {
        let dist = player_tf.translation.distance(tf.translation);
        if dist < INTERACT_DISTANCE {
            if nearest.is_none() || dist < nearest.unwrap().1 {
                nearest = Some((interactable, dist));
            }
        }
    }

    let current_name = nearest.map(|(i, _)| i.name.clone());

    if current_name != *last_hint {
        if let Some(ref name) = current_name {
            println!("[Press E to interact with {}]", name);
        }
        *last_hint = current_name;
    }
}
