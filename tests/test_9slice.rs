use bevy::prelude::*;
use bevy::ui::widget::NodeImageMode;
use bevy::render::view::screenshot::{save_to_disk, Screenshot};

#[derive(Resource)]
struct Frame(usize);

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "9-slice Test".into(),
                resolution: (1280.0, 720.0).into(),
                ..default()
            }),
            ..default()
        }))
        .insert_resource(Frame(0))
        .add_systems(Startup, setup)
        .add_systems(Update, take_screenshot)
        .run();
}

fn take_screenshot(mut frame: ResMut<Frame>, mut commands: Commands, mut exit: EventWriter<AppExit>) {
    frame.0 += 1;
    if frame.0 == 30 {
        commands.spawn(Screenshot::primary_window())
            .observe(save_to_disk("test_screenshots/9slice_test.png"));
    }
    if frame.0 == 45 {
        exit.send(AppExit::Success);
    }
}

fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.spawn(Camera2d);

    let panel = asset_server.load("ui/Panel/panel-012.png");
    let slicer = TextureSlicer {
        border: BorderRect::square(18.0),
        center_scale_mode: SliceScaleMode::Stretch,
        sides_scale_mode: SliceScaleMode::Tile { stretch_value: 3.0 },
        max_corner_scale: 1.0,
    };

    commands.spawn(Node {
        width: Val::Percent(100.0),
        height: Val::Percent(100.0),
        flex_direction: FlexDirection::Row,
        justify_content: JustifyContent::SpaceEvenly,
        align_items: AlignItems::Center,
        flex_wrap: FlexWrap::Wrap,
        padding: UiRect::all(Val::Px(20.0)),
        ..default()
    }).with_children(|parent| {
        for (w, h, label) in [
            (200.0, 100.0, "Small"),
            (400.0, 150.0, "Medium"),
            (600.0, 200.0, "Large"),
            (300.0, 300.0, "Square"),
        ] {
            parent.spawn((
                ImageNode {
                    image: panel.clone(),
                    image_mode: NodeImageMode::Sliced(slicer.clone()),
                    color: Color::srgba(0.0, 0.0, 0.0, 0.5),
                    ..default()
                },
                Node {
                    width: Val::Px(w),
                    height: Val::Px(h),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    margin: UiRect::all(Val::Px(10.0)),
                    ..default()
                },
            )).with_children(|p| {
                p.spawn((
                    Text::new(label),
                    TextFont { font_size: 24.0, ..default() },
                    TextColor(Color::WHITE),
                ));
            });
        }
    });
}
