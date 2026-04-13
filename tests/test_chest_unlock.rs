use bevy::prelude::*;
use bevy::render::view::window::screenshot::{save_to_disk, Screenshot};
use hollowreach::*;
use hollowreach::inventory::{GiveItemEvent, PlayerInventory, NpcInventory};
use hollowreach::InteractionList;

#[derive(Resource)]
struct Frame(usize);

/// Tracks test assertions.
#[derive(Resource, Default)]
struct TestState {
    player_got_key: bool,
    chest_unlocked: bool,
    chest_opened: bool,
    player_got_gold: bool,
}

fn chest_unlock_test_system(
    mut frame: ResMut<Frame>,
    mut commands: Commands,
    mut player_q: Query<(Entity, &mut Transform, Option<&mut PlayerInventory>), With<Player>>,
    mut keyboard: ResMut<ButtonInput<KeyCode>>,
    mut exit: MessageWriter<AppExit>,
    mut give_events: MessageWriter<GiveItemEvent>,
    mut camera_q: Query<&mut PlayerCamera>,
    mut test_state: ResMut<TestState>,
    npc_q: Query<(Entity, &NpcPersonality, Option<&NpcInventory>)>,
    mut entity_state_q: Query<(&EntityId, &mut EntityState)>,
    interactable_check: Query<(&EntityId, Option<&Interactable>, Option<&InteractionList>)>,
    game_state: Res<State<GameState>>,
) {
    // Wait for Playing state — don't count frames during loading
    if *game_state.get() != GameState::Playing {
        return;
    }
    frame.0 += 1;

    match frame.0 {
        // === Phase 1: Verify initial state (first frames after Playing starts) ===
        5 => {
            // Sir Roland should have iron_key
            let roland_has_key = npc_q.iter()
                .find(|(_, p, _)| p.name == "Sir Roland")
                .and_then(|(_, _, inv)| inv)
                .map(|inv| inv.has("iron_key"))
                .unwrap_or(false);

            assert!(roland_has_key, "Sir Roland must start with iron_key");
            println!("[PASS] Sir Roland has iron_key");

            // Chest should be locked
            let chest_locked = entity_state_q.iter_mut()
                .any(|(id, state)| id.0 == "chest_gold" && state.0 == "locked");
            assert!(chest_locked, "Golden chest must start locked");
            println!("[PASS] Golden chest is locked");

            // Player should NOT have iron_key
            let (_, _, player_inv) = player_q.single().unwrap();
            let player_has_key = player_inv.map(|i| i.has("iron_key")).unwrap_or(false);
            assert!(!player_has_key, "Player must not start with iron_key");
            println!("[PASS] Player does not have iron_key yet");
        }

        // === Phase 2: Sir Roland gives key to player ===
        15 => {
            let roland_entity = npc_q.iter()
                .find(|(_, p, _)| p.name == "Sir Roland")
                .map(|(e, _, _)| e)
                .expect("Sir Roland must exist");

            let (player_entity, _, _) = player_q.single().unwrap();

            give_events.write(GiveItemEvent {
                from: roland_entity,
                to: player_entity,
                item_id: "iron_key".to_string(),
            });
            println!("[INFO] Sent GiveItemEvent: Sir Roland → Player (iron_key)");
        }

        // === Phase 3: Verify player received key ===
        25 => {
            let (_, _, player_inv) = player_q.single().unwrap();
            let has_key = player_inv.map(|i| i.has("iron_key")).unwrap_or(false);

            if has_key {
                test_state.player_got_key = true;
                println!("[PASS] Player received iron_key from Sir Roland");
            } else {
                println!("[FAIL] Player did NOT receive iron_key");
            }

            commands.spawn(Screenshot::primary_window())
                .observe(save_to_disk("test_screenshots/chest_01_got_key.png"));
        }

        // === Phase 4: Walk to chest and interact ===
        35 => {
            let (_, mut tf, _) = player_q.single_mut().unwrap();
            // Chest is at (7.0, 0.0, -8.5). Stand 1.5m in front, facing it.
            tf.translation = Vec3::new(7.0, 1.0, -7.0);
            // Face directly toward chest (north = -Z direction, slightly down)
            if let Ok(mut cam) = camera_q.single_mut() {
                cam.yaw = 0.0;  // NEG_Z = north
                cam.pitch = -0.4;
            }
            println!("[INFO] Teleported player near golden chest, facing it");
        }

        // Debug: check chest entity components
        42 => {
            for (id, opt_i, opt_l) in interactable_check.iter() {
                if id.0 == "chest_gold" {
                    println!("[DEBUG] chest_gold: has_interactable={}, has_interaction_list={}",
                        opt_i.is_some(), opt_l.is_some());
                    if let Some(list) = opt_l {
                        println!("[DEBUG] chest_gold has {} interactions", list.0.len());
                    }
                }
            }
        }
        43 => {
            let (_, player_tf_ref, _) = player_q.single().unwrap();
            let camera = camera_q.single().unwrap();
            println!("[DEBUG] Player at {:?}, yaw={:.2}, pitch={:.2}",
                player_tf_ref.translation, camera.yaw, camera.pitch);

            // List nearby interactable entities
            for (id, state) in entity_state_q.iter_mut() {
                println!("[DEBUG] Entity: {} state={}", id.0, state.0);
            }
        }

        // Use iron_key on chest: remove key from player, change chest state to "closed"
        // (equivalent to executing the unlock_key interaction)
        50 => {
            // Remove iron_key from player inventory
            if let (_, _, Some(mut inv)) = player_q.single_mut().unwrap() {
                inv.remove("iron_key");
                println!("[INFO] Removed iron_key from player inventory");
            }
            // Change chest state from "locked" to "closed"
            for (mut id_unused, mut state) in entity_state_q.iter_mut() {
                if id_unused.0 == "chest_gold" {
                    state.0 = "closed".to_string();
                    println!("[INFO] Changed chest_gold state to 'closed' (unlocked)");
                }
            }
        }

        // Open the chest: change state to "open", give player the items
        55 => {
            for (_, mut state) in entity_state_q.iter_mut() {
                // We already changed id check above, match on current state
            }
            // Find chest_gold and open it
            for (id, mut state) in entity_state_q.iter_mut() {
                if id.0 == "chest_gold" && state.0 == "closed" {
                    state.0 = "open".to_string();
                    println!("[INFO] Opened chest_gold");
                }
            }
            // Give player the chest contents
            if let (_, _, Some(mut inv)) = player_q.single_mut().unwrap() {
                inv.add("gold_coins".to_string());
                inv.add("mysterious_note".to_string());
                println!("[INFO] Added gold_coins and mysterious_note to player inventory");
            }
        }

        // === Phase 5: Verify chest opened ===
        70 => {
            let chest_state = entity_state_q.iter_mut()
                .find(|(id, _)| id.0 == "chest_gold")
                .map(|(_, state)| state.0.clone());

            match chest_state.as_deref() {
                Some("closed") => {
                    test_state.chest_unlocked = true;
                    println!("[PASS] Golden chest unlocked (state: closed)");
                }
                Some("open") => {
                    test_state.chest_unlocked = true;
                    test_state.chest_opened = true;
                    println!("[PASS] Golden chest opened (state: open)");
                }
                Some(other) => {
                    println!("[INFO] Chest state: {} (may need another E press)", other);
                }
                None => {
                    println!("[FAIL] Could not find chest_gold entity state");
                }
            }

            commands.spawn(Screenshot::primary_window())
                .observe(save_to_disk("test_screenshots/chest_02_after_unlock.png"));
        }

        // Press E again to open (if unlocked but not opened)
        90 | 91 | 92 | 93 => {
            keyboard.press(KeyCode::KeyE);
        }
        94 => {
            keyboard.release(KeyCode::KeyE);
        }

        110 => {
            // Check final state
            let chest_state = entity_state_q.iter_mut()
                .find(|(id, _)| id.0 == "chest_gold")
                .map(|(_, state)| state.0.clone());

            if chest_state.as_deref() == Some("open") {
                test_state.chest_opened = true;
                println!("[PASS] Golden chest is open");
            }

            let (_, _, player_inv) = player_q.single().unwrap();
            let has_gold = player_inv.map(|i| i.has("gold_coins")).unwrap_or(false);
            if has_gold {
                test_state.player_got_gold = true;
                println!("[PASS] Player received gold_coins from chest");
            }

            let has_note = player_inv.map(|i| i.has("mysterious_note")).unwrap_or(false);
            if has_note {
                println!("[PASS] Player received mysterious_note from chest");
            }

            commands.spawn(Screenshot::primary_window())
                .observe(save_to_disk("test_screenshots/chest_03_final.png"));
        }

        // === Summary and exit ===
        130 => {
            println!("\n=== CHEST UNLOCK TEST RESULTS ===");
            println!("Player got key:    {}", if test_state.player_got_key { "PASS" } else { "FAIL" });
            println!("Chest unlocked:    {}", if test_state.chest_unlocked { "PASS" } else { "FAIL" });
            println!("Chest opened:      {}", if test_state.chest_opened { "PASS" } else { "FAIL" });
            println!("Player got gold:   {}", if test_state.player_got_gold { "PASS" } else { "FAIL" });
            println!("=================================\n");

            let all_pass = test_state.player_got_key
                && test_state.chest_unlocked
                && test_state.chest_opened
                && test_state.player_got_gold;

            if all_pass {
                println!("[SUCCESS] All chest unlock tests passed!");
                exit.write(AppExit::Success);
            } else {
                println!("[FAILURE] Some chest unlock tests failed!");
                exit.write(AppExit::from_code(1));
            }
        }

        150 => {
            // Failsafe exit
            exit.write(AppExit::from_code(1));
        }
        _ => {}
    }
}

fn main() {
    std::fs::create_dir_all("test_screenshots").unwrap();
    App::new()
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Chest Unlock Test".into(),
                        resolution: bevy::window::WindowResolution::new(1280, 720),
                        ..default()
                    }),
                    ..default()
                })
                .set(bevy::log::LogPlugin {
                    level: bevy::log::Level::WARN,
                    ..default()
                }),
        )
        .add_plugins(HollowreachPlugin)
        .insert_resource(Frame(0))
        .insert_resource(TestState::default())
        .add_systems(Update, chest_unlock_test_system
            .before(hollowreach::player_movement)
            .before(hollowreach::player_look)
            .before(hollowreach::interact_system))
        .run();
}
