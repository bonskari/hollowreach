//! Multi-NPC dialogue validation.
//! Fires SayEvents directly at multiple NPCs and checks each response.

use bevy::prelude::*;
use hollowreach::*;
use hollowreach::chat_log::ChatMessage;
use hollowreach::text_input::SayEvent;

#[derive(Resource)]
struct Frame(usize);

#[derive(Resource, Default)]
struct TestState {
    sent: Vec<(String, String)>,       // (npc_name, question)
    received: Vec<(String, String)>,   // (npc_name, response)
    phase: usize,
}

const QUESTIONS: &[(&str, &str)] = &[
    ("Sir Roland", "Hello"),
    ("Sir Roland", "What is your name?"),
    ("Sir Roland", "Do you have the key?"),
    ("Grok", "Hello"),
    ("Grok", "Who are you?"),
    ("Grok", "Are you hungry?"),
    ("Whisper", "Hello"),
    ("Whisper", "Who has the key?"),
    ("Elara the Wise", "What do you study?"),
];

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

    // Send one question every 120 frames (give LLM time to respond)
    if state.phase < QUESTIONS.len() {
        let send_frame = 10 + state.phase * 300;
        let check_frame = send_frame + 250;

        if frame.0 == send_frame {
            let (npc_name, question) = QUESTIONS[state.phase];
            if let Some((entity, _)) = npc_q.iter().find(|(_, p)| p.name == npc_name) {
                say_events.write(SayEvent { npc: entity, text: question.to_string() });
                state.sent.push((npc_name.to_string(), question.to_string()));
                println!("[SEND] {} <- '{}'", npc_name, question);
            } else {
                println!("[SKIP] NPC '{}' not found", npc_name);
                state.sent.push((npc_name.to_string(), question.to_string()));
            }
        }

        if frame.0 == check_frame {
            let (npc_name, _question) = QUESTIONS[state.phase];
            let prefix = format!("{}:", npc_name);
            let mut found = false;
            for text in &chat_msgs_q {
                let d = &text.0;
                if d.starts_with(&prefix)
                    && !state.received.iter().any(|(_, r)| r == d.as_str())
                {
                    println!("[RECV] {}", d);
                    state.received.push((npc_name.to_string(), d.clone()));
                    found = true;
                    break;
                }
            }
            if !found {
                println!("[MISS] No response from {}", npc_name);
                state.received.push((npc_name.to_string(), String::new()));
            }
            state.phase += 1;
        }
    }

    // Report after all questions
    let report_frame = 10 + QUESTIONS.len() * 300 + 30;
    if frame.0 == report_frame {
        println!("\n=== MULTI-NPC DIALOGUE RESULTS ===");
        let mut ok = 0;
        let mut bad = 0;
        for i in 0..state.sent.len() {
            let (npc, question) = &state.sent[i];
            let response = state.received.get(i).map(|(_, r)| r.as_str()).unwrap_or("");
            let is_ok = !response.is_empty()
                && !response.to_lowercase().contains("respond in character")
                && !response.to_lowercase().contains("never narrate")
                && !response.to_lowercase().contains("spoken words")
                && !response.to_lowercase().contains("spoken dialogue")
                && response.to_lowercase() != question.to_lowercase();
            if is_ok { ok += 1; } else { bad += 1; }
            println!("  {} [{}] Q:'{}' A:'{}'",
                npc, if is_ok { "OK" } else { "BAD" }, question, response);
        }
        println!("Score: {}/{}", ok, ok + bad);
        println!("==================================\n");

        if bad == 0 {
            println!("[SUCCESS] All NPC dialogues are in-character");
            exit.write(AppExit::Success);
        } else {
            println!("[FAILURE] {} bad responses", bad);
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
                        title: "Multi NPC Test".into(),
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
