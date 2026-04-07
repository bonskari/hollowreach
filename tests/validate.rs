use bevy::prelude::*;
use bevy::render::view::screenshot::{save_to_disk, Screenshot};
use hollowreach::*;

#[derive(Resource)]
struct Frame(usize);

fn validate_system(
    mut frame: ResMut<Frame>,
    mut commands: Commands,
    mut player_q: Query<&mut Transform, With<Player>>,
    mut mouse_events: EventWriter<bevy::input::mouse::MouseMotion>,
    mut exit: EventWriter<AppExit>,
) {
    frame.0 += 1;

    match frame.0 {
        // Wait for assets to load
        120 => {
            // Position 1: Looking at the village from spawn point (south, looking north)
            let mut tf = player_q.single_mut();
            tf.translation = Vec3::new(0.0, 1.0, 8.0);
            // Reset camera to look forward
            mouse_events.send(bevy::input::mouse::MouseMotion { delta: Vec2::ZERO });
        }
        125 => {
            commands.spawn(Screenshot::primary_window())
                .observe(save_to_disk("test_screenshots/v_01_entrance.png"));
        }

        // Position 2: Close-up on Knight NPC
        135 => {
            let mut tf = player_q.single_mut();
            tf.translation = Vec3::new(1.0, 1.0, 6.0);
        }
        140 => {
            commands.spawn(Screenshot::primary_window())
                .observe(save_to_disk("test_screenshots/v_02_knight_closeup.png"));
        }

        // Position 3: Looking at tavern area (left side with table/chairs)
        150 => {
            let mut tf = player_q.single_mut();
            tf.translation = Vec3::new(-4.0, 1.0, 0.0);
            mouse_events.send(bevy::input::mouse::MouseMotion { delta: Vec2::new(300.0, 0.0) });
        }
        155 => {
            commands.spawn(Screenshot::primary_window())
                .observe(save_to_disk("test_screenshots/v_03_tavern_area.png"));
        }

        // Position 4: Looking at barrel/storage area (right side)
        165 => {
            let mut tf = player_q.single_mut();
            tf.translation = Vec3::new(5.0, 1.0, -4.0);
            mouse_events.send(bevy::input::mouse::MouseMotion { delta: Vec2::new(-600.0, 0.0) });
        }
        170 => {
            commands.spawn(Screenshot::primary_window())
                .observe(save_to_disk("test_screenshots/v_04_storage_area.png"));
        }

        // Position 5: Overview from high angle (center, looking down)
        180 => {
            let mut tf = player_q.single_mut();
            tf.translation = Vec3::new(0.0, 1.0, 0.0);
            mouse_events.send(bevy::input::mouse::MouseMotion { delta: Vec2::new(300.0, 200.0) });
        }
        185 => {
            commands.spawn(Screenshot::primary_window())
                .observe(save_to_disk("test_screenshots/v_05_center_view.png"));
        }

        // Position 6: Back wall with banners
        195 => {
            let mut tf = player_q.single_mut();
            tf.translation = Vec3::new(0.0, 1.0, -6.0);
            mouse_events.send(bevy::input::mouse::MouseMotion { delta: Vec2::new(600.0, -200.0) });
        }
        200 => {
            commands.spawn(Screenshot::primary_window())
                .observe(save_to_disk("test_screenshots/v_06_back_wall.png"));
        }

        210 => {
            exit.send(AppExit::Success);
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
                        resolution: (1280.0, 720.0).into(),
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
            .before(hollowreach::player_look))
        .run();
}
