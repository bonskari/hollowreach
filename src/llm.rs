//! Local LLM integration via llama-cpp-2 (Gemma GGUF).
//!
//! Loads a quantized Gemma model in a background thread.  Communication happens
//! via typed mpsc channels — one pair for dialogue (player ↔ NPC) and one for
//! autonomous NPC decisions.

use bevy::prelude::*;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};

// ---------------------------------------------------------------------------
// Public request / response types
// ---------------------------------------------------------------------------

/// Personality data sent with every LLM request so the prompt can be built.
#[derive(Debug, Clone)]
pub struct NpcContext {
    pub name: String,
    pub role: String,
    pub traits: Vec<String>,
    pub backstory: String,
    pub speech_style: String,
    pub knowledge: Vec<String>,
    pub goals: Vec<String>,
    pub likes: Vec<String>,
    pub dislikes: Vec<String>,
    pub inventory: Vec<String>,
    pub memories: String,
}

/// A single exchange in the conversation history.
#[derive(Debug, Clone)]
pub struct ChatEntry {
    /// "user" or "assistant"
    pub role: String,
    pub text: String,
}

/// Request the LLM to generate an NPC response to something the player said.
#[derive(Debug)]
pub struct DialogueRequest {
    pub npc: NpcContext,
    pub player_text: String,
    pub history: Vec<ChatEntry>,
    pub npc_entity: Entity,
}

/// The generated dialogue response.
#[derive(Debug, Clone)]
pub struct DialogueResponse {
    pub text: String,
    pub npc_entity: Entity,
}

/// Describes what an NPC can see around it so the LLM can decide.
#[derive(Debug, Clone)]
pub struct SurroundingsInfo {
    pub nearby_entities: Vec<NearbyEntity>,
    pub player_distance: Option<f32>,
}

#[derive(Debug, Clone)]
pub struct NearbyEntity {
    pub name: String,
    pub entity_type: String,
    pub distance: f32,
    pub entity: Entity,
}

/// Request the LLM to choose an action for an NPC.
#[derive(Debug)]
pub struct DecisionRequest {
    pub npc: NpcContext,
    pub surroundings: SurroundingsInfo,
    pub npc_entity: Entity,
}

/// The LLM's chosen action for an NPC.
#[derive(Debug, Clone)]
pub struct DecisionResponse {
    pub action: LlmAction,
    pub npc_entity: Entity,
}

#[derive(Debug, Clone)]
pub enum LlmAction {
    Idle,
    Speak(String),
    SpeakTo { target_name: String, text: String },
    MoveTo { target_name: String },
    Give { target_name: String, item_id: String, text: String },
}

fn humanize_item(item_id: &str) -> String {
    item_id.replace('_', " ")
}

/// Loading progress, forwarded to the loading screen.
#[derive(Debug, Clone)]
pub struct LlmLoadingStatus {
    pub message: String,
    pub progress: f32,
}

// ---------------------------------------------------------------------------
// LlmEngine resource
// ---------------------------------------------------------------------------

#[derive(Resource)]
pub struct LlmEngine {
    dialogue_tx: mpsc::Sender<DialogueRequest>,
    dialogue_rx: Mutex<mpsc::Receiver<DialogueResponse>>,
    decision_tx: mpsc::Sender<DecisionRequest>,
    decision_rx: Mutex<mpsc::Receiver<DecisionResponse>>,
    loading_rx: Mutex<mpsc::Receiver<LlmLoadingStatus>>,
    pub ready: Arc<AtomicBool>,
}

impl LlmEngine {
    pub fn new() -> Self {
        let (dialogue_req_tx, dialogue_req_rx) = mpsc::channel::<DialogueRequest>();
        let (dialogue_resp_tx, dialogue_resp_rx) = mpsc::channel::<DialogueResponse>();
        let (decision_req_tx, decision_req_rx) = mpsc::channel::<DecisionRequest>();
        let (decision_resp_tx, decision_resp_rx) = mpsc::channel::<DecisionResponse>();
        let (loading_tx, loading_rx) = mpsc::channel::<LlmLoadingStatus>();
        let ready = Arc::new(AtomicBool::new(false));
        let ready_clone = ready.clone();

        std::thread::spawn(move || {
            worker_thread(
                dialogue_req_rx,
                dialogue_resp_tx,
                decision_req_rx,
                decision_resp_tx,
                loading_tx,
                ready_clone,
            );
        });

        Self {
            dialogue_tx: dialogue_req_tx,
            dialogue_rx: Mutex::new(dialogue_resp_rx),
            decision_tx: decision_req_tx,
            decision_rx: Mutex::new(decision_resp_rx),
            loading_rx: Mutex::new(loading_rx),
            ready,
        }
    }

    pub fn request_dialogue(&self, req: DialogueRequest) {
        if let Err(e) = self.dialogue_tx.send(req) {
            warn!("LLM: dialogue request send failed: {}", e);
        }
    }

    pub fn request_decision(&self, req: DecisionRequest) {
        if let Err(e) = self.decision_tx.send(req) {
            warn!("LLM: decision request send failed: {}", e);
        }
    }

    pub fn poll_dialogue(&self) -> Option<DialogueResponse> {
        self.dialogue_rx.lock().ok()?.try_recv().ok()
    }

    pub fn poll_decision(&self) -> Option<DecisionResponse> {
        self.decision_rx.lock().ok()?.try_recv().ok()
    }

    pub fn poll_loading(&self) -> Option<LlmLoadingStatus> {
        self.loading_rx.lock().ok()?.try_recv().ok()
    }
}

// ---------------------------------------------------------------------------
// Prompt building
// ---------------------------------------------------------------------------

fn build_system_prompt(npc: &NpcContext) -> String {
    // Rich personality context helps the 2B model stay in character.
    // NO meta-instructions ("respond in character", "only spoken words", etc.)
    // — those get echoed. The personality itself guides the model.
    let mut lines = Vec::new();
    lines.push(format!("{}, {} in Hollowreach.", npc.name, npc.role));
    if !npc.speech_style.is_empty() {
        lines.push(npc.speech_style.clone());
    }
    if !npc.traits.is_empty() {
        lines.push(format!("Traits: {}", npc.traits.join(", ")));
    }
    if !npc.backstory.is_empty() {
        lines.push(npc.backstory.clone());
    }
    if !npc.knowledge.is_empty() {
        lines.push(format!("Knows: {}", npc.knowledge.join(". ")));
    }
    if !npc.inventory.is_empty() {
        let items: Vec<String> = npc.inventory.iter().map(|i| humanize_item(i)).collect();
        lines.push(format!("Carrying: {}", items.join(", ")));
    }
    if !npc.memories.is_empty() {
        lines.push(npc.memories.clone());
    }
    lines.join("\n")
}

fn build_decision_prompt(npc: &NpcContext, surroundings: &SurroundingsInfo) -> String {
    let mut lines = Vec::new();

    // Character context — NOT to be spoken aloud
    lines.push("[CHARACTER]".to_string());
    lines.push(format!("{}, {}.", npc.name, npc.role));
    lines.push(format!("Traits: {}", npc.traits.join(", ")));
    lines.push(format!("Speech style: {}", npc.speech_style));
    if !npc.goals.is_empty() {
        lines.push(format!("Goals: {}", npc.goals.join("; ")));
    }
    if !npc.inventory.is_empty() {
        let items: Vec<String> = npc.inventory.iter().map(|i| humanize_item(i)).collect();
        lines.push(format!("Carrying: {}", items.join(", ")));
    }

    if !npc.memories.is_empty() {
        lines.push(String::new());
        lines.push(npc.memories.clone());
    }

    // Surroundings context — NOT to be spoken aloud
    lines.push(String::new());
    lines.push("[SURROUNDINGS]".to_string());

    if !surroundings.nearby_entities.is_empty() {
        for e in &surroundings.nearby_entities {
            lines.push(format!("- {} ({})", e.name, e.entity_type));
        }
    }

    if let Some(d) = surroundings.player_distance {
        if d < 8.0 {
            lines.push("- A young wanderer is nearby.".to_string());
        }
    }

    // Instruction
    lines.push(String::new());
    lines.push("[DECIDE]".to_string());
    lines.push("Pick ONE action. The text field must be dialogue only — never narrate or describe.".to_string());
    lines.push(r#"{"action": "idle"}"#.to_string());
    lines.push(r#"{"action": "speak", "text": "DIALOGUE"}"#.to_string());
    lines.push(r#"{"action": "move_to", "target": "NAME"}"#.to_string());
    lines.push(r#"{"action": "speak_to", "target": "NAME", "text": "DIALOGUE"}"#.to_string());
    if !npc.inventory.is_empty() {
        lines.push(r#"{"action": "give", "target": "NAME", "item": "ITEM_ID", "text": "DIALOGUE"}"#.to_string());
    }

    lines.join("\n")
}

// ---------------------------------------------------------------------------
// Worker thread — owns the model and runs inference
// ---------------------------------------------------------------------------

fn find_model_path() -> Option<String> {
    let candidates = [
        "assets/models/gemma-4-E2B-it-Q4_K_M.gguf",
        "assets/models/gemma-4-4b-it-Q4_K_M.gguf",
    ];
    for p in &candidates {
        if std::path::Path::new(p).exists() {
            return Some(p.to_string());
        }
    }
    // Try relative to executable
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            for p in &candidates {
                let full = dir.join(p);
                if full.exists() {
                    return Some(full.to_string_lossy().to_string());
                }
            }
        }
    }
    None
}

fn worker_thread(
    dialogue_rx: mpsc::Receiver<DialogueRequest>,
    dialogue_tx: mpsc::Sender<DialogueResponse>,
    decision_rx: mpsc::Receiver<DecisionRequest>,
    decision_tx: mpsc::Sender<DecisionResponse>,
    loading_tx: mpsc::Sender<LlmLoadingStatus>,
    ready_flag: Arc<AtomicBool>,
) {
    use llama_cpp_2::llama_backend::LlamaBackend;
    use llama_cpp_2::model::params::LlamaModelParams;
    use llama_cpp_2::model::LlamaModel;

    let _ = loading_tx.send(LlmLoadingStatus {
        message: "Starting LLM engine...".into(),
        progress: 0.0,
    });

    let Some(model_path) = find_model_path() else {
        error!("LLM: could not find GGUF model file");
        let _ = loading_tx.send(LlmLoadingStatus {
            message: "LLM model not found".into(),
            progress: 1.0,
        });
        // Still mark as ready so the game doesn't hang — LLM just won't work.
        ready_flag.store(true, Ordering::SeqCst);
        return;
    };

    let _ = loading_tx.send(LlmLoadingStatus {
        message: "Loading language model...".into(),
        progress: 0.2,
    });

    let backend = match LlamaBackend::init() {
        Ok(b) => b,
        Err(e) => {
            error!("LLM: failed to init backend: {e:?}");
            let _ = loading_tx.send(LlmLoadingStatus {
                message: "LLM backend failed".into(),
                progress: 1.0,
            });
            ready_flag.store(true, Ordering::SeqCst);
            return;
        }
    };

    let model_params = LlamaModelParams::default().with_n_gpu_layers(999);
    let model = match LlamaModel::load_from_file(&backend, &model_path, &model_params) {
        Ok(m) => {
            info!("LLM: model loaded on GPU");
            m
        }
        Err(e) => {
            error!("LLM: GPU memory insufficient — 8 GB VRAM required. Error: {e:?}");
            let _ = loading_tx.send(LlmLoadingStatus {
                message: "Not enough GPU memory. 8 GB VRAM required.".into(),
                progress: 1.0,
            });
            // Do not mark ready — game stays on loading screen
            return;
        }
    };

    let _ = loading_tx.send(LlmLoadingStatus {
        message: "Language model loaded".into(),
        progress: 0.9,
    });

    // Get the chat template from the model
    let chat_template = match model.chat_template(None) {
        Ok(t) => {
            info!("LLM: chat template loaded OK");
            Some(t)
        }
        Err(e) => {
            warn!("LLM: no chat template in model: {e:?}, using manual Gemma format");
            None
        }
    };

    let _ = loading_tx.send(LlmLoadingStatus {
        message: "LLM ready".into(),
        progress: 1.0,
    });

    ready_flag.store(true, Ordering::SeqCst);
    info!("LLM: model loaded and ready ({})", model_path);

    // Single persistent context — cleared between requests via clear_kv_cache().
    // Creating a new context per call is too slow (~100ms overhead per allocation).
    let ctx_params = llama_cpp_2::context::params::LlamaContextParams::default()
        .with_n_ctx(std::num::NonZeroU32::new(2048))
        .with_n_batch(2048);
    let mut ctx = match model.new_context(&backend, ctx_params) {
        Ok(c) => c,
        Err(e) => {
            error!("LLM: failed to create context: {e:?}");
            ready_flag.store(true, Ordering::SeqCst);
            return;
        }
    };

    // Main request loop — check both channels
    loop {
        // Try dialogue first (higher priority — player is waiting)
        match dialogue_rx.try_recv() {
            Ok(req) => {
                let response =
                    generate_dialogue(&model, &mut ctx, &chat_template, &req);
                let _ = dialogue_tx.send(DialogueResponse {
                    text: response,
                    npc_entity: req.npc_entity,
                });
                continue;
            }
            Err(mpsc::TryRecvError::Disconnected) => break,
            Err(mpsc::TryRecvError::Empty) => {}
        }

        // Then decisions
        match decision_rx.try_recv() {
            Ok(req) => {
                info!("LLM: processing decision for {}", req.npc.name);
                let response =
                    generate_decision(&model, &mut ctx, &chat_template, &req);
                info!("LLM: decision for {}: {:?}", req.npc.name, response);
                let _ = decision_tx.send(DecisionResponse {
                    action: response,
                    npc_entity: req.npc_entity,
                });
                continue;
            }
            Err(mpsc::TryRecvError::Disconnected) => break,
            Err(mpsc::TryRecvError::Empty) => {}
        }

        // Nothing to do — sleep briefly to avoid busy-waiting
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
}

// ---------------------------------------------------------------------------
// Inference helpers
// ---------------------------------------------------------------------------

fn generate_dialogue(
    model: &llama_cpp_2::model::LlamaModel,
    ctx: &mut llama_cpp_2::context::LlamaContext,
    _chat_template: &Option<llama_cpp_2::model::LlamaChatTemplate>,
    req: &DialogueRequest,
) -> String {
    let system_prompt = build_system_prompt(&req.npc);

    // Single-turn with labeled dialogue roles.
    // Priming the model turn with "NpcName:" triggers pattern-completion
    // in the character's voice — reliable on small (2B) models.
    let mut prompt = String::new();
    prompt.push_str("<start_of_turn>user\n");
    prompt.push_str(&system_prompt);
    prompt.push_str("\n\nTraveler: ");
    prompt.push_str(&req.player_text);
    prompt.push_str("<end_of_turn>\n<start_of_turn>model\n");
    prompt.push_str(&req.npc.name);
    prompt.push_str(": ");

    let raw = run_inference(model, ctx, &prompt, 120, None);
    let result = clean_dialogue_output(&raw, &req.npc.name);

    // If the model produced nothing (common with very short inputs like "Hello"),
    // retry once with a slightly expanded prompt.
    if result.is_empty() {
        ctx.clear_kv_cache();
        let mut retry_prompt = String::new();
        retry_prompt.push_str("<start_of_turn>user\n");
        retry_prompt.push_str(&system_prompt);
        retry_prompt.push_str("\n\nA traveler greets you with: ");
        retry_prompt.push_str(&req.player_text);
        retry_prompt.push_str(". How do you greet them back?");
        retry_prompt.push_str("<end_of_turn>\n<start_of_turn>model\n");
        retry_prompt.push_str(&req.npc.name);
        retry_prompt.push_str(": ");
        let raw2 = run_inference(model, ctx, &retry_prompt, 120, None);
        return clean_dialogue_output(&raw2, &req.npc.name);
    }

    result
}

/// Strip thinking-mode artifacts and surrounding quotes from LLM dialogue output.
fn clean_dialogue_output(raw: &str, npc_name: &str) -> String {
    let mut text = raw.to_string();

    // Strip NPC name prefix if the model repeated it (we already primed with it)
    let prefix = format!("{}: ", npc_name);
    let trimmed_start = text.trim_start();
    if trimmed_start.starts_with(&prefix) {
        text = trimmed_start[prefix.len()..].to_string();
    }
    // Strip "Traveler:" prefix if model echoed the player's role label
    let trimmed_start = text.trim_start();
    if trimmed_start.starts_with("Traveler: ") || trimmed_start.starts_with("Traveler:") {
        text = trimmed_start[trimmed_start.find(':').unwrap() + 1..].trim_start().to_string();
    }

    // Remove channel tags and everything before the actual dialogue
    if let Some(pos) = text.find("<|channel>") {
        if let Some(after_thought) = text[pos..].find("Final Answer:") {
            text = text[pos + after_thought + "Final Answer:".len()..].trim().to_string();
        } else if let Some(blank_line) = text.rfind("\n\n") {
            text = text[blank_line + 2..].trim().to_string();
        } else {
            return String::new();
        }
    }

    // Strip special tokens
    text = text
        .replace("<end_of_turn>", "")
        .replace("</start_of_turn>", "")
        .replace("<start_of_turn>", "")
        .replace("<eos>", "");

    // Take only the first line — multi-line output clutters the chat log.
    let first_line = text.lines().next().unwrap_or("").trim();

    // Trim quotes if model wrapped the reply
    let cleaned = first_line.trim_matches('"').trim_matches('\'').trim().to_string();

    // Reject output that is a leak of prompt instructions / raw prompt content.
    // These markers never belong in in-character dialogue.
    let lower = cleaned.to_lowercase();
    let leak_markers = [
        "the traveler says to you",
        "reply in character",
        "respond in character",
        "spoken words, no more than",
        "speak naturally in your character",
        "no more than 2 sentences",
        "never narrate",
        "never describe actions",
        "no narration",
    ];
    for marker in &leak_markers {
        if lower.contains(marker) {
            use std::sync::atomic::Ordering;
            crate::debug_overlay::llm_stats()
                .total_rejections
                .fetch_add(1, Ordering::Relaxed);
            crate::game_log::push_llm_error(format!(
                "dialogue output rejected: prompt leak detected ({marker}): {cleaned:?}"
            ));
            return String::new();
        }
    }

    cleaned
}

fn generate_decision(
    model: &llama_cpp_2::model::LlamaModel,
    ctx: &mut llama_cpp_2::context::LlamaContext,
    _chat_template: &Option<llama_cpp_2::model::LlamaChatTemplate>,
    req: &DecisionRequest,
) -> LlmAction {
    let decision_prompt = build_decision_prompt(&req.npc, &req.surroundings);

    let prompt = format!(
        "<start_of_turn>user\n{}<end_of_turn>\n<start_of_turn>model\n",
        decision_prompt
    );

    let text = run_inference(model, ctx, &prompt, 80, None);
    info!("LLM: raw decision output: {:?}", text);
    parse_decision(&text)
}

/// GBNF grammar constraining NPC decision output to valid JSON actions.
const DECISION_GRAMMAR: &str = r#"
root ::= "{" ws action ws "}"
action ::= idle | speak | speak-to | move-to | give
idle ::= "\"action\": \"idle\""
speak ::= "\"action\": \"speak\", \"text\": " string
speak-to ::= "\"action\": \"speak_to\", \"target\": " string ", \"text\": " string
move-to ::= "\"action\": \"move_to\", \"target\": " string
give ::= "\"action\": \"give\", \"target\": " string ", \"item\": " string ", \"text\": " string
string ::= "\"" chars "\""
chars ::= char*
char ::= [^"\\] | "\\" escape
escape ::= ["\\nrt]
ws ::= [ \t\n]*
"#;

fn run_inference(
    model: &llama_cpp_2::model::LlamaModel,
    ctx: &mut llama_cpp_2::context::LlamaContext,
    prompt: &str,
    max_tokens: usize,
    grammar: Option<&str>,
) -> String {
    use llama_cpp_2::llama_batch::LlamaBatch;
    use llama_cpp_2::model::AddBos;
    use llama_cpp_2::sampling::LlamaSampler;

    // Clear KV cache from previous inference.
    ctx.clear_kv_cache();

    // Tokenize prompt (add BOS — Gemma models require it for proper generation)
    let tokens = match model.str_to_token(prompt, AddBos::Always) {
        Ok(t) => t,
        Err(e) => {
            let msg = format!("tokenization failed: {e:?}");
            error!("LLM: {msg}");
            crate::game_log::push_llm_error(msg);
            return String::new();
        }
    };

    // Create batch and fill with prompt tokens
    let mut batch = LlamaBatch::new(tokens.len().max(1), 1);
    for (i, &tok) in tokens.iter().enumerate() {
        let is_last = i == tokens.len() - 1;
        let _ = batch.add(tok, i as i32, &[0], is_last);
    }

    // Decode prompt
    if let Err(e) = ctx.decode(&mut batch) {
        let msg = format!("prompt decode failed: {e:?}");
        error!("LLM: {msg}");
        crate::game_log::push_llm_error(msg);
        return String::new();
    }

    // Set up sampler — with optional grammar constraint
    let mut samplers: Vec<LlamaSampler> = vec![
        LlamaSampler::temp(0.8),
        LlamaSampler::top_p(0.9, 1),
        LlamaSampler::top_k(40),
    ];
    if let Some(grammar_str) = grammar {
        match LlamaSampler::grammar(model, grammar_str, "root") {
            Ok(g) => samplers.push(g),
            Err(e) => warn!("LLM: grammar init failed: {e:?}"),
        }
    }
    samplers.push(LlamaSampler::dist(42));
    let mut sampler = LlamaSampler::chain_simple(samplers);

    let n_prompt = tokens.len();
    let n_ctx = ctx.n_ctx() as usize;
    info!("LLM: prompt={} tokens, ctx={} tokens, budget={} tokens for generation",
        n_prompt, n_ctx, n_ctx.saturating_sub(n_prompt));
    {
        use std::sync::atomic::Ordering;
        let stats = crate::debug_overlay::llm_stats();
        stats.last_prompt_tokens.store(n_prompt, Ordering::Relaxed);
        stats.last_ctx_tokens.store(n_ctx, Ordering::Relaxed);
        stats.total_requests.fetch_add(1, Ordering::Relaxed);
    }

    // Generate tokens
    let mut output = String::new();
    let mut decoder = encoding_rs::UTF_8.new_decoder();
    let mut n_cur = tokens.len();

    for i in 0..max_tokens {
        let token = sampler.sample(&ctx, -1);
        sampler.accept(token);

        // Check for end of generation
        if model.is_eog_token(token) {
            info!("LLM: EOS at token {i}, token_id={}", token.0);
            break;
        }

        // Convert token to text
        if let Ok(piece) = model.token_to_piece(token, &mut decoder, true, None) {
            output.push_str(&piece);
            // Stop after the first complete sentence (period/question/exclamation + space or newline).
            let trimmed = output.trim();
            if trimmed.len() > 3 {
                let last_chars: Vec<char> = trimmed.chars().rev().take(2).collect();
                if matches!(last_chars.as_slice(),
                    [' ' | '\n', '.' | '!' | '?'] | ['\n', _]
                ) {
                    break;
                }
            }
        }

        // Prepare next batch
        batch.clear();
        let _ = batch.add(token, n_cur as i32, &[0], true);
        n_cur += 1;

        if let Err(e) = ctx.decode(&mut batch) {
            let msg = format!("decode failed at token {n_cur}: {e:?}");
            error!("LLM: {msg}");
            crate::game_log::push_llm_error(msg);
            break;
        }
    }

    output.trim().to_string()
}

fn parse_decision(text: &str) -> LlmAction {
    // Clean up: remove <eos>, code fences, leading/trailing whitespace
    let clean = text
        .replace("<eos>", "")
        .replace("```json", "")
        .replace("```", "")
        .replace("<end_of_turn>", "");
    let clean = clean.trim();

    // Try JSON first
    if let Some(action) = try_parse_json(clean) {
        return action;
    }

    // Try key-value format: action: speak\ntarget: X\ntext: "Y"
    if let Some(action) = try_parse_keyvalue(clean) {
        return action;
    }

    LlmAction::Idle
}

fn try_parse_json(text: &str) -> Option<LlmAction> {
    let start = text.find('{')?;
    let mut depth = 0;
    let mut end = start;
    for (i, ch) in text[start..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    end = start + i;
                    break;
                }
            }
            _ => {}
        }
    }
    if depth != 0 {
        return None;
    }

    let val: serde_json::Value = serde_json::from_str(&text[start..=end]).ok()?;
    Some(value_to_action(&val))
}

fn try_parse_keyvalue(text: &str) -> Option<LlmAction> {
    let mut action = None;
    let mut target = None;
    let mut dialogue = None;

    for line in text.lines() {
        let line = line.trim().trim_matches(',');
        if let Some(rest) = line.strip_prefix("action:").or_else(|| line.strip_prefix("\"action\":")) {
            action = Some(rest.trim().trim_matches('"').trim_matches('\'').to_string());
        } else if let Some(rest) = line.strip_prefix("target:").or_else(|| line.strip_prefix("\"target\":")) {
            target = Some(rest.trim().trim_matches('"').trim_matches('\'').to_string());
        } else if let Some(rest) = line.strip_prefix("text:").or_else(|| line.strip_prefix("\"text\":")) {
            dialogue = Some(rest.trim().trim_matches('"').trim_matches('\'').to_string());
        }
    }

    let action_str = action?;
    match action_str.as_str() {
        "speak" | "speak_to" => {
            let text = dialogue.filter(|t| !t.is_empty())?;
            if let Some(tgt) = target.filter(|t| !t.is_empty()) {
                Some(LlmAction::SpeakTo { target_name: tgt, text })
            } else {
                Some(LlmAction::Speak(text))
            }
        }
        "move_to" => {
            let tgt = target.filter(|t| !t.is_empty())?;
            Some(LlmAction::MoveTo { target_name: tgt })
        }
        "idle" => Some(LlmAction::Idle),
        _ => None,
    }
}

fn value_to_action(val: &serde_json::Value) -> LlmAction {
    let action = val.get("action").and_then(|a| a.as_str()).unwrap_or("idle");
    let target = val.get("target").and_then(|t| t.as_str()).unwrap_or("").to_string();
    let text = val.get("text").and_then(|t| t.as_str()).unwrap_or("").to_string();

    let item = val.get("item").and_then(|t| t.as_str()).unwrap_or("").to_string();

    match action {
        "speak" if !text.is_empty() => {
            if !target.is_empty() {
                LlmAction::SpeakTo { target_name: target, text }
            } else {
                LlmAction::Speak(text)
            }
        }
        "speak_to" if !text.is_empty() && !target.is_empty() => {
            LlmAction::SpeakTo { target_name: target, text }
        }
        "move_to" if !target.is_empty() => LlmAction::MoveTo { target_name: target },
        "give" if !target.is_empty() && !item.is_empty() => {
            LlmAction::Give { target_name: target, item_id: item, text }
        }
        _ => LlmAction::Idle,
    }
}

// ---------------------------------------------------------------------------
// Plugin
// ---------------------------------------------------------------------------

pub struct LlmPlugin;

impl Plugin for LlmPlugin {
    fn build(&self, app: &mut App) {
        let engine = LlmEngine::new();
        app.insert_resource(engine);
    }
}
