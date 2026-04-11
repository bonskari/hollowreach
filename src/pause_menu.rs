//! Pause menu — Esc toggles a menu with Resume, Save, Settings, Quit.
//! While open: game is paused, cursor is visible.

use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions};
use crate::ui_helpers::{self, UiAssets};
use crate::DismissPanel;

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
            .add_systems(Startup, setup_pause_menu.after(ui_helpers::setup_ui_assets))
            .add_systems(Update, (
                toggle_pause
                    .after(crate::panel::panel_close_system),
                pause_button_system,
            ));
    }
}

fn setup_pause_menu(mut commands: Commands, ui: Res<UiAssets>) {
    commands
        .spawn((
            PauseOverlay,
            ui_helpers::AnimatedPanel,
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
            parent
                .spawn((
                    ui_helpers::panel_image_node(&ui),
                    Node {
                        flex_direction: FlexDirection::Column,
                        align_items: AlignItems::Center,
                        padding: UiRect::all(Val::Px(40.0)),
                        row_gap: Val::Px(12.0),
                        min_width: Val::Px(250.0),
                        ..default()
                    },
                ))
                .with_children(|panel| {
                    panel.spawn((
                        Text::new("Paused"),
                        TextFont { font_size: 32.0, ..default() },
                        TextColor(ui_helpers::COLOR_GOLD),
                        Node { margin: UiRect::bottom(Val::Px(16.0)), ..default() },
                    ));

                    for (label, action) in [
                        ("Resume", PauseAction::Resume),
                        ("Save", PauseAction::Save),
                        ("Settings", PauseAction::Settings),
                        ("Quit", PauseAction::Quit),
                    ] {
                        ui_helpers::spawn_button(panel, &ui, label, PauseButton { action });
                    }
                });
        });
}

fn toggle_pause(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut pause: ResMut<PauseState>,
    overlay_q: Query<Entity, With<PauseOverlay>>,
    mut cursor_q: Query<&mut CursorOptions>,
    panel_state: Res<crate::panel::PanelState>,
    text_input_state: Res<crate::text_input::TextInputState>,
    esc_consumed: Res<crate::EscapeConsumed>,
    mut commands: Commands,
) {
    if keyboard.just_pressed(KeyCode::Escape) {
        if panel_state.visual != crate::panel::PanelVisual::Hidden || text_input_state.active || esc_consumed.0 {
            return;
        }
        pause.paused = !pause.paused;

        let mut cursor_opts = cursor_q.single_mut().unwrap();

        if pause.paused {
            if let Ok(entity) = overlay_q.single() {
                commands.entity(entity).insert(Visibility::Visible);
            }
            cursor_opts.grab_mode = CursorGrabMode::None;
            cursor_opts.visible = true;
        } else {
            if let Ok(entity) = overlay_q.single() {
                commands.entity(entity).insert(DismissPanel);
            }
            cursor_opts.grab_mode = CursorGrabMode::Locked;
            cursor_opts.visible = false;
        }
    }
}

fn pause_button_system(
    mut interaction_q: Query<(&Interaction, &PauseButton), Changed<Interaction>>,
    mut pause: ResMut<PauseState>,
    overlay_q: Query<Entity, With<PauseOverlay>>,
    mut cursor_q: Query<&mut CursorOptions>,
    mut exit: MessageWriter<AppExit>,
    mut audio_settings: ResMut<crate::AudioSettings>,
    mut commands: Commands,
) {
    for (interaction, button) in &mut interaction_q {
        if *interaction != Interaction::Pressed {
            continue;
        }

        match button.action {
            PauseAction::Resume => {
                pause.paused = false;
                if let Ok(entity) = overlay_q.single() {
                    commands.entity(entity).insert(DismissPanel);
                }
                let mut cursor_opts = cursor_q.single_mut().unwrap();
                cursor_opts.grab_mode = CursorGrabMode::Locked;
                cursor_opts.visible = false;
            }
            PauseAction::Save => {
                println!("Save not implemented yet");
            }
            PauseAction::Settings => {
                if audio_settings.master_volume > 0.0 {
                    audio_settings.master_volume = 0.0;
                    info!("Audio: master muted");
                } else {
                    audio_settings.master_volume = 0.8;
                    info!("Audio: master unmuted (0.8)");
                }
            }
            PauseAction::Quit => {
                exit.write(AppExit::Success);
            }
        }
    }
}

/// Run condition: returns true when game is NOT paused
pub fn game_not_paused(pause: Res<PauseState>) -> bool {
    !pause.paused
}
