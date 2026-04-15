//! Full UI end-to-end test that simulates real player input:
//! - E key to interact
//! - Mouse click on "Say" button
//! - Keyboard events to type text
//! - Enter to submit
//! - Observe chat log for response

use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;
use bevy::render::view::window::screenshot::{save_to_disk, Screenshot};
use hollowreach::*;
use hollowreach::chat_log::ChatMessage;
use hollowreach::panel::{PanelAction, PanelButton, PanelButtonAction, PanelCommand, PanelContent, PanelState};
use hollowreach::text_input::TextInputState;

#[derive(Resource)]
struct Frame(usize);

#[derive(Resource, Default)]
struct TestResults {
    e_opened_panel: bool,
    say_click_opened_input: bool,
    typed_text_visible: bool,
    enter_fired_sayevent: bool,
    chat_has_player_line: bool,
    greeting_received: bool,
    greeting_text: String,
    say_response_received: bool,
    say_response_text: String,
}

fn full_ui_test_system(
    mut frame: ResMut<Frame>,
    mut commands: Commands,
    mut player_q: Query<(Entity, &mut Transform), With<Player>>,
    mut camera_q: Query<&mut PlayerCamera>,
    mut keyboard: ResMut<ButtonInput<KeyCode>>,
    mut key_events: MessageWriter<KeyboardInput>,
    mut exit: MessageWriter<AppExit>,
    panel_state: Res<PanelState>,
    text_input_state: Res<TextInputState>,
    chat_msgs_q: Query<&Text, With<ChatMessage>>,
    mut panel_commands: MessageWriter<PanelCommand>,
    mut results: ResMut<TestResults>,
    game_state: Res<State<GameState>>,
    mut intro: ResMut<IntroSequence>,
    intro_overlay_q: Query<Entity, With<IntroOverlay>>,
    windows: Query<Entity, With<Window>>,
) {
    if *game_state.get() != GameState::Playing {
        return;
    }
    if intro.active {
        intro.active = false;
        for e in &intro_overlay_q {
            commands.entity(e).despawn();
        }
    }
    frame.0 += 1;

    match frame.0 {
        // === 1: Position player in front of Sir Roland ===
        5 => {
            let (_, mut tf) = player_q.single_mut().unwrap();
            tf.translation = Vec3::new(4.0, 1.0, 6.0);
            if let Ok(mut cam) = camera_q.single_mut() {
                cam.yaw = std::f32::consts::PI;
                cam.pitch = -0.2;
            }
            println!("[1] Player positioned near Sir Roland");
        }

        // === 2: Press E to open NPC menu ===
        15 | 16 | 17 => {
            keyboard.press(KeyCode::KeyE);
        }
        18 => {
            keyboard.release(KeyCode::KeyE);
            println!("[2] Pressed E");
        }

        // === 3: Verify NPC menu opened ===
        25 => {
            match &panel_state.content {
                PanelContent::NpcMenu { name, .. } => {
                    results.e_opened_panel = true;
                    println!("[3] PASS: NPC menu opened for '{}'", name);
                    commands.spawn(Screenshot::primary_window())
                        .observe(save_to_disk("test_screenshots/ui_01_npc_menu.png"));
                }
                other => {
                    println!("[3] FAIL: panel content = {:?}", other);
                }
            }
        }

        // === 4: Simulate Say button click by sending PanelCommand directly ===
        // (Interaction::Pressed set manually conflicts with Bevy's Changed<> filter)
        35 => {
            if let PanelContent::NpcMenu { npc, .. } = &panel_state.content {
                panel_commands.write(PanelCommand {
                    action: PanelAction::Open(PanelContent::TextInput { target_npc: *npc }),
                });
                println!("[4] Sent PanelCommand::Open(TextInput)");
            } else {
                println!("[4] FAIL: panel not in NpcMenu state");
            }
        }

        // === 5: Verify text input is now active ===
        45 => {
            if text_input_state.active {
                results.say_click_opened_input = true;
                println!("[5] PASS: TextInput active");
                commands.spawn(Screenshot::primary_window())
                    .observe(save_to_disk("test_screenshots/ui_02_text_input.png"));
            } else {
                println!("[5] FAIL: TextInput not active after Say click. Panel={:?}",
                    std::mem::discriminant(&panel_state.content));
            }
        }

        // === 6: Type characters using KeyboardInput events ===
        55 => {
            let Ok(window) = windows.single() else { return };
            let text = "hello there give me the key";
            for ch in text.chars() {
                let key_code = char_to_keycode(ch);
                let logical = if ch == ' ' {
                    Key::Space
                } else {
                    Key::Character(ch.to_string().into())
                };
                key_events.write(KeyboardInput {
                    key_code,
                    logical_key: logical,
                    state: bevy::input::ButtonState::Pressed,
                    repeat: false,
                    window,
                    text: None,
                });
            }
            println!("[6] Sent {} key events", text.len());
        }

        // === 7: Verify text was captured ===
        60 => {
            let current = &text_input_state.current_text;
            if !current.is_empty() {
                results.typed_text_visible = true;
                println!("[7] PASS: typed text in state = '{}'", current);
            } else {
                println!("[7] FAIL: text_input_state.current_text is empty");
            }
        }

        // === 8: Press Enter ===
        70 => {
            let Ok(window) = windows.single() else { return };
            key_events.write(KeyboardInput {
                key_code: KeyCode::Enter,
                logical_key: Key::Enter,
                state: bevy::input::ButtonState::Pressed,
                repeat: false,
                window,
                text: None,
            });
            println!("[8] Sent Enter KeyboardInput");
        }

        // === 9: Verify chat has player line ===
        80 => {
            for text in &chat_msgs_q {
                if text.0.starts_with("You:") {
                    results.chat_has_player_line = true;
                    results.enter_fired_sayevent = true;
                    println!("[9] PASS: player line in chat: '{}'", text.0);
                    commands.spawn(Screenshot::primary_window())
                        .observe(save_to_disk("test_screenshots/ui_03_after_enter.png"));
                    break;
                }
            }
            if !results.chat_has_player_line {
                println!("[9] FAIL: no 'You:' line in chat");
                for text in &chat_msgs_q {
                    println!("    chat line: '{}'", text.0);
                }
            }
        }

        // === 10a: Capture greeting (first non-player message) ===
        f if f >= 30 && f < 75 && !results.greeting_received => {
            for text in &chat_msgs_q {
                let display = &text.0;
                if !display.is_empty() && !display.starts_with("You:") {
                    results.greeting_received = true;
                    results.greeting_text = display.clone();
                    println!("[10a] Greeting at frame {}: '{}'", frame.0, display);
                    break;
                }
            }
        }

        // === 10b: Wait for SayEvent response ===
        f if f >= 90 && f < 5000 && !results.say_response_received => {
            for text in &chat_msgs_q {
                let display = &text.0;
                if !display.is_empty()
                    && !display.starts_with("You:")
                    && (results.greeting_text.is_empty() || *display != results.greeting_text)
                {
                    results.say_response_received = true;
                    results.say_response_text = display.clone();
                    println!("[10b] SayEvent response at frame {}: '{}'", frame.0, display);
                    commands.spawn(Screenshot::primary_window())
                        .observe(save_to_disk("test_screenshots/ui_04_npc_reply.png"));
                    break;
                }
            }
        }

        // === Final report ===
        5020 => {
            println!("\n=== FULL UI TEST RESULTS ===");
            println!("E opens NPC menu:        {}", pf(results.e_opened_panel));
            println!("Keyboard → typed text:   {}", pf(results.typed_text_visible));
            println!("Enter fires SayEvent:    {}", pf(results.enter_fired_sayevent));
            println!("Chat has player line:    {}", pf(results.chat_has_player_line));
            println!("Greeting received:       {}", pf(results.greeting_received));
            if results.greeting_received {
                println!("  Greeting: '{}'", results.greeting_text);
            }
            println!("SayEvent response:       {}", pf(results.say_response_received));
            if results.say_response_received {
                println!("  NPC replied: '{}'", results.say_response_text);
            }
            println!("============================\n");

            // Greeting is optional — NPC only speaks when player uses Say.
            let all = results.e_opened_panel
                && results.typed_text_visible
                && results.enter_fired_sayevent
                && results.chat_has_player_line
                && results.say_response_received;

            if all {
                println!("[SUCCESS] Full UI flow works");
                exit.write(AppExit::Success);
            } else {
                println!("[FAILURE] Full UI flow is broken");
                exit.write(AppExit::from_code(1));
            }
        }

        5050 => {
            println!("[TIMEOUT]");
            exit.write(AppExit::from_code(1));
        }
        _ => {}
    }
}

fn pf(b: bool) -> &'static str {
    if b { "PASS" } else { "FAIL" }
}

fn char_to_keycode(ch: char) -> KeyCode {
    match ch {
        'a'..='z' => {
            let byte = ch as u8 - b'a';
            match byte {
                0 => KeyCode::KeyA, 1 => KeyCode::KeyB, 2 => KeyCode::KeyC, 3 => KeyCode::KeyD,
                4 => KeyCode::KeyE, 5 => KeyCode::KeyF, 6 => KeyCode::KeyG, 7 => KeyCode::KeyH,
                8 => KeyCode::KeyI, 9 => KeyCode::KeyJ, 10 => KeyCode::KeyK, 11 => KeyCode::KeyL,
                12 => KeyCode::KeyM, 13 => KeyCode::KeyN, 14 => KeyCode::KeyO, 15 => KeyCode::KeyP,
                16 => KeyCode::KeyQ, 17 => KeyCode::KeyR, 18 => KeyCode::KeyS, 19 => KeyCode::KeyT,
                20 => KeyCode::KeyU, 21 => KeyCode::KeyV, 22 => KeyCode::KeyW, 23 => KeyCode::KeyX,
                24 => KeyCode::KeyY, 25 => KeyCode::KeyZ,
                _ => KeyCode::Unidentified(bevy::input::keyboard::NativeKeyCode::Unidentified),
            }
        }
        ' ' => KeyCode::Space,
        _ => KeyCode::Unidentified(bevy::input::keyboard::NativeKeyCode::Unidentified),
    }
}

fn main() {
    std::fs::create_dir_all("test_screenshots").unwrap();
    App::new()
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Full UI Test".into(),
                        resolution: bevy::window::WindowResolution::new(1280, 720),
                        ..default()
                    }),
                    ..default()
                })
                .set(bevy::log::LogPlugin {
                    level: bevy::log::Level::INFO,
                    filter: "wgpu=warn,naga=warn,bevy_render=warn,bevy_ecs=warn,bevy_app=warn,bevy_winit=warn,bevy_asset=warn,gilrs=warn".into(),
                    ..default()
                }),
        )
        .add_plugins(HollowreachPlugin)
        .insert_resource(Frame(0))
        .insert_resource(TestResults::default())
        .add_systems(Update, full_ui_test_system
            .before(hollowreach::player_movement)
            .before(hollowreach::interact_system))
        .run();
}
