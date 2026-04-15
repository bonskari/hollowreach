//! Comprehensive dialogue test — mirrors a real player session.
//! Tests ALL 5 NPCs with multiple questions each.
//! Validates: non-empty response, no prompt leaks, no narration, reasonable length.

use bevy::prelude::*;
use hollowreach::*;
use hollowreach::chat_log::ChatMessage;
use hollowreach::text_input::SayEvent;

#[derive(Resource)]
struct Frame(usize);

#[derive(Resource, Default)]
struct TestState {
    sent: Vec<(String, String)>,
    received: Vec<(String, String, bool)>, // (npc, response, ok)
    phase: usize,
}

const QUESTIONS: &[(&str, &str)] = &[
    // Grok
    ("Grok", "hi"),
    ("Grok", "What are you?"),
    ("Grok", "Are you hungry?"),
    // Sir Roland
    ("Sir Roland", "hello"),
    ("Sir Roland", "What is your name?"),
    ("Sir Roland", "Do you have a key?"),
    ("Sir Roland", "Can you give me the key?"),
    // Whisper
    ("Whisper", "hello"),
    ("Whisper", "Who has the key?"),
    // Elara the Wise
    ("Elara the Wise", "hello"),
    ("Elara the Wise", "What do you study?"),
    // Sylva
    ("Sylva", "hello"),
    ("Sylva", "What do you see in the forest?"),
];

fn validate_response(npc: &str, question: &str, response: &str) -> (bool, &'static str) {
    if response.is_empty() {
        return (false, "empty response");
    }
    let lower = response.to_lowercase();
    // Prompt leak
    if lower.contains("respond in character") || lower.contains("never narrate")
        || lower.contains("spoken words") || lower.contains("spoken dialogue")
        || lower.contains("reply in character") {
        return (false, "prompt leak");
    }
    // Narration
    if response.starts_with('(') {
        return (false, "narration in parentheses");
    }
    // Echo of player text
    if lower == question.to_lowercase() {
        return (false, "echoed player text");
    }
    // Too long (>200 chars for a single reply)
    if response.len() > 200 {
        return (false, "too long");
    }
    // Contains special tokens
    if response.contains("<start_of_turn>") || response.contains("<end_of_turn>")
        || response.contains("<eos>") || response.contains("</start_of_turn>") {
        return (false, "special tokens leaked");
    }
    (true, "ok")
}

fn test_system(
    mut frame: ResMut<Frame>,
    mut say_events: MessageWriter<SayEvent>,
    chat_msgs_q: Query<&Text, With<ChatMessage>>,
    npc_q: Query<(Entity, &NpcPersonality)>,
    mut state: ResMut<TestState>,
    mut exit: MessageWriter<AppExit>,
    game_state: Res<State<GameState>>,
    mut intro: ResMut<IntroSequence>,
) {
    if *game_state.get() != GameState::Playing { return; }
    intro.active = false;
    frame.0 += 1;

    // Send one question every 200 frames (~3.3s at 60fps).
    // Poll EVERY frame for the response (chat messages despawn after ~5s).
    if state.phase < QUESTIONS.len() {
        let send_frame = 10 + state.phase * 200;
        let deadline = send_frame + 180;
        let already_sent = state.sent.len() > state.phase;

        if frame.0 == send_frame && !already_sent {
            let (npc_name, question) = QUESTIONS[state.phase];
            if let Some((entity, _)) = npc_q.iter().find(|(_, p)| p.name == npc_name) {
                say_events.write(SayEvent { npc: entity, text: question.to_string() });
                state.sent.push((npc_name.to_string(), question.to_string()));
                println!("[SEND] {} <- '{}'", npc_name, question);
            } else {
                println!("[SKIP] NPC '{}' not found", npc_name);
                state.sent.push((npc_name.to_string(), question.to_string()));
                state.received.push((npc_name.to_string(), String::new(), false));
                state.phase += 1;
            }
        }

        // Poll every frame for response
        if already_sent && state.received.len() <= state.phase {
            let (npc_name, question) = QUESTIONS[state.phase];
            let prefix = format!("{}:", npc_name);
            let mut found = false;
            for text in &chat_msgs_q {
                let d = &text.0;
                if d.starts_with(&prefix)
                    && !state.received.iter().any(|(_, r, _)| r == d.as_str())
                {
                    let (ok, reason) = validate_response(npc_name, question, d);
                    println!("[RECV] {} [{}] '{}'", npc_name, reason, d);
                    state.received.push((npc_name.to_string(), d.clone(), ok));
                    found = true;
                    state.phase += 1;
                    break;
                }
            }
            // Deadline expired — count as miss
            if !found && frame.0 >= deadline {
                println!("[MISS] No response from {} for '{}'", npc_name, question);
                state.received.push((npc_name.to_string(), String::new(), false));
                state.phase += 1;
            }
        }
    }

    let report_frame = 10 + QUESTIONS.len() * 200 + 30;
    if frame.0 == report_frame {
        println!("\n=== COMPREHENSIVE DIALOGUE TEST ===");
        let mut ok = 0;
        let mut bad = 0;
        for i in 0..state.sent.len() {
            let (npc, question) = &state.sent[i];
            let (_, response, is_ok) = state.received.get(i)
                .cloned()
                .unwrap_or((String::new(), String::new(), false));
            if is_ok { ok += 1; } else { bad += 1; }
            println!("  {} [{}] Q:'{}' A:'{}'",
                npc, if is_ok { "OK" } else { "BAD" }, question, response);
        }
        println!("Score: {}/{}", ok, ok + bad);
        println!("====================================\n");

        if bad == 0 {
            println!("[SUCCESS] All NPC dialogues pass validation");
            exit.write(AppExit::Success);
        } else {
            println!("[FAILURE] {} responses failed validation", bad);
            exit.write(AppExit::from_code(1));
        }
    }

    if frame.0 > report_frame + 60 {
        exit.write(AppExit::from_code(1));
    }
}

fn main() {
    App::new()
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Comprehensive Dialogue Test".into(),
                        resolution: bevy::window::WindowResolution::new(1280, 720),
                        ..default()
                    }),
                    ..default()
                })
                .set(bevy::log::LogPlugin {
                    level: bevy::log::Level::INFO,
                    filter: "wgpu=warn,naga=warn,bevy_render=warn,bevy_ecs=warn,bevy_app=warn,bevy_winit=warn,bevy_asset=warn,gilrs=warn".into(),
                    ..default()
                }),
        )
        .add_plugins(HollowreachPlugin)
        .insert_resource(Frame(0))
        .insert_resource(TestState::default())
        .add_systems(Update, test_system)
        .run();
}
