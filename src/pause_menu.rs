//! Pause menu — Esc toggles a menu with Resume, Save, Settings, Quit.
//! While open: game is paused, cursor is visible.

use bevy::prelude::*;
use bevy::window::CursorGrabMode;
use bevy::ui::widget::NodeImageMode;

#[derive(Resource)]
pub struct PauseState {
    pub paused: bool,
}

impl Default for PauseState {
    fn default() -> Self {
        Self { paused: false }
    }
}

#[derive(Component)]
pub struct PauseOverlay;

#[derive(Component)]
pub struct PauseButton {
    pub action: PauseAction,
}

#[derive(Clone, Copy)]
pub enum PauseAction {
    Resume,
    Save,
    Settings,
    Quit,
}

pub struct PauseMenuPlugin;

impl Plugin for PauseMenuPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PauseState>()
            .add_systems(Startup, setup_pause_menu)
            .add_systems(Update, (toggle_pause, pause_button_system));
    }
}

fn setup_pause_menu(mut commands: Commands, asset_server: Res<AssetServer>) {
    let panel_image = asset_server.load("ui/Panel/panel-012.png");
    let button_image: Handle<Image> = asset_server.load("ui/Panel/panel-003.png");
    let slicer = TextureSlicer {
        border: BorderRect::square(18.0),
        center_scale_mode: SliceScaleMode::Stretch,
        sides_scale_mode: SliceScaleMode::Tile { stretch_value: 3.0 },
        max_corner_scale: 2.0,
    };

    commands
        .spawn((
            PauseOverlay,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
            GlobalZIndex(200),
            Visibility::Hidden,
        ))
        .with_children(|parent| {
            // Menu panel
            parent
                .spawn((
                    Node {
                        flex_direction: FlexDirection::Column,
                        align_items: AlignItems::Center,
                        padding: UiRect::all(Val::Px(30.0)),
                        row_gap: Val::Px(12.0),
                        min_width: Val::Px(250.0),
                        ..default()
                    },
                    
                ))
                .with_children(|panel| {
                    // 9-slice border
                    panel.spawn((
                        ImageNode {
                            image: panel_image.clone(),
                            image_mode: NodeImageMode::Sliced(slicer.clone()),
                            
                            ..default()
                        },
                        Node {
                            position_type: PositionType::Absolute,
                            top: Val::Px(-4.0),
                            left: Val::Px(-4.0),
                            right: Val::Px(-4.0),
                            bottom: Val::Px(-4.0),
                            ..default()
                        },
                    ));

                    // Title
                    panel.spawn((
                        Text::new("Paused"),
                        TextFont { font_size: 32.0, ..default() },
                        TextColor(Color::srgb(0.95, 0.82, 0.4)),
                        Node { margin: UiRect::bottom(Val::Px(16.0)), ..default() },
                    ));

                    // Buttons
                    for (label, action) in [
                        ("Resume", PauseAction::Resume),
                        ("Save", PauseAction::Save),
                        ("Settings", PauseAction::Settings),
                        ("Quit", PauseAction::Quit),
                    ] {
                        panel
                            .spawn((
                                PauseButton { action },
                                Button,
                                Node {
                                    width: Val::Px(200.0),
                                    height: Val::Px(40.0),
                                    justify_content: JustifyContent::Center,
                                    align_items: AlignItems::Center,
                                    ..default()
                                },
                                
                                BorderRadius::all(Val::Px(4.0)),
                            ))
                            .with_children(|btn| {
                                btn.spawn((
                                    Text::new(label),
                                    TextFont { font_size: 18.0, ..default() },
                                    TextColor(Color::srgba(0.0, 0.0, 0.0, 1.0)),
                                ));
                            });
                    }
                });
        });
}

fn toggle_pause(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut pause: ResMut<PauseState>,
    mut overlay_q: Query<&mut Visibility, With<PauseOverlay>>,
    mut windows: Query<&mut Window>,
    npc_panel_state: Res<crate::NpcPanelState>,
    text_input_state: Res<crate::text_input::TextInputState>,
) {
    if keyboard.just_pressed(KeyCode::Escape) {
        // Don't toggle pause if NPC panel or text input is open — Esc closes those first
        if npc_panel_state.open || text_input_state.active {
            return;
        }
        pause.paused = !pause.paused;

        let mut vis = overlay_q.single_mut();
        let mut window = windows.single_mut();

        if pause.paused {
            *vis = Visibility::Visible;
            window.cursor_options.grab_mode = CursorGrabMode::None;
            window.cursor_options.visible = true;
        } else {
            *vis = Visibility::Hidden;
            window.cursor_options.grab_mode = CursorGrabMode::Locked;
            window.cursor_options.visible = false;
        }
    }
}

fn pause_button_system(
    mut interaction_q: Query<(&Interaction, &PauseButton, &mut BackgroundColor), Changed<Interaction>>,
    mut pause: ResMut<PauseState>,
    mut overlay_q: Query<&mut Visibility, With<PauseOverlay>>,
    mut windows: Query<&mut Window>,
    mut exit: EventWriter<AppExit>,
    mut audio_settings: ResMut<crate::AudioSettings>,
) {
    for (interaction, button, mut bg) in &mut interaction_q {
        match *interaction {
            Interaction::Hovered => {
                *bg = // hover handled separately
                continue;
            }
            Interaction::None => {
                *bg = BackgroundColor(Color::srgba(0.15, 0.12, 0.2, 0.8));
                continue;
            }
            Interaction::Pressed => {}
        }

        match button.action {
            PauseAction::Resume => {
                pause.paused = false;
                let mut vis = overlay_q.single_mut();
                *vis = Visibility::Hidden;
                let mut window = windows.single_mut();
                window.cursor_options.grab_mode = CursorGrabMode::Locked;
                window.cursor_options.visible = false;
            }
            PauseAction::Save => {
                // TODO: implement save
                println!("Save not implemented yet");
            }
            PauseAction::Settings => {
                // Toggle master mute (0.0 ↔ 0.8). Full settings UI coming later.
                if audio_settings.master_volume > 0.0 {
                    audio_settings.master_volume = 0.0;
                    info!("Audio: master muted");
                } else {
                    audio_settings.master_volume = 0.8;
                    info!("Audio: master unmuted (0.8)");
                }
            }
            PauseAction::Quit => {
                exit.send(AppExit::Success);
            }
        }
    }
}

/// Run condition: returns true when game is NOT paused
pub fn game_not_paused(pause: Res<PauseState>) -> bool {
    !pause.paused
}
