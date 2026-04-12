//! Single interaction panel — NPC menu, prop menu, text input, and dialogue
//! all share one panel entity driven by a state machine.
//!
//! Send `PanelCommand` events to open/close/transition the panel.
//! The panel spawns dynamic content based on the requested `PanelContent`.

use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions};
use crate::{interactions, npc_ai, text_input, ui_helpers};

const PANEL_MIN_WIDTH: f32 = 500.0;

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

/// What the panel is currently showing.
#[derive(Clone, Debug)]
pub enum PanelContent {
    /// Panel is closed / hidden.
    None,
    /// NPC interaction menu: Say, Give Item, Nevermind.
    NpcMenu {
        npc: Entity,
        name: String,
        role: String,
        greeting: String,
    },
    /// Prop interaction menu: dynamic buttons per available interaction.
    PropMenu {
        prop: Entity,
        name: String,
        subtitle: String,
        interactions: Vec<interactions::Interaction>,
    },
    /// Text input for "Say" to NPC.
    TextInput {
        target_npc: Entity,
    },
    /// Read-only dialogue / info text display (auto-dismisses after timer).
    Dialogue {
        speaker: String,
        text: String,
    },
}

/// Visual state of the panel animation.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub enum PanelVisual {
    /// Panel is fully closed and invisible.
    #[default]
    Hidden,
    /// Panel is visible (or opening — animation in progress).
    Open,
    /// Close animation in progress.
    Closing,
}

/// Single source of truth for the interaction panel.
#[derive(Resource)]
pub struct PanelState {
    /// What content is currently being shown.
    pub content: PanelContent,
    /// Animation/visual state.
    pub visual: PanelVisual,
    /// Queued content for close → reopen transitions.
    pub pending: Option<PanelContent>,
    /// Auto-dismiss timer for Dialogue content.
    pub dialogue_timer: Option<Timer>,
}

impl Default for PanelState {
    fn default() -> Self {
        Self {
            content: PanelContent::None,
            visual: PanelVisual::Hidden,
            pending: None,
            dialogue_timer: None,
        }
    }
}

/// Run condition: true when the panel is closed (no content showing).
pub fn panel_not_open(state: Res<PanelState>) -> bool {
    matches!(state.content, PanelContent::None)
}

/// Run condition: true when the current panel content should not block gameplay.
/// Read-only dialogue does not lock player movement/look/interact.
pub fn panel_not_blocking_gameplay(state: Res<PanelState>) -> bool {
    matches!(state.content, PanelContent::None | PanelContent::Dialogue { .. })
}

// ---------------------------------------------------------------------------
// Commands (events)
// ---------------------------------------------------------------------------

/// Event to open or close the panel.
#[derive(Message, Clone, Debug)]
pub struct PanelCommand {
    pub action: PanelAction,
}

#[derive(Clone, Debug)]
pub enum PanelAction {
    /// Open with new content. If already open, transitions via close → reopen.
    Open(PanelContent),
    /// Close the panel.
    Close,
}

// ---------------------------------------------------------------------------
// Markers
// ---------------------------------------------------------------------------

/// Outer wrapper of the single interaction panel.
#[derive(Component)]
pub struct InteractionPanel;

/// Inner 9-slice panel that holds dynamically spawned content.
#[derive(Component)]
pub struct PanelInner;

/// Button inside the panel.
#[derive(Component)]
pub struct PanelButton {
    pub action: PanelButtonAction,
}

/// Actions for panel buttons.
#[derive(Clone, Debug)]
pub enum PanelButtonAction {
    Say,
    GiveItem,
    Nevermind,
    PropInteraction { index: usize },
}

// ---------------------------------------------------------------------------
// Setup
// ---------------------------------------------------------------------------

/// Spawns the single interaction panel entity (outer wrapper + inner shell).
fn setup_panel(mut commands: Commands, ui: Res<ui_helpers::UiAssets>) {
    commands
        .spawn((
            InteractionPanel,
            ui_helpers::AnimatedPanel,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                right: Val::Px(0.0),
                bottom: Val::Px(120.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            Visibility::Hidden,
            GlobalZIndex(100),
        ))
        .with_children(|wrapper| {
            wrapper.spawn((
                PanelInner,
                ui_helpers::panel_image_node(&ui),
                Node {
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    padding: UiRect::axes(Val::Px(40.0), Val::Px(24.0)),
                    ..default()
                },
            ));
        });
}

fn content_root_node() -> Node {
    Node {
        flex_direction: FlexDirection::Column,
        align_items: AlignItems::Center,
        min_width: Val::Px(PANEL_MIN_WIDTH),
        ..default()
    }
}

// ---------------------------------------------------------------------------
// Content spawning
// ---------------------------------------------------------------------------

fn spawn_content(
    commands: &mut Commands,
    inner: Entity,
    content: &PanelContent,
    ui: &ui_helpers::UiAssets,
) {
    match content {
        PanelContent::None => {}
        PanelContent::NpcMenu { name, role, greeting, .. } => {
            commands.entity(inner).with_children(|panel| {
                panel.spawn(content_root_node()).with_children(|content| {
                    // Name + role row
                    content.spawn(Node {
                        flex_direction: FlexDirection::Row,
                        align_items: AlignItems::Baseline,
                        column_gap: Val::Px(12.0),
                        margin: UiRect::bottom(Val::Px(4.0)),
                        ..default()
                    }).with_children(|row| {
                        row.spawn((
                            Text::new(name.as_str()),
                            TextFont { font_size: 22.0, ..default() },
                            TextColor(ui_helpers::COLOR_GOLD),
                        ));
                        row.spawn((
                            Text::new(role.as_str()),
                            TextFont { font_size: 14.0, ..default() },
                            TextColor(ui_helpers::COLOR_GREY),
                        ));
                    });

                    // Greeting
                    content.spawn((
                        Text::new(greeting.as_str()),
                        TextFont { font_size: 16.0, ..default() },
                        TextColor(ui_helpers::COLOR_BODY),
                        Node { margin: UiRect::bottom(Val::Px(8.0)), ..default() },
                        TextLayout::new_with_justify(Justify::Center),
                    ));

                    // Divider
                    ui_helpers::spawn_divider(content, ui);

                    // Buttons
                    content.spawn(Node {
                        flex_direction: FlexDirection::Row,
                        justify_content: JustifyContent::Center,
                        column_gap: Val::Px(8.0),
                        ..default()
                    }).with_children(|row| {
                        ui_helpers::spawn_button(row, ui, "Say", PanelButton { action: PanelButtonAction::Say });
                        ui_helpers::spawn_button(row, ui, "Give Item", PanelButton { action: PanelButtonAction::GiveItem });
                        ui_helpers::spawn_button_muted(row, ui, "Nevermind", PanelButton { action: PanelButtonAction::Nevermind });
                    });
                });
            });
        }
        PanelContent::PropMenu { name, subtitle, interactions, .. } => {
            commands.entity(inner).with_children(|panel| {
                panel.spawn(content_root_node()).with_children(|content| {
                    // Name + subtitle row
                    content.spawn(Node {
                        flex_direction: FlexDirection::Row,
                        align_items: AlignItems::Baseline,
                        column_gap: Val::Px(12.0),
                        margin: UiRect::bottom(Val::Px(8.0)),
                        ..default()
                    }).with_children(|row| {
                        row.spawn((
                            Text::new(name.as_str()),
                            TextFont { font_size: 22.0, ..default() },
                            TextColor(ui_helpers::COLOR_GOLD),
                        ));
                        row.spawn((
                            Text::new(subtitle.as_str()),
                            TextFont { font_size: 14.0, ..default() },
                            TextColor(ui_helpers::COLOR_GREY),
                        ));
                    });

                    // Divider
                    ui_helpers::spawn_divider(content, ui);

                    // Buttons
                    content.spawn(Node {
                        flex_direction: FlexDirection::Row,
                        justify_content: JustifyContent::Center,
                        column_gap: Val::Px(8.0),
                        ..default()
                    }).with_children(|row| {
                        for (i, interaction) in interactions.iter().enumerate() {
                            ui_helpers::spawn_button(row, ui, &interaction.label, PanelButton {
                                action: PanelButtonAction::PropInteraction { index: i },
                            });
                        }
                        ui_helpers::spawn_button_muted(row, ui, "Nevermind", PanelButton { action: PanelButtonAction::Nevermind });
                    });
                });
            });
        }
        PanelContent::TextInput { .. } => {
            commands.entity(inner).with_children(|content| {
                content.spawn(content_root_node()).with_children(|content| {
                    // "Say to ..." label
                    content.spawn((
                        text_input::TextInputLabel,
                        Text::new("Say to ..."),
                        TextFont { font_size: 18.0, ..default() },
                        TextColor(ui_helpers::COLOR_GOLD),
                    ));

                    // Spacer
                    content.spawn(Node { height: Val::Px(8.0), ..default() });

                    // Input area
                    content.spawn((
                        Node {
                            flex_direction: FlexDirection::Row,
                            align_items: AlignItems::Center,
                            min_height: Val::Px(28.0),
                            padding: UiRect::axes(Val::Px(8.0), Val::Px(6.0)),
                            ..default()
                        },
                        BackgroundColor(Color::srgba(0.04, 0.03, 0.08, 0.7)),
                    )).with_children(|input_row| {
                        input_row.spawn((
                            Text::new("> "),
                            TextFont { font_size: 17.0, ..default() },
                            TextColor(Color::srgb(0.6, 0.55, 0.45)),
                        ));
                        input_row.spawn((
                            text_input::TextInputDisplay,
                            Text::new(""),
                            TextFont { font_size: 17.0, ..default() },
                            TextColor(Color::srgba(0.9, 0.9, 0.9, 1.0)),
                        ));
                        input_row.spawn((
                            text_input::TextInputCursor,
                            Text::new("|"),
                            TextFont { font_size: 17.0, ..default() },
                            TextColor(Color::srgba(0.95, 0.82, 0.4, 1.0)),
                        ));
                    });

                    // Spacer
                    content.spawn(Node { height: Val::Px(6.0), ..default() });

                    // Hint text
                    content.spawn((
                        Text::new("[Enter] Send    [Esc] Cancel"),
                        TextFont { font_size: 13.0, ..default() },
                        TextColor(Color::srgba(0.6, 0.58, 0.5, 0.7)),
                    ));
                });
            });
        }
        PanelContent::Dialogue { speaker, text } => {
            commands.entity(inner).with_children(|content| {
                content.spawn(content_root_node()).with_children(|content| {
                    content.spawn((
                        Text::new(speaker.as_str()),
                        TextFont { font_size: 22.0, ..default() },
                        TextColor(ui_helpers::COLOR_GOLD),
                    ));
                    ui_helpers::spawn_divider(content, ui);
                    content.spawn((
                        Text::new(text.as_str()),
                        TextFont { font_size: 17.0, ..default() },
                        TextColor(ui_helpers::COLOR_BODY),
                        TextLayout::new_with_justify(Justify::Left),
                    ));
                });
            });
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn needs_cursor(content: &PanelContent) -> bool {
    matches!(content, PanelContent::NpcMenu { .. } | PanelContent::PropMenu { .. } | PanelContent::TextInput { .. })
}

fn make_dialogue_timer(content: &PanelContent) -> Option<Timer> {
    if matches!(content, PanelContent::Dialogue { .. }) {
        Some(Timer::from_seconds(4.0, TimerMode::Once))
    } else {
        None
    }
}

fn activate_content(content: &PanelContent, text_input_state: &mut text_input::TextInputState) {
    if let PanelContent::TextInput { target_npc } = content {
        text_input::activate_text_input(text_input_state, *target_npc);
    }
}

fn deactivate_content(content: &PanelContent, text_input_state: &mut text_input::TextInputState) {
    if matches!(content, PanelContent::TextInput { .. }) {
        text_input::deactivate_text_input(text_input_state);
    }
}

// ---------------------------------------------------------------------------
// Systems
// ---------------------------------------------------------------------------

/// Processes PanelCommand events — opens, closes, or queues transitions.
pub fn panel_command_system(
    mut commands: Commands,
    mut panel_state: ResMut<PanelState>,
    mut events: MessageReader<PanelCommand>,
    mut panel_q: Query<(Entity, &mut Visibility), With<InteractionPanel>>,
    inner_q: Query<Entity, With<PanelInner>>,
    ui: Res<ui_helpers::UiAssets>,
    mut cursor_q: Query<&mut CursorOptions>,
    mut text_input_state: ResMut<text_input::TextInputState>,
) {
    for cmd in events.read() {
        let Ok((panel_entity, mut panel_vis)) = panel_q.single_mut() else { continue };
        let Ok(inner_entity) = inner_q.single() else { continue };

        match &cmd.action {
            PanelAction::Open(content) => {
                match panel_state.visual {
                    PanelVisual::Hidden => {
                        // Open directly
                        activate_content(content, &mut text_input_state);
                        panel_state.dialogue_timer = make_dialogue_timer(content);
                        spawn_content(&mut commands, inner_entity, content, &ui);
                        *panel_vis = Visibility::Visible;

                        if needs_cursor(content) {
                            if let Ok(mut c) = cursor_q.single_mut() {
                                c.grab_mode = CursorGrabMode::None;
                                c.visible = true;
                            }
                        }

                        panel_state.content = content.clone();
                        panel_state.visual = PanelVisual::Open;
                    }
                    PanelVisual::Open => {
                        // Queue transition: close current, then open new
                        panel_state.pending = Some(content.clone());
                        commands.entity(panel_entity).insert(crate::DismissPanel);
                        panel_state.visual = PanelVisual::Closing;
                    }
                    PanelVisual::Closing => {
                        // Replace pending content
                        panel_state.pending = Some(content.clone());
                    }
                }
            }
            PanelAction::Close => {
                match panel_state.visual {
                    PanelVisual::Hidden => {} // already closed
                    PanelVisual::Open => {
                        panel_state.pending = None;

                        // Lock cursor immediately
                        if needs_cursor(&panel_state.content) {
                            if let Ok(mut c) = cursor_q.single_mut() {
                                c.grab_mode = CursorGrabMode::Locked;
                                c.visible = false;
                            }
                        }

                        commands.entity(panel_entity).insert(crate::DismissPanel);
                        panel_state.visual = PanelVisual::Closing;
                    }
                    PanelVisual::Closing => {
                        // Clear pending (ensure no reopen)
                        panel_state.pending = None;
                    }
                }
            }
        }
    }
}

/// Detects when the close animation finishes and processes pending transitions.
pub fn panel_transition_system(
    mut panel_state: ResMut<PanelState>,
    mut panel_q: Query<(Entity, &mut Visibility), With<InteractionPanel>>,
    inner_q: Query<Entity, With<PanelInner>>,
    mut commands: Commands,
    ui: Res<ui_helpers::UiAssets>,
    mut cursor_q: Query<&mut CursorOptions>,
    mut text_input_state: ResMut<text_input::TextInputState>,
) {
    if panel_state.visual != PanelVisual::Closing { return; }

    let Ok((_panel_entity, mut panel_vis)) = panel_q.single_mut() else { return };
    let Ok(inner_entity) = inner_q.single() else { return };

    // Wait until ui_scale_out_system has hidden the wrapper
    if *panel_vis != Visibility::Hidden { return; }

    // Deactivate old content
    deactivate_content(&panel_state.content, &mut text_input_state);

    // Despawn old content children
    commands.entity(inner_entity).despawn_children();

    if let Some(pending) = panel_state.pending.take() {
        // Transition to new content
        activate_content(&pending, &mut text_input_state);
        panel_state.dialogue_timer = make_dialogue_timer(&pending);
        spawn_content(&mut commands, inner_entity, &pending, &ui);

        // Show panel again (panel_appear_animation_system will add UiScaleIn)
        *panel_vis = Visibility::Visible;

        // Manage cursor for new content
        if needs_cursor(&pending) {
            if let Ok(mut c) = cursor_q.single_mut() {
                c.grab_mode = CursorGrabMode::None;
                c.visible = true;
            }
        } else {
            if let Ok(mut c) = cursor_q.single_mut() {
                c.grab_mode = CursorGrabMode::Locked;
                c.visible = false;
            }
        }

        panel_state.content = pending;
        panel_state.visual = PanelVisual::Open;
    } else {
        // Fully closed
        panel_state.content = PanelContent::None;
        panel_state.visual = PanelVisual::Hidden;
        panel_state.dialogue_timer = None;
    }
}

/// Handles button clicks inside the panel.
fn panel_button_system(
    mut interaction_q: Query<
        (&bevy::ui::Interaction, &PanelButton),
        Changed<bevy::ui::Interaction>,
    >,
    panel_state: Res<PanelState>,
    mut panel_commands: MessageWriter<PanelCommand>,
    mut commands: Commands,
    mut interaction_events: MessageWriter<interactions::InteractionEvent>,
    player_q: Query<Entity, With<crate::Player>>,
    entity_id_q: Query<Option<&crate::EntityId>>,
    mut cooldown: ResMut<crate::InteractionCooldown>,
) {
    for (interaction, button) in &mut interaction_q {
        if *interaction != bevy::ui::Interaction::Pressed {
            continue;
        }

        match &button.action {
            PanelButtonAction::Say => {
                if let PanelContent::NpcMenu { npc, .. } = &panel_state.content {
                    // Transition to text input (NPC stays frozen)
                    panel_commands.write(PanelCommand {
                        action: PanelAction::Open(PanelContent::TextInput { target_npc: *npc }),
                    });
                }
            }
            PanelButtonAction::GiveItem => {
                info!("Give item: not implemented yet");
                if let PanelContent::NpcMenu { npc, .. } = &panel_state.content {
                    commands.entity(*npc).remove::<npc_ai::NpcInteracting>();
                }
                panel_commands.write(PanelCommand { action: PanelAction::Close });
                cooldown.0.tick(std::time::Duration::from_secs(2));
            }
            PanelButtonAction::Nevermind => {
                // Unfreeze NPC if in NPC menu
                if let PanelContent::NpcMenu { npc, .. } = &panel_state.content {
                    commands.entity(*npc).remove::<npc_ai::NpcInteracting>();
                }
                panel_commands.write(PanelCommand { action: PanelAction::Close });
                cooldown.0.tick(std::time::Duration::from_secs(2));
            }
            PanelButtonAction::PropInteraction { index } => {
                if let PanelContent::PropMenu { prop, interactions, .. } = &panel_state.content {
                    if let Some(chosen) = interactions.get(*index) {
                        let effects = interactions::execute_reaction(&chosen.reaction);
                        let target_id = entity_id_q
                            .get(*prop)
                            .ok()
                            .flatten()
                            .map(|eid| eid.0.clone())
                            .unwrap_or_default();
                        let actor = player_q.single().unwrap();
                        interaction_events.write(interactions::InteractionEvent {
                            target: *prop,
                            target_id,
                            actor,
                            interaction_id: chosen.id.clone(),
                            effects,
                        });
                    }
                }
                panel_commands.write(PanelCommand { action: PanelAction::Close });
                cooldown.0.tick(std::time::Duration::from_secs(2));
            }
        }
    }
}

/// Closes the panel on Escape key (NPC menu, prop menu, dialogue).
/// Text input handles its own Escape in text_input_system.
pub fn panel_close_system(
    keyboard: Res<ButtonInput<KeyCode>>,
    panel_state: Res<PanelState>,
    mut panel_commands: MessageWriter<PanelCommand>,
    mut commands: Commands,
    mut esc_consumed: ResMut<crate::EscapeConsumed>,
    mut cooldown: ResMut<crate::InteractionCooldown>,
) {
    if !keyboard.just_pressed(KeyCode::Escape) { return; }

    match &panel_state.content {
        PanelContent::None => return,
        PanelContent::TextInput { .. } => return, // handled by text_input_system
        PanelContent::Dialogue { .. } => {
            esc_consumed.0 = true;
            panel_commands.write(PanelCommand { action: PanelAction::Close });
        }
        PanelContent::NpcMenu { npc, .. } => {
            esc_consumed.0 = true;
            commands.entity(*npc).remove::<npc_ai::NpcInteracting>();
            panel_commands.write(PanelCommand { action: PanelAction::Close });
            cooldown.0.tick(std::time::Duration::from_secs(2));
        }
        PanelContent::PropMenu { .. } => {
            esc_consumed.0 = true;
            panel_commands.write(PanelCommand { action: PanelAction::Close });
            cooldown.0.tick(std::time::Duration::from_secs(2));
        }
    }
}

/// Auto-dismisses dialogue after 4 seconds.
fn panel_dialogue_timer_system(
    time: Res<Time>,
    mut panel_state: ResMut<PanelState>,
    mut panel_commands: MessageWriter<PanelCommand>,
) {
    if let Some(ref mut timer) = panel_state.dialogue_timer {
        timer.tick(time.delta());
        if timer.is_finished() {
            panel_state.dialogue_timer = None;
            panel_commands.write(PanelCommand { action: PanelAction::Close });
        }
    }
}

// ---------------------------------------------------------------------------
// Plugin
// ---------------------------------------------------------------------------

pub struct PanelPlugin;

impl Plugin for PanelPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PanelState>()
            .add_message::<PanelCommand>()
            .add_systems(Startup, setup_panel.after(ui_helpers::setup_ui_assets))
            .add_systems(
                Update,
                (
                    panel_close_system,
                    panel_button_system,
                    panel_dialogue_timer_system,
                    panel_command_system,
                    panel_transition_system,
                ).run_if(in_state(crate::GameState::Playing)),
            );
    }
}
