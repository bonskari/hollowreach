use bevy::prelude::*;
use bevy::render::view::screenshot::{save_to_disk, Screenshot};
use bevy::window::CursorGrabMode;
use hollowreach::*;
use std::f32::consts::PI;

const SCREENSHOT_DIR: &str = "test_screenshots";

// ============================================================
// Test state machine
// ============================================================

#[derive(Resource)]
struct TestRunner {
    phase: usize,
    frame_in_phase: usize,
    passed: Vec<String>,
    failed: Vec<(String, String)>,
    saved_pos: Option<Vec3>,
    saved_float: Option<f32>,
}

impl TestRunner {
    fn new() -> Self {
        Self {
            phase: 0,
            frame_in_phase: 0,
            passed: Vec::new(),
            failed: Vec::new(),
            saved_pos: None,
            saved_float: None,
        }
    }

    fn pass(&mut self, name: &str) {
        println!("  PASS: {}", name);
        self.passed.push(name.to_string());
    }

    fn fail(&mut self, name: &str, reason: &str) {
        println!("  FAIL: {} — {}", name, reason);
        self.failed.push((name.to_string(), reason.to_string()));
    }

    fn check(&mut self, name: &str, condition: bool, fail_msg: &str) {
        if condition {
            self.pass(name);
        } else {
            self.fail(name, fail_msg);
        }
    }

    fn next_phase(&mut self) {
        self.phase += 1;
        self.frame_in_phase = 0;
    }

    fn report(&self) {
        println!("\n========================================");
        println!(
            "Results: {} passed, {} failed",
            self.passed.len(),
            self.failed.len()
        );
        if !self.failed.is_empty() {
            println!("\nFailures:");
            for (name, reason) in &self.failed {
                println!("  - {}: {}", name, reason);
            }
        }
        println!("========================================");
    }

    fn all_passed(&self) -> bool {
        self.failed.is_empty()
    }
}

fn screenshot(commands: &mut Commands, name: &str) {
    let path = format!("{}/{}.png", SCREENSHOT_DIR, name);
    commands
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(path));
}

fn test_system(
    mut runner: ResMut<TestRunner>,
    mut commands: Commands,
    mut keyboard: ResMut<ButtonInput<KeyCode>>,
    mut mouse_events: EventWriter<bevy::input::mouse::MouseMotion>,
    mut player_q: Query<&mut Transform, With<Player>>,
    camera_q: Query<(&PlayerCamera, &GlobalTransform)>,
    _interactable_q: Query<(&Transform, &Interactable), Without<Player>>,
    cooldown: Option<Res<InteractionCooldown>>,
    ui_dialogue_q: Query<Entity, With<DialogueText>>,
    ui_hint_q: Query<Entity, With<ProximityHintText>>,
    window_q: Query<&Window>,
    mut exit: EventWriter<AppExit>,
) {
    runner.frame_in_phase += 1;
    let phase = runner.phase;
    let frame = runner.frame_in_phase;

    match phase {
        // ---- Phase 0: Warmup (wait for GLTF assets to load) ----
        0 => {
            if frame >= 120 {
                println!("\n=== Hollowreach E2E Tests ===");
                println!("=== Bug-hunting mode ===\n");
                runner.next_phase();
            }
        }

        // ---- Phase 1: Initial screenshot from spawn ----
        1 => {
            println!("[1] Initial scene from spawn point");
            screenshot(&mut commands, "01_spawn_view");
            runner.next_phase();
        }

        // ============================================================
        // COLLISION TESTS — Can the player walk through walls?
        // ============================================================

        // ---- Phase 2: Walk into back wall (z = -10) ----
        2 => {
            if frame == 1 {
                println!("\n[2] Collision: walk into back wall");
                // Place player facing the back wall, close to it
                let mut tf = player_q.single_mut();
                tf.translation = Vec3::new(0.0, 1.0, -8.0);
                // Start walking forward (toward wall at z=-10)
                keyboard.press(KeyCode::KeyW);
            }
            if frame >= 60 {
                keyboard.release(KeyCode::KeyW);
                let tf = player_q.single();
                // Back wall is at z=-10, thickness 0.5, so surface is at z=-9.75
                // If player walked through, z will be < -10
                runner.check(
                    "wall_collision_back",
                    tf.translation.z >= -10.0,
                    &format!(
                        "Player walked through back wall! z={} (wall at z=-10)",
                        tf.translation.z
                    ),
                );
                screenshot(&mut commands, "02_wall_collision_back");
                runner.next_phase();
            }
        }

        // ---- Phase 3: Walk into left wall (x = -10) ----
        3 => {
            if frame == 1 {
                let mut tf = player_q.single_mut();
                tf.translation = Vec3::new(-8.0, 1.0, 0.0);
                // Set camera yaw to face left (-X direction)
                // We need to modify PlayerCamera yaw... use mouse event
            }
            if frame == 2 {
                // Send large mouse motion to turn left
                mouse_events.send(bevy::input::mouse::MouseMotion {
                    delta: Vec2::new(500.0, 0.0), // turn right a lot, then we'll walk
                });
            }
            if frame == 5 {
                // Now walk forward (which is now roughly toward -X due to yaw)
                // Actually let's just directly test: teleport near left wall and press A
                let mut tf = player_q.single_mut();
                tf.translation = Vec3::new(-8.0, 1.0, 0.0);
                keyboard.press(KeyCode::KeyA);
            }
            if frame >= 60 {
                keyboard.release(KeyCode::KeyA);
                let tf = player_q.single();
                runner.check(
                    "wall_collision_left",
                    tf.translation.x >= -10.5,
                    &format!(
                        "Player walked through left wall! x={} (wall at x=-10)",
                        tf.translation.x
                    ),
                );
                screenshot(&mut commands, "03_wall_collision_left");
                runner.next_phase();
            }
        }

        // ============================================================
        // FLOOR TEST — Does the player fall through the ground?
        // ============================================================

        // ---- Phase 4: Check player stays on ground level ----
        4 => {
            if frame == 1 {
                println!("\n[3] Floor: does player stay above ground?");
                // Place player at normal height
                let mut tf = player_q.single_mut();
                tf.translation = Vec3::new(0.0, 1.0, 5.0);
            }
            // Walk for a while
            if frame >= 2 && frame < 60 {
                keyboard.press(KeyCode::KeyW);
            }
            if frame >= 60 {
                keyboard.release(KeyCode::KeyW);
                let tf = player_q.single();
                runner.check(
                    "player_stays_above_ground",
                    tf.translation.y >= 0.0,
                    &format!("Player fell below ground! y={}", tf.translation.y),
                );
                runner.check(
                    "player_at_consistent_height",
                    (tf.translation.y - 1.0).abs() < 0.01,
                    &format!(
                        "Player height changed from 1.0 to {} — no gravity system keeps player grounded",
                        tf.translation.y
                    ),
                );
                runner.next_phase();
            }
        }

        // ============================================================
        // BOUNDARY TEST — Can the player walk off the map?
        // ============================================================

        // ---- Phase 5: Walk far beyond the play area ----
        5 => {
            if frame == 1 {
                println!("\n[4] Boundaries: can player leave the map?");
                // Place player at edge and walk outward
                let mut tf = player_q.single_mut();
                tf.translation = Vec3::new(0.0, 1.0, 20.0);
                keyboard.press(KeyCode::KeyS); // Walk backward (positive Z)
            }
            if frame >= 120 {
                keyboard.release(KeyCode::KeyS);
                let tf = player_q.single();
                // Ground plane is 50x50, centered at origin, so edges at +-25
                // But there's no wall on the south side
                runner.check(
                    "player_bounded_south",
                    tf.translation.z <= 25.0,
                    &format!(
                        "Player walked off map! z={} (ground ends at z=25)",
                        tf.translation.z
                    ),
                );
                screenshot(&mut commands, "04_out_of_bounds");
                runner.next_phase();
            }
        }

        // ---- Phase 6: Verify boundary clamping works ----
        6 => {
            if frame == 1 {
                let mut tf = player_q.single_mut();
                tf.translation = Vec3::new(0.0, 1.0, 100.0); // Way off map
            }
            if frame >= 5 {
                let tf = player_q.single();
                runner.check(
                    "boundary_clamp_works",
                    tf.translation.z <= 25.0,
                    &format!("Player not clamped: z={}", tf.translation.z),
                );
                screenshot(&mut commands, "05_boundary_clamp");
                runner.next_phase();
            }
        }

        // ============================================================
        // OBJECT COLLISION — Can player walk through objects/NPCs?
        // ============================================================

        // ---- Phase 7: Walk through the pillar ----
        7 => {
            if frame == 1 {
                println!("\n[5] Object collision: walk through pillar");
                // Reset camera yaw to face forward
                // Pillar is at (-4, 2.5, -3), size 1x5x1
                // Place player just in front of it
                let mut tf = player_q.single_mut();
                tf.translation = Vec3::new(-4.0, 1.0, 0.0);
                keyboard.press(KeyCode::KeyW);
            }
            if frame >= 60 {
                keyboard.release(KeyCode::KeyW);
                let tf = player_q.single();
                // Pillar is at z=-3. If no collision, player will be past it
                runner.check(
                    "pillar_collision",
                    tf.translation.z > -3.0,
                    &format!(
                        "Player walked through pillar! z={} (pillar at z=-3)",
                        tf.translation.z
                    ),
                );
                screenshot(&mut commands, "06_pillar_walkthrough");
                runner.next_phase();
            }
        }

        // ---- Phase 8: Walk through an NPC ----
        8 => {
            if frame == 1 {
                println!("\n[6] NPC collision: walk through The Wanderer");
                // Wanderer at (5, 0.9, -5)
                let mut tf = player_q.single_mut();
                tf.translation = Vec3::new(5.0, 1.0, 0.0);
                keyboard.press(KeyCode::KeyW);
            }
            if frame >= 90 {
                keyboard.release(KeyCode::KeyW);
                let tf = player_q.single();
                // Wanderer is at z=-5. If no collision, player will be past it
                runner.check(
                    "npc_collision",
                    tf.translation.z > -5.5,
                    &format!(
                        "Player walked through NPC! z={} (NPC at z=-5)",
                        tf.translation.z
                    ),
                );
                screenshot(&mut commands, "07_npc_walkthrough");
                runner.next_phase();
            }
        }

        // ============================================================
        // INTERACTION TESTS — Edge cases
        // ============================================================

        // ---- Phase 9: Interact from too far ----
        9 => {
            if frame == 1 {
                println!("\n[7] Interaction: verify distance limit");
                // Place player just outside interaction range of orb (at 3, 1.2, -2)
                // INTERACT_DISTANCE is 3.5
                let mut tf = player_q.single_mut();
                tf.translation = Vec3::new(3.0, 1.0, 2.0); // distance ~4.0
                let dist = tf.translation.distance(Vec3::new(3.0, 1.2, -2.0));
                runner.check(
                    "player_outside_interact_range",
                    dist > INTERACT_DISTANCE,
                    &format!("Player too close: dist={}", dist),
                );
            }
            // E pressed while too far should do nothing (no panic)
            if frame == 5 {
                keyboard.press(KeyCode::KeyE);
            }
            if frame == 7 {
                keyboard.release(KeyCode::KeyE);
            }
            if frame >= 10 {
                runner.pass("interact_out_of_range_no_crash");
                runner.next_phase();
            }
        }

        // ---- Phase 10: Rapid E pressing (interaction spam) ----
        10 => {
            if frame == 1 {
                println!("\n[8] Interaction spam: rapid E pressing near NPC");
                // Place player right next to The Keeper
                let mut tf = player_q.single_mut();
                tf.translation = Vec3::new(-3.0, 1.0, -7.0);
            }
            // Spam E every 2 frames for 20 frames
            if frame >= 3 && frame < 23 {
                if frame % 2 == 1 {
                    keyboard.press(KeyCode::KeyE);
                } else {
                    keyboard.release(KeyCode::KeyE);
                }
            }
            if frame >= 25 {
                runner.pass("interaction_spam_no_crash");
                // Check if cooldown resource exists
                runner.check(
                    "interaction_cooldown",
                    cooldown.is_some(),
                    "No InteractionCooldown resource — player can spam E",
                );
                screenshot(&mut commands, "08_interaction_spam");
                runner.next_phase();
            }
        }

        // ============================================================
        // VISUAL/UI TESTS
        // ============================================================

        // ---- Phase 11: Is there any on-screen UI? ----
        11 => {
            if frame == 1 {
                println!("\n[9] UI: check for on-screen elements");
            }
            if frame >= 3 {
                // Check for UI components
                let has_dialogue = ui_dialogue_q.iter().count() > 0;
                let has_hint = ui_hint_q.iter().count() > 0;
                runner.check("has_onscreen_ui", has_dialogue || has_hint, "No UI nodes found");
                runner.check("has_interaction_feedback_ui", has_dialogue, "No DialogueText entity found");
                runner.check("has_proximity_hint_ui", has_hint, "No ProximityHintText entity found");
                runner.next_phase();
            }
        }

        // ---- Phase 12: Camera yaw duplication test ----
        12 => {
            if frame == 1 {
                println!("\n[10] Camera: yaw duplication check");
                // Reset everything: player at origin, no rotation
                let mut tf = player_q.single_mut();
                tf.translation = Vec3::new(0.0, 1.0, 5.0);
                tf.rotation = Quat::IDENTITY;
                // Send a known mouse motion to set a specific yaw
                mouse_events.send(bevy::input::mouse::MouseMotion {
                    delta: Vec2::new(200.0, 0.0), // horizontal only
                });
            }
            if frame >= 5 {
                let (cam, global_tf) = camera_q.single();
                // cam.yaw is the intended yaw angle
                // Player rotation = yaw (set by player_movement)
                // Camera LOCAL rotation = yaw * pitch (set by player_look)
                // Camera GLOBAL rotation = player_rot * camera_local = yaw * (yaw * pitch) = yaw² * pitch
                //
                // If this is correct (no duplication), camera global should be just yaw * pitch
                // Extract yaw from global transform
                let (global_yaw, global_pitch, _) = global_tf
                    .compute_transform()
                    .rotation
                    .to_euler(EulerRot::YXZ);

                // The intended yaw is cam.yaw. If doubled, global_yaw ≈ 2 * cam.yaw
                let yaw_ratio = if cam.yaw.abs() > 0.01 {
                    global_yaw / cam.yaw
                } else {
                    1.0 // can't test if yaw is ~0
                };

                runner.check(
                    "camera_yaw_not_doubled",
                    (yaw_ratio - 1.0).abs() < 0.2,
                    &format!(
                        "Camera world yaw ({:.3}) is {:.1}x the intended yaw ({:.3}). \
                         Yaw is applied to both player AND camera child, causing double rotation.",
                        global_yaw, yaw_ratio, cam.yaw
                    ),
                );
                screenshot(&mut commands, "09_yaw_test");
                runner.next_phase();
            }
        }

        // ---- Phase 13: Camera extreme angles - look down ----
        13 => {
            if frame == 1 {
                println!("\n[11] Camera: extreme angles");
                let mut tf = player_q.single_mut();
                tf.translation = Vec3::new(0.0, 1.0, 5.0);
                mouse_events.send(bevy::input::mouse::MouseMotion {
                    delta: Vec2::new(0.0, 10000.0),
                });
            }
            if frame >= 5 {
                let (cam, _) = camera_q.single();
                runner.check(
                    "pitch_clamp_down",
                    cam.pitch >= -PI / 2.0 + 0.04,
                    &format!("Pitch went too far down: {}", cam.pitch),
                );
                screenshot(&mut commands, "10_look_down");
                runner.next_phase();
            }
        }

        // ---- Phase 14: Look up extreme ----
        14 => {
            if frame == 1 {
                mouse_events.send(bevy::input::mouse::MouseMotion {
                    delta: Vec2::new(0.0, -20000.0),
                });
            }
            if frame >= 5 {
                let (cam, _) = camera_q.single();
                runner.check(
                    "pitch_clamp_up",
                    cam.pitch <= PI / 2.0 - 0.04,
                    &format!("Pitch went too far up: {}", cam.pitch),
                );
                screenshot(&mut commands, "11_look_up");
                runner.next_phase();
            }
        }

        // ============================================================
        // MOVEMENT EDGE CASES
        // ============================================================

        // ---- Phase 15: Diagonal movement speed ----
        15 => {
            if frame == 1 {
                println!("\n[12] Movement: diagonal speed check");
                // Reset position and camera
                let mut tf = player_q.single_mut();
                tf.translation = Vec3::new(0.0, 1.0, 5.0);
                runner.saved_pos = Some(tf.translation);
                // Walk forward only
                keyboard.press(KeyCode::KeyW);
            }
            if frame == 31 {
                keyboard.release(KeyCode::KeyW);
                let tf = player_q.single();
                let forward_dist = (tf.translation - runner.saved_pos.unwrap()).length();
                runner.saved_float = Some(forward_dist);
                // Reset
                let mut tf = player_q.single_mut();
                tf.translation = Vec3::new(0.0, 1.0, 5.0);
                runner.saved_pos = Some(tf.translation);
            }
            if frame == 32 {
                // Walk diagonally
                keyboard.press(KeyCode::KeyW);
                keyboard.press(KeyCode::KeyD);
            }
            if frame == 62 {
                keyboard.release(KeyCode::KeyW);
                keyboard.release(KeyCode::KeyD);
                let tf = player_q.single();
                let diag_dist = (tf.translation - runner.saved_pos.unwrap()).length();
                let forward_dist = runner.saved_float.unwrap();
                let ratio = diag_dist / forward_dist;
                // If properly normalized, ratio should be ~1.0
                // If not normalized, ratio would be ~1.414
                runner.check(
                    "diagonal_speed_normalized",
                    ratio < 1.1,
                    &format!(
                        "Diagonal movement is {:.1}x faster than forward ({:.2} vs {:.2}). Speed not normalized.",
                        ratio, diag_dist, forward_dist
                    ),
                );
                runner.next_phase();
            }
        }

        // ---- Phase 16: Cursor lock — re-lock after unlock ----
        16 => {
            if frame == 1 {
                println!("\n[13] Cursor: re-lock after unlock");
                // First make sure cursor is locked (press Esc twice to toggle back)
                // We left it unlocked from mouse look tests potentially
                keyboard.press(KeyCode::Escape);
            }
            if frame == 3 {
                keyboard.release(KeyCode::Escape);
            }
            if frame == 10 {
                keyboard.press(KeyCode::Escape);
            }
            if frame == 12 {
                keyboard.release(KeyCode::Escape);
            }
            if frame >= 15 {
                // Check that double-escape returns to original state
                let w = window_q.single();
                // We don't know the current state for sure, but verify it's consistent
                let locked = w.cursor_options.grab_mode == CursorGrabMode::Locked;
                let hidden = !w.cursor_options.visible;
                runner.check(
                    "cursor_state_consistent",
                    locked == hidden,
                    &format!(
                        "Cursor state inconsistent: locked={}, hidden={}",
                        locked, hidden
                    ),
                );
                runner.next_phase();
            }
        }

        // ============================================================
        // FINAL OVERVIEW SCREENSHOTS
        // ============================================================

        // ---- Phase 17: Take overview screenshots from strategic positions ----
        17 => {
            if frame == 1 {
                println!("\n[14] Overview screenshots");
                // Reset camera to neutral
                let mut tf = player_q.single_mut();
                tf.translation = Vec3::new(0.0, 1.0, 8.0);
                mouse_events.send(bevy::input::mouse::MouseMotion {
                    delta: Vec2::new(0.0, 3000.0), // reset pitch to look forward-ish
                });
            }
            if frame >= 10 {
                screenshot(&mut commands, "12_overview_south");
                runner.next_phase();
            }
        }

        18 => {
            if frame == 1 {
                // Look at the scene from a high angle
                let mut tf = player_q.single_mut();
                tf.translation = Vec3::new(0.0, 1.0, 0.0);
            }
            if frame >= 5 {
                screenshot(&mut commands, "13_overview_center");
                runner.next_phase();
            }
        }

        // ---- Phase 19: Wait for screenshots to save ----
        19 => {
            if frame >= 15 {
                runner.next_phase();
            }
        }

        // ---- Phase 20: Exit ----
        _ => {
            runner.report();
            let success = runner.all_passed();
            if success {
                exit.send(AppExit::Success);
            } else {
                exit.send(AppExit::Error(1.try_into().unwrap()));
            }
        }
    }
}

fn main() {
    std::fs::create_dir_all(SCREENSHOT_DIR).unwrap();

    App::new()
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Hollowreach E2E Test".into(),
                        resolution: (1280.0, 720.0).into(),
                        ..default()
                    }),
                    ..default()
                })
                .set(bevy::log::LogPlugin {
                    level: bevy::log::Level::WARN,
                    ..default()
                })
                ,
        )
        .add_plugins(HollowreachPlugin)
        .insert_resource(TestRunner::new())
        .add_systems(
            Update,
            test_system
                .before(hollowreach::player_movement)
                .before(hollowreach::player_collision)
                .before(hollowreach::player_look)
                .before(hollowreach::interact_system)
                .before(hollowreach::proximity_hint_system)
                .before(hollowreach::dialogue_fade_system),
        )
        .run();
}
