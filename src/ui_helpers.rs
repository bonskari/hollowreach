//! Shared UI helpers — UiAssets resource, panel/button spawn functions, color constants.
//! Every menu in the game uses these instead of duplicating image loads and slicer configs.

use bevy::prelude::*;
use bevy::ui::widget::NodeImageMode;

use bevy::ecs::hierarchy::ChildSpawnerCommands;

// ---------------------------------------------------------------------------
// Color constants
// ---------------------------------------------------------------------------

/// Gold text color for titles / NPC names.
pub const COLOR_GOLD: Color = Color::srgb(0.95, 0.82, 0.4);
/// Subtitle / secondary text.
pub const COLOR_GREY: Color = Color::srgb(0.7, 0.7, 0.7);
/// Button label text (dark on light button).
pub const COLOR_BUTTON_TEXT: Color = Color::srgba(0.0, 0.0, 0.0, 1.0);
/// Body / dialogue text.
pub const COLOR_BODY: Color = Color::srgba(0.9, 0.9, 0.9, 1.0);
/// Panel background tint (black 80%).
pub const COLOR_PANEL_BG: Color = Color::srgba(0.0, 0.0, 0.0, 0.8);
/// Button hover tint.
pub const COLOR_BTN_HOVER: Color = Color::srgba(0.8, 0.8, 0.8, 1.0);
/// Button press tint.
pub const COLOR_BTN_PRESS: Color = Color::srgba(0.6, 0.6, 0.6, 1.0);

// ---------------------------------------------------------------------------
// UiAssets resource
// ---------------------------------------------------------------------------

/// Shared UI image assets — loaded once at startup, used by all menus.
#[derive(Resource)]
pub struct UiAssets {
    pub panel_image: Handle<Image>,
    pub button_image: Handle<Image>,
    pub divider_image: Handle<Image>,
    pub panel_slicer: TextureSlicer,
}

pub fn setup_ui_assets(mut commands: Commands, asset_server: Res<AssetServer>) {
    let panel_image = asset_server.load_with_settings(
        "ui/Panel/panel-012.png",
        |s: &mut bevy::image::ImageLoaderSettings| {
            s.sampler = bevy::image::ImageSampler::nearest();
        },
    );
    let button_image = asset_server.load_with_settings(
        "ui/Panel/panel-012.png",
        |s: &mut bevy::image::ImageLoaderSettings| {
            s.sampler = bevy::image::ImageSampler::nearest();
        },
    );
    let divider_image = asset_server.load("ui/Divider Fade/divider-fade-003.png");
    let panel_slicer = TextureSlicer {
        border: BorderRect::all(18.0),
        center_scale_mode: SliceScaleMode::Stretch,
        sides_scale_mode: SliceScaleMode::Tile { stretch_value: 3.0 },
        max_corner_scale: 2.0,
    };

    commands.insert_resource(UiAssets {
        panel_image,
        button_image,
        divider_image,
        panel_slicer,
    });
}

// ---------------------------------------------------------------------------
// Spawn helpers
// ---------------------------------------------------------------------------

/// Returns an `ImageNode` configured as a 9-slice panel with the standard dark tint.
pub fn panel_image_node(ui: &UiAssets) -> ImageNode {
    ImageNode {
        image: ui.panel_image.clone(),
        image_mode: NodeImageMode::Sliced(ui.panel_slicer.clone()),
        color: COLOR_PANEL_BG,
        ..default()
    }
}

/// Returns an `ImageNode` configured as a 9-slice button (white, no tint).
pub fn button_image_node(ui: &UiAssets) -> ImageNode {
    ImageNode {
        image: ui.button_image.clone(),
        image_mode: NodeImageMode::Sliced(ui.panel_slicer.clone()),
        ..default()
    }
}

/// Spawns a standard button with label text. Returns the button entity.
pub fn spawn_button<C: Component>(
    parent: &mut ChildSpawnerCommands,
    ui: &UiAssets,
    label: &str,
    marker: C,
) -> Entity {
    parent
        .spawn((
            marker,
            Button,
            button_image_node(ui),
            BackgroundColor(Color::NONE),
            Node {
                width: Val::Px(220.0),
                height: Val::Px(38.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                margin: UiRect::vertical(Val::Px(4.0)),
                ..default()
            },
        ))
        .with_children(|btn| {
            btn.spawn((
                Text::new(label),
                TextFont { font_size: 17.0, ..default() },
                TextColor(COLOR_BUTTON_TEXT),
            ));
        })
        .id()
}

/// Spawns a divider image (fade line).
pub fn spawn_divider(parent: &mut ChildSpawnerCommands, ui: &UiAssets) {
    parent.spawn((
        ImageNode::new(ui.divider_image.clone()),
        Node {
            width: Val::Percent(90.0),
            height: Val::Px(6.0),
            margin: UiRect::vertical(Val::Px(8.0)),
            ..default()
        },
    ));
}

/// Generic hover system for any button with an `ImageNode`.
/// Tints on hover/press, resets on none. Works for any button type.
pub fn button_hover_system(
    mut q: Query<(&Interaction, &mut ImageNode), (Changed<Interaction>, With<Button>)>,
) {
    for (interaction, mut img) in &mut q {
        match *interaction {
            Interaction::Hovered => img.color = COLOR_BTN_HOVER,
            Interaction::Pressed => img.color = COLOR_BTN_PRESS,
            Interaction::None => img.color = Color::WHITE,
        }
    }
}
