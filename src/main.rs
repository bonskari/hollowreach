use bevy::prelude::*;
use bevy::window::WindowMode;
use hollowreach::HollowreachPlugin;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Hollowreach".into(),
                mode: WindowMode::BorderlessFullscreen(MonitorSelection::Current),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(HollowreachPlugin)
        .run();
}
