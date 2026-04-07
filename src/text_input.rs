//! Text input UI for the "Say" interaction — player types freely to NPCs.
//!
//! When the player selects "Say" on an NPC, a text input box appears at the bottom
//! of the screen. The player types their message, presses Enter to submit (firing a
//! `SayEvent`), or Escape to cancel. While the input is active, player movement and
//! mouse look are disabled.

use bevy::input::keyboard::KeyboardInput;
use bevy::prelude::*;
use bevy::ui::widget::NodeImageMode;
use bevy::window::CursorGrabMode;

// ---------------------------------------------------------------------------
// Events
// ---------------------------------------------------------------------------

/// Fired when the player submits text to an NPC via the "Say" interaction.
/// Will be consumed by the LLM system in a later phase.
#[derive(Event)]
pub struct SayEvent {
    pub npc: Entity,
    pub text: String,
}

// ---------------------------------------------------------------------------
// Resources
// ---------------------------------------------------------------------------

/// Tracks the current state of the text input UI.
#[derive(Resource)]
pub struct TextInputState {
    /// Whether the text input box is currently visible and capturing input.
    pub active: bool,
    /// The text the player has typed so far.
    pub current_text: String,
    /// The NPC entity the player is talking to.
    pub target_npc: Option<Entity>,
}

impl Default for TextInputState {
    fn default() -> Self {
        Self {
            active: false,
            current_text: String::new(),
            target_npc: None,
        }
    }
}

/// Timer resource for cursor blinking (toggles every 0.5s).
#[derive(Resource)]
pub struct CursorBlinkTimer {
    pub timer: Timer,
    pub visible: bool,
}

impl Default for CursorBlinkTimer {
    fn default() -> Self {
        Self {
            timer: Timer::from_seconds(0.5, TimerMode::Repeating),
            visible: true,
        }
    }
}

// ---------------------------------------------------------------------------
// Components
// ---------------------------------------------------------------------------

/// Marker for the text input box container (dark panel at bottom).
#[derive(Component)]
pub struct TextInputBox;

/// Marker for the text display inside the input box.
#[derive(Component)]
pub struct TextInputDisplay;

/// Marker for the blinking cursor element.
#[derive(Component)]
pub struct TextInputCursor;

/// Marker for the "Say to <NPC>" label above the input.
#[derive(Component)]
pub struct TextInputLabel;

// ---------------------------------------------------------------------------
// Setup system
// ---------------------------------------------------------------------------

/// Creates the text input UI — a dark panel at the bottom of the screen,
/// styled to match the existing dialogue box (Kenney Fantasy UI borders).
/// Hidden by default; shown when the player selects "Say" on an NPC.
pub fn setup_text_input_ui(mut commands: Commands, asset_server: Res<AssetServer>) {
    let panel_image: Handle<Image> = asset_server.load_with_settings("ui/Panel/panel-012.png", |s: &mut bevy::image::ImageLoaderSettings| {
        s.sampler = bevy::image::ImageSampler::nearest();
    });
    let button_image: Handle<Image> = asset_server.load_with_settings("ui/Panel/panel-012.png", |s: &mut bevy::image::ImageLoaderSettings| {
        s.sampler = bevy::image::ImageSampler::nearest();
    });
    let slicer = TextureSlicer {
        border: BorderRect::square(18.0),
        center_scale_mode: SliceScaleMode::Stretch,
        sides_scale_mode: SliceScaleMode::Tile { stretch_value: 3.0 },
        max_corner_scale: 2.0,
    };

    // Root container — anchored to bottom center, hidden by default
    commands
        .spawn((
            TextInputBox,
            Node {
                position_type: PositionType::Absolute,
                bottom: Val::Px(30.0),
                left: Val::Percent(15.0),
                right: Val::Percent(15.0),
                flex_direction: FlexDirection::Column,
                ..default()
            },
            
            Visibility::Hidden,
            // Render on top of dialogue box (dialogue=20, text input=50, pause=200)
            GlobalZIndex(50),
        ))
        .with_children(|box_parent| {
            // 9-slice border overlay (matching dialogue box style)
            box_parent.spawn((
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

            // Content area with padding
            box_parent
                .spawn(Node {
                    padding: UiRect::axes(Val::Px(24.0), Val::Px(16.0)),
                    flex_direction: FlexDirection::Column,
                    ..default()
                })
                .with_children(|content| {
                    // "Say to <NPC name>" label
                    content.spawn((
                        TextInputLabel,
                        Text::new("Say to ..."),
                        TextFont {
                            font_size: 18.0,
                            ..default()
                        },
                        TextColor(Color::srgb(0.95, 0.82, 0.4)),
                    ));

                    // Spacer
                    content.spawn(Node {
                        height: Val::Px(8.0),
                        ..default()
                    });

                    // Input area — row with typed text + blinking cursor
                    content
                        .spawn((Node {
                            flex_direction: FlexDirection::Row,
                            align_items: AlignItems::Center,
                            min_height: Val::Px(28.0),
                            padding: UiRect::axes(Val::Px(8.0), Val::Px(6.0)),
                            ..default()
                        },
                        BackgroundColor(Color::srgba(0.04, 0.03, 0.08, 0.7)),
                        ))
                        .with_children(|input_row| {
                            // "> " prompt
                            input_row.spawn((
                                Text::new("> "),
                                TextFont {
                                    font_size: 17.0,
                                    ..default()
                                },
                                TextColor(Color::srgb(0.6, 0.55, 0.45)),
                            ));

                            // Typed text
                            input_row.spawn((
                                TextInputDisplay,
                                Text::new(""),
                                TextFont {
                                    font_size: 17.0,
                                    ..default()
                                },
                                TextColor(Color::srgba(0.9, 0.9, 0.9, 1.0)),
                            ));

                            // Blinking cursor (a thin "|")
                            input_row.spawn((
                                TextInputCursor,
                                Text::new("|"),
                                TextFont {
                                    font_size: 17.0,
                                    ..default()
                                },
                                TextColor(Color::srgba(0.95, 0.82, 0.4, 1.0)),
                            ));
                        });

                    // Spacer
                    content.spawn(Node {
                        height: Val::Px(6.0),
                        ..default()
                    });

                    // Hint text: "[Enter] Send  [Esc] Cancel"
                    content.spawn((
                        Text::new("[Enter] Send    [Esc] Cancel"),
                        TextFont {
                            font_size: 13.0,
                            ..default()
                        },
                        TextColor(Color::srgba(0.6, 0.58, 0.5, 0.7)),
                    ));
                });
        });
}

// ---------------------------------------------------------------------------
// Activation helpers
// ---------------------------------------------------------------------------

/// Opens the text input box for a specific NPC.
/// Call this from the interaction system when the player selects "Say".
pub fn activate_text_input(state: &mut TextInputState, npc_entity: Entity) {
    state.active = true;
    state.current_text.clear();
    state.target_npc = Some(npc_entity);
}

/// Closes the text input box and clears state.
pub fn deactivate_text_input(state: &mut TextInputState) {
    state.active = false;
    state.current_text.clear();
    state.target_npc = None;
}

// ---------------------------------------------------------------------------
// Systems
// ---------------------------------------------------------------------------

/// Captures keyboard input when the text input is active.
/// Handles character input, backspace, enter (submit), and escape (cancel).
pub fn text_input_system(
    mut state: ResMut<TextInputState>,
    mut say_events: EventWriter<SayEvent>,
    mut keyboard_events: EventReader<KeyboardInput>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut display_q: Query<&mut Text, With<TextInputDisplay>>,
    mut box_q: Query<&mut Visibility, With<TextInputBox>>,
    mut label_q: Query<&mut Text, (With<TextInputLabel>, Without<TextInputDisplay>)>,
    mut windows: Query<&mut Window>,
    mut blink_timer: ResMut<CursorBlinkTimer>,
) {
    // Show/hide the input box based on state
    if let Ok(mut box_vis) = box_q.get_single_mut() {
        *box_vis = if state.active {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }

    if !state.active {
        return;
    }

    // Unlock cursor while typing
    if let Ok(mut window) = windows.get_single_mut() {
        window.cursor_options.grab_mode = CursorGrabMode::None;
        window.cursor_options.visible = true;
    }

    // Process keyboard events
    for event in keyboard_events.read() {
        // Only process key-down events
        if !event.state.is_pressed() {
            continue;
        }

        match event.key_code {
            KeyCode::Enter | KeyCode::NumpadEnter => {
                let text = state.current_text.trim().to_string();
                if !text.is_empty() {
                    if let Some(npc) = state.target_npc {
                        say_events.send(SayEvent {
                            npc,
                            text,
                        });
                    }
                }
                deactivate_text_input(&mut state);

                // Re-lock cursor for gameplay
                if let Ok(mut window) = windows.get_single_mut() {
                    window.cursor_options.grab_mode = CursorGrabMode::Locked;
                    window.cursor_options.visible = false;
                }

                // Reset blink timer
                blink_timer.timer.reset();
                blink_timer.visible = true;
                return;
            }
            KeyCode::Escape => {
                deactivate_text_input(&mut state);

                // Re-lock cursor for gameplay
                if let Ok(mut window) = windows.get_single_mut() {
                    window.cursor_options.grab_mode = CursorGrabMode::Locked;
                    window.cursor_options.visible = false;
                }

                // Reset blink timer
                blink_timer.timer.reset();
                blink_timer.visible = true;
                return;
            }
            KeyCode::Backspace => {
                state.current_text.pop();
            }
            _ => {
                // Map key codes to characters, respecting shift
                if let Some(ch) = key_to_char(event.key_code, keyboard.pressed(KeyCode::ShiftLeft) || keyboard.pressed(KeyCode::ShiftRight)) {
                    // Limit input length to prevent overflow
                    if state.current_text.len() < 256 {
                        state.current_text.push(ch);
                    }
                }
            }
        }
    }

    // Update display text
    if let Ok(mut display) = display_q.get_single_mut() {
        **display = state.current_text.clone();
    }

    // Update label (could set NPC name here if available)
    let _ = label_q.get_single_mut();
}

/// Blinks the text input cursor every 0.5 seconds.
pub fn cursor_blink_system(
    time: Res<Time>,
    state: Res<TextInputState>,
    mut blink_timer: ResMut<CursorBlinkTimer>,
    mut cursor_q: Query<&mut TextColor, With<TextInputCursor>>,
) {
    if !state.active {
        return;
    }

    blink_timer.timer.tick(time.delta());

    if blink_timer.timer.just_finished() {
        blink_timer.visible = !blink_timer.visible;
    }

    if let Ok(mut color) = cursor_q.get_single_mut() {
        let alpha = if blink_timer.visible { 1.0 } else { 0.0 };
        *color = TextColor(Color::srgba(0.95, 0.82, 0.4, alpha));
    }
}

/// Guards player movement — returns early if text input is active.
/// Add this as a run condition: `.run_if(text_input_not_active)`
pub fn text_input_not_active(state: Res<TextInputState>) -> bool {
    !state.active
}

// ---------------------------------------------------------------------------
// Key-to-character mapping
// ---------------------------------------------------------------------------

/// Maps a Bevy `KeyCode` to a character, respecting shift state.
/// Returns `None` for non-printable keys.
fn key_to_char(key: KeyCode, shift: bool) -> Option<char> {
    match key {
        // Letters
        KeyCode::KeyA => Some(if shift { 'A' } else { 'a' }),
        KeyCode::KeyB => Some(if shift { 'B' } else { 'b' }),
        KeyCode::KeyC => Some(if shift { 'C' } else { 'c' }),
        KeyCode::KeyD => Some(if shift { 'D' } else { 'd' }),
        KeyCode::KeyE => Some(if shift { 'E' } else { 'e' }),
        KeyCode::KeyF => Some(if shift { 'F' } else { 'f' }),
        KeyCode::KeyG => Some(if shift { 'G' } else { 'g' }),
        KeyCode::KeyH => Some(if shift { 'H' } else { 'h' }),
        KeyCode::KeyI => Some(if shift { 'I' } else { 'i' }),
        KeyCode::KeyJ => Some(if shift { 'J' } else { 'j' }),
        KeyCode::KeyK => Some(if shift { 'K' } else { 'k' }),
        KeyCode::KeyL => Some(if shift { 'L' } else { 'l' }),
        KeyCode::KeyM => Some(if shift { 'M' } else { 'm' }),
        KeyCode::KeyN => Some(if shift { 'N' } else { 'n' }),
        KeyCode::KeyO => Some(if shift { 'O' } else { 'o' }),
        KeyCode::KeyP => Some(if shift { 'P' } else { 'p' }),
        KeyCode::KeyQ => Some(if shift { 'Q' } else { 'q' }),
        KeyCode::KeyR => Some(if shift { 'R' } else { 'r' }),
        KeyCode::KeyS => Some(if shift { 'S' } else { 's' }),
        KeyCode::KeyT => Some(if shift { 'T' } else { 't' }),
        KeyCode::KeyU => Some(if shift { 'U' } else { 'u' }),
        KeyCode::KeyV => Some(if shift { 'V' } else { 'v' }),
        KeyCode::KeyW => Some(if shift { 'W' } else { 'w' }),
        KeyCode::KeyX => Some(if shift { 'X' } else { 'x' }),
        KeyCode::KeyY => Some(if shift { 'Y' } else { 'y' }),
        KeyCode::KeyZ => Some(if shift { 'Z' } else { 'z' }),

        // Numbers (top row)
        KeyCode::Digit0 => Some(if shift { ')' } else { '0' }),
        KeyCode::Digit1 => Some(if shift { '!' } else { '1' }),
        KeyCode::Digit2 => Some(if shift { '@' } else { '2' }),
        KeyCode::Digit3 => Some(if shift { '#' } else { '3' }),
        KeyCode::Digit4 => Some(if shift { '$' } else { '4' }),
        KeyCode::Digit5 => Some(if shift { '%' } else { '5' }),
        KeyCode::Digit6 => Some(if shift { '^' } else { '6' }),
        KeyCode::Digit7 => Some(if shift { '&' } else { '7' }),
        KeyCode::Digit8 => Some(if shift { '*' } else { '8' }),
        KeyCode::Digit9 => Some(if shift { '(' } else { '9' }),

        // Punctuation
        KeyCode::Space => Some(' '),
        KeyCode::Period => Some(if shift { '>' } else { '.' }),
        KeyCode::Comma => Some(if shift { '<' } else { ',' }),
        KeyCode::Semicolon => Some(if shift { ':' } else { ';' }),
        KeyCode::Quote => Some(if shift { '"' } else { '\'' }),
        KeyCode::Slash => Some(if shift { '?' } else { '/' }),
        KeyCode::Backslash => Some(if shift { '|' } else { '\\' }),
        KeyCode::BracketLeft => Some(if shift { '{' } else { '[' }),
        KeyCode::BracketRight => Some(if shift { '}' } else { ']' }),
        KeyCode::Minus => Some(if shift { '_' } else { '-' }),
        KeyCode::Equal => Some(if shift { '+' } else { '=' }),
        KeyCode::Backquote => Some(if shift { '~' } else { '`' }),

        // Numpad digits
        KeyCode::Numpad0 => Some('0'),
        KeyCode::Numpad1 => Some('1'),
        KeyCode::Numpad2 => Some('2'),
        KeyCode::Numpad3 => Some('3'),
        KeyCode::Numpad4 => Some('4'),
        KeyCode::Numpad5 => Some('5'),
        KeyCode::Numpad6 => Some('6'),
        KeyCode::Numpad7 => Some('7'),
        KeyCode::Numpad8 => Some('8'),
        KeyCode::Numpad9 => Some('9'),

        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Plugin
// ---------------------------------------------------------------------------

/// Plugin that adds the text input UI and systems to the app.
///
/// Usage in your app:
/// ```rust,ignore
/// app.add_plugins(TextInputPlugin);
/// ```
///
/// To disable player movement/look while typing, add run conditions to those
/// systems using `text_input_not_active`:
/// ```rust,ignore
/// app.add_systems(Update, (
///     player_movement.run_if(text_input_not_active),
///     player_look.run_if(text_input_not_active),
/// ));
/// ```
///
/// To open the text input for an NPC, mutate `TextInputState`:
/// ```rust,ignore
/// fn my_system(mut state: ResMut<TextInputState>) {
///     activate_text_input(&mut state, npc_entity);
/// }
/// ```
pub struct TextInputPlugin;

impl Plugin for TextInputPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TextInputState>()
            .init_resource::<CursorBlinkTimer>()
            .add_event::<SayEvent>()
            .add_systems(Startup, setup_text_input_ui)
            .add_systems(
                Update,
                (
                    text_input_system,
                    cursor_blink_system,
                ),
            );
    }
}
