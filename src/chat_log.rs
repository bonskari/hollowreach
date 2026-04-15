//! Chat-style message log at the top of the screen.
//!
//! Replaces the Dialogue panel for NPC speech. Messages appear as multi-line
//! text without background. When a new message arrives, the previous one
//! moves up and drops to 50% opacity, then fades out after a delay.

use bevy::prelude::*;

// ---------------------------------------------------------------------------
// Timing constants
// ---------------------------------------------------------------------------

/// How long the newest message stays fully visible before starting to fade.
const FULL_VISIBILITY_SECS: f32 = 4.0;
/// Duration of the fade-out animation.
const FADE_DURATION_SECS: f32 = 1.5;
/// Vertical spacing between chat lines (px).
const LINE_SPACING: f32 = 8.0;
/// Maximum number of messages visible at once — older ones despawn immediately.
const MAX_MESSAGES: usize = 4;

/// Compute the target opacity for a message at a given slot index (0 = newest).
/// Slot 0 → 100%, slot 1 → 50%, slot 2 → 25%, slot 3 → 12.5%, etc.
fn opacity_for_slot(slot: u32) -> f32 {
    1.0 / (1u32 << slot) as f32
}

// ---------------------------------------------------------------------------
// Components
// ---------------------------------------------------------------------------

/// Root container for all chat messages (vertical column, top-center).
#[derive(Component)]
pub struct ChatLogRoot;

/// A single chat message entity.
#[derive(Component)]
pub struct ChatMessage {
    /// Time elapsed since this message was spawned.
    pub age: f32,
    /// Slot index — 0 is newest, increases each time a newer message arrives.
    pub slot: u32,
}

// ---------------------------------------------------------------------------
// Events
// ---------------------------------------------------------------------------

/// Push a new message into the chat log.
#[derive(Message, Debug, Clone)]
pub struct PushChatMessage {
    pub speaker: String,
    pub text: String,
}

// ---------------------------------------------------------------------------
// Plugin
// ---------------------------------------------------------------------------

pub struct ChatLogPlugin;

impl Plugin for ChatLogPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<PushChatMessage>()
            .add_systems(Startup, setup_chat_log)
            .add_systems(
                Update,
                (
                    spawn_chat_messages,
                    age_chat_messages,
                    despawn_expired_messages,
                ),
            );
    }
}

// ---------------------------------------------------------------------------
// Systems
// ---------------------------------------------------------------------------

fn setup_chat_log(mut commands: Commands) {
    // Chat log at the bottom of the screen. NPC activation panel sits above it.
    // Stack so newest appears closest to the bottom, older ones rise up.
    commands.spawn((
        ChatLogRoot,
        Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(24.0),
            left: Val::Px(0.0),
            right: Val::Px(0.0),
            flex_direction: FlexDirection::Column, // oldest on top, newest at bottom
            align_items: AlignItems::Center,
            row_gap: Val::Px(LINE_SPACING),
            ..default()
        },
        GlobalZIndex(200),
    ));
}

fn spawn_chat_messages(
    mut commands: Commands,
    mut events: MessageReader<PushChatMessage>,
    root_q: Query<Entity, With<ChatLogRoot>>,
    mut existing: Query<&mut ChatMessage>,
) {
    let Ok(root) = root_q.single() else { return };

    for event in events.read() {
        // Shift all existing messages up one slot (newest → 0, was-0 → 1, etc.)
        for mut msg in &mut existing {
            msg.slot += 1;
        }

        // Build the displayed text: speaker + message
        let display = if event.speaker.is_empty() {
            event.text.clone()
        } else {
            format!("{}: {}", event.speaker, event.text)
        };

        commands.entity(root).with_children(|parent| {
            parent.spawn((
                ChatMessage { age: 0.0, slot: 0 },
                Text::new(display),
                TextFont { font_size: 20.0, ..default() },
                TextColor(Color::srgba(1.0, 1.0, 1.0, 1.0)),
                TextLayout::new_with_justify(Justify::Center),
                Node {
                    max_width: Val::Percent(70.0),
                    ..default()
                },
            ));
        });
    }
}

fn age_chat_messages(
    time: Res<Time>,
    mut messages: Query<(&mut ChatMessage, &mut TextColor)>,
) {
    for (mut msg, mut color) in &mut messages {
        msg.age += time.delta_secs();

        let target_opacity = opacity_for_slot(msg.slot);

        // Only the newest (slot 0) waits before fading. Older messages keep
        // their slot-based opacity until the fade-out timer kicks in.
        let alpha = if msg.age < FULL_VISIBILITY_SECS {
            target_opacity
        } else {
            let fade_progress = (msg.age - FULL_VISIBILITY_SECS) / FADE_DURATION_SECS;
            target_opacity * (1.0 - fade_progress.clamp(0.0, 1.0))
        };

        let current = color.0.to_srgba();
        *color = TextColor(Color::srgba(current.red, current.green, current.blue, alpha));
    }
}

fn despawn_expired_messages(
    mut commands: Commands,
    messages: Query<(Entity, &ChatMessage)>,
) {
    // Despawn fully faded messages.
    for (entity, msg) in &messages {
        if msg.age > FULL_VISIBILITY_SECS + FADE_DURATION_SECS {
            commands.entity(entity).despawn();
        }
    }

    // Also enforce hard cap MAX_MESSAGES: despawn oldest beyond the limit.
    let mut entries: Vec<(Entity, f32)> = messages.iter().map(|(e, m)| (e, m.age)).collect();
    if entries.len() > MAX_MESSAGES {
        entries.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        for (entity, _) in entries.iter().skip(MAX_MESSAGES) {
            commands.entity(*entity).despawn();
        }
    }
}
