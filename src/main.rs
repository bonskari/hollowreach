use bevy::prelude::*;
use bevy::window::WindowMode;
use hollowreach::HollowreachPlugin;

fn main() {
    App::new()
        .register_asset_source(
            "tts",
            bevy::asset::io::AssetSourceBuilder::platform_default(
                &format!("{}/.cache/hollowreach/tts", std::env::var("HOME").unwrap_or_default()),
                None,
            ),
        )
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
