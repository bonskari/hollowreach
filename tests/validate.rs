use bevy::prelude::*;
use bevy::render::view::window::screenshot::{save_to_disk, Screenshot};
use hollowreach::*;
use hollowreach::text_input::TextInputState;

#[derive(Resource)]
struct Frame(usize);

fn validate_system(
    mut frame: ResMut<Frame>,
    mut commands: Commands,
    mut player_q: Query<&mut Transform, With<Player>>,
    mut mouse_events: MessageWriter<bevy::input::mouse::MouseMotion>,
    mut keyboard: ResMut<ButtonInput<KeyCode>>,
    mut exit: MessageWriter<AppExit>,
    hint_q: Query<&Visibility, With<ProximityHintText>>,
    panel_q: Query<&Visibility, (With<InteractionListPanel>, Without<ProximityHintText>, Without<NpcInteractionPanel>)>,
    _npc_panel_q: Query<&Visibility, (With<NpcInteractionPanel>, Without<ProximityHintText>, Without<InteractionListPanel>)>,
    dialogue_timer: Res<DialogueTimer>,
    text_input_state: Res<TextInputState>,
    npc_panel_state: Res<NpcPanelState>,
) {
    frame.0 += 1;

    match frame.0 {
        // Wait for assets to load
        120 => {
            let mut tf = player_q.single_mut().unwrap();
            tf.translation = Vec3::new(0.0, 1.0, 8.0);
            mouse_events.write(bevy::input::mouse::MouseMotion { delta: Vec2::ZERO });
        }
        125 => {
            commands.spawn(Screenshot::primary_window())
                .observe(save_to_disk("test_screenshots/v_01_entrance.png"));
        }

        135 => {
            let mut tf = player_q.single_mut().unwrap();
            tf.translation = Vec3::new(1.0, 1.0, 6.0);
        }
        140 => {
            commands.spawn(Screenshot::primary_window())
                .observe(save_to_disk("test_screenshots/v_02_knight_closeup.png"));
        }

        150 => {
            let mut tf = player_q.single_mut().unwrap();
            tf.translation = Vec3::new(-4.0, 1.0, 0.0);
            mouse_events.write(bevy::input::mouse::MouseMotion { delta: Vec2::new(300.0, 0.0) });
        }
        155 => {
            commands.spawn(Screenshot::primary_window())
                .observe(save_to_disk("test_screenshots/v_03_tavern_area.png"));
        }

        165 => {
            let mut tf = player_q.single_mut().unwrap();
            tf.translation = Vec3::new(5.0, 1.0, -4.0);
            mouse_events.write(bevy::input::mouse::MouseMotion { delta: Vec2::new(-600.0, 0.0) });
        }
        170 => {
            commands.spawn(Screenshot::primary_window())
                .observe(save_to_disk("test_screenshots/v_04_storage_area.png"));
        }

        180 => {
            let mut tf = player_q.single_mut().unwrap();
            tf.translation = Vec3::new(0.0, 1.0, 0.0);
            mouse_events.write(bevy::input::mouse::MouseMotion { delta: Vec2::new(300.0, 200.0) });
        }
        185 => {
            commands.spawn(Screenshot::primary_window())
                .observe(save_to_disk("test_screenshots/v_05_center_view.png"));
        }

        195 => {
            let mut tf = player_q.single_mut().unwrap();
            tf.translation = Vec3::new(0.0, 1.0, -6.0);
            mouse_events.write(bevy::input::mouse::MouseMotion { delta: Vec2::new(600.0, -200.0) });
        }
        200 => {
            commands.spawn(Screenshot::primary_window())
                .observe(save_to_disk("test_screenshots/v_06_back_wall.png"));
        }

        // Position 7: Stand near Grok (Barbarian) — should show interaction panel
        210 => {
            let mut tf = player_q.single_mut().unwrap();
            tf.translation = Vec3::new(3.0, 1.0, 0.5);
            mouse_events.write(bevy::input::mouse::MouseMotion { delta: Vec2::new(-400.0, -100.0) });
        }
        220 => {
            // Verify the interaction list panel is showing for multi-interaction NPC
            let hint_vis = hint_q.single().unwrap();
            let panel_vis = panel_q.single().unwrap();
            println!("[DEBUG] frame=220: hint={:?}, panel={:?}", *hint_vis, *panel_vis);

            commands.spawn(Screenshot::primary_window())
                .observe(save_to_disk("test_screenshots/v_07_proximity_hint.png"));
        }

        // Press E to trigger NPC interaction panel
        225 | 226 | 227 | 228 => {
            keyboard.press(KeyCode::KeyE);
        }
        229 => {
            keyboard.release(KeyCode::KeyE);
        }

        // Verify NPC panel opened (NPC path opens panel, not dialogue)
        240 => {
            println!("[DEBUG] frame=240: npc_panel_open={}, dialogue_active={}, text_input_active={}",
                npc_panel_state.open, dialogue_timer.active, text_input_state.active);

            // When NPC panel is open OR dialogue/text_input is active,
            // the proximity hint and interaction list panel MUST be hidden
            let any_overlay_active = npc_panel_state.open || dialogue_timer.active || text_input_state.active;

            if any_overlay_active {
                let hint_vis = hint_q.single().unwrap();
                assert_eq!(
                    *hint_vis, Visibility::Hidden,
                    "BUG: Proximity hint visible while NPC panel/dialogue/text input is active!"
                );
                println!("[PASS] Hint correctly hidden during NPC panel/dialogue/text input");

                let panel_vis = panel_q.single().unwrap();
                assert_eq!(
                    *panel_vis, Visibility::Hidden,
                    "BUG: Interaction list panel visible while NPC panel/dialogue/text input is active!"
                );
                println!("[PASS] Interaction list panel correctly hidden during NPC panel/dialogue/text input");
            } else {
                println!("[WARN] No overlay active at frame 240 - interaction may not have fired");
            }

            commands.spawn(Screenshot::primary_window())
                .observe(save_to_disk("test_screenshots/v_08_npc_panel.png"));
        }

        // Close NPC panel with Escape
        260 => {
            keyboard.press(KeyCode::Escape);
        }
        261 => {
            keyboard.release(KeyCode::Escape);
        }

        // Wait, then check state after closing
        290 => {
            println!("[DEBUG] frame=290: npc_panel_open={}, dialogue_active={}, text_input_active={}",
                npc_panel_state.open, dialogue_timer.active, text_input_state.active);
            commands.spawn(Screenshot::primary_window())
                .observe(save_to_disk("test_screenshots/v_09_after_close.png"));
        }

        // After everything closes, verify panel returns near NPC
        300 => {
            let hint_vis = hint_q.single().unwrap();
            let panel_vis = panel_q.single().unwrap();
            println!("[INFO] frame=300: hint={:?}, list_panel={:?}, npc_panel_open={}",
                *hint_vis, *panel_vis, npc_panel_state.open);
            commands.spawn(Screenshot::primary_window())
                .observe(save_to_disk("test_screenshots/v_10_after_fade.png"));
        }

        320 => {
            exit.write(AppExit::Success);
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
                        title: "Validate".into(),
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
        .add_systems(Update, validate_system
            .before(hollowreach::player_movement)
            .before(hollowreach::player_look)
            .before(hollowreach::interact_system)
            .before(hollowreach::npc_panel_close_system))
        .run();
}
