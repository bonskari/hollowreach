# LLM Integration Research for NPC AI (llama.cpp + Rust/Bevy)

Research date: 2026-04-07

## Crate Comparison

### 1. `llama-cpp-2` (RECOMMENDED)

- **Repository:** https://github.com/utilityai/llama-cpp-rs
- **Version:** 0.1.142 (released 2026-04-06)
- **License:** MIT / Apache-2.0
- **Stars:** 527, Forks: 174, Commits: 1,577
- **Status:** Actively maintained, tracks latest llama.cpp closely
- Safe Rust wrappers around llama.cpp bindings
- GGUF support built-in
- GPU features: `cuda`, `metal`, `vulkan`, `rocm`
- Most popular and best-maintained option

### 2. `llama-cpp-4`

- **Repository:** https://github.com/eugenehp/llama-cpp-rs (fork)
- **Version:** 0.2.26
- **Stars:** 13
- Tracks upstream llama.cpp (April 2026 / c30e01225)
- Similar feature set (cuda, metal, vulkan, opencl, webgpu)
- Much smaller community; riskier long-term bet

### 3. `llama_cpp_rs`

- **Repository:** https://github.com/mdrokz/rust-llama.cpp
- **Version:** 0.3.0
- Older, fewer features, no vulkan support
- Not recommended

### 4. `llama-gguf` (Pure Rust)

- **Version:** 0.14.0
- Pure Rust reimplementation (not bindings)
- Supports CUDA, Metal, Vulkan via native Rust
- Interesting but less battle-tested than C++ bindings
- Could be worth revisiting later

### Decision: Use `llama-cpp-2`

Most maintained, largest community, tracks upstream closely, proven in production.

---

## Cargo.toml Integration

```toml
[dependencies]
llama-cpp-2 = "0.1"

# For GPU acceleration, enable one of:
# llama-cpp-2 = { version = "0.1", features = ["cuda"] }
# llama-cpp-2 = { version = "0.1", features = ["vulkan"] }
# llama-cpp-2 = { version = "0.1", features = ["metal"] }  # macOS only
```

**Build requirements:** cmake, a C/C++ compiler. The crate builds llama.cpp from source via its `-sys` crate.

---

## Example: Basic Inference

```rust
use llama_cpp_2::context::params::LlamaContextParams;
use llama_cpp_2::llama_backend::LlamaBackend;
use llama_cpp_2::llama_batch::LlamaBatch;
use llama_cpp_2::model::params::LlamaModelParams;
use llama_cpp_2::model::LlamaModel;
use llama_cpp_2::model::{AddBos, Special};
use llama_cpp_2::sampling::LlamaSampler;
use std::io::Write;

fn run_inference(model_path: &str, prompt: &str) -> String {
    let backend = LlamaBackend::init().unwrap();
    let params = LlamaModelParams::default();
    let model = LlamaModel::load_from_file(&backend, model_path, &params)
        .expect("unable to load model");

    let ctx_params = LlamaContextParams::default();
    let mut ctx = model
        .new_context(&backend, ctx_params)
        .expect("unable to create context");

    let tokens_list = model
        .str_to_token(prompt, AddBos::Always)
        .expect("failed to tokenize");

    let max_tokens = 256;
    let mut batch = LlamaBatch::new(512, 1);

    let last_index = tokens_list.len() as i32 - 1;
    for (i, token) in (0_i32..).zip(tokens_list.into_iter()) {
        let is_last = i == last_index;
        batch.add(token, i, &[0], is_last).unwrap();
    }
    ctx.decode(&mut batch).expect("llama_decode() failed");

    let mut n_cur = batch.n_tokens();
    let mut decoder = encoding_rs::UTF_8.new_decoder();
    let mut sampler = LlamaSampler::greedy();
    let mut output = String::new();

    while n_cur <= max_tokens as i32 {
        let token = sampler.sample(&ctx, batch.n_tokens() - 1);
        sampler.accept(token);

        if token == model.token_eos() {
            break;
        }

        let piece = model.token_to_piece(token, &mut decoder, true, None).unwrap();
        output.push_str(&piece);

        batch.clear();
        batch.add(token, n_cur, &[0], true).unwrap();
        n_cur += 1;
        ctx.decode(&mut batch).expect("failed to eval");
    }

    output
}
```

**Note:** Also add `encoding_rs` to dependencies for the UTF-8 decoder:
```toml
encoding_rs = "0.8"
```

---

## Async Integration with Bevy (Background Thread + Channels)

llama.cpp inference is synchronous and CPU/GPU-intensive. It must NOT run on the
main thread or it will block Bevy's game loop. Use a dedicated thread with channels.

```rust
use bevy::prelude::*;
use std::sync::mpsc;

/// Request sent to the LLM thread
pub struct NpcDialogueRequest {
    pub npc_entity: Entity,
    pub system_prompt: String,
    pub user_message: String,
}

/// Response received from the LLM thread
pub struct NpcDialogueResponse {
    pub npc_entity: Entity,
    pub text: String,
}

#[derive(Resource)]
pub struct LlmBridge {
    pub sender: mpsc::Sender<NpcDialogueRequest>,
    pub receiver: mpsc::Receiver<NpcDialogueResponse>,
}

/// Spawn the LLM worker thread at startup
fn setup_llm(mut commands: Commands) {
    let (req_tx, req_rx) = mpsc::channel::<NpcDialogueRequest>();
    let (resp_tx, resp_rx) = mpsc::channel::<NpcDialogueResponse>();

    std::thread::spawn(move || {
        // Initialize once - model loading takes several seconds
        let backend = llama_cpp_2::llama_backend::LlamaBackend::init().unwrap();
        let params = llama_cpp_2::model::params::LlamaModelParams::default();
        let model = llama_cpp_2::model::LlamaModel::load_from_file(
            &backend,
            "assets/models/npc-dialogue.gguf",
            &params,
        )
        .expect("Failed to load LLM model");

        // Process requests in a loop
        while let Ok(request) = req_rx.recv() {
            let prompt = format!(
                "<|im_start|>system\n{}<|im_end|>\n<|im_start|>user\n{}<|im_end|>\n<|im_start|>assistant\n",
                request.system_prompt,
                request.user_message,
            );

            let response_text = run_inference_with_model(&backend, &model, &prompt);

            let _ = resp_tx.send(NpcDialogueResponse {
                npc_entity: request.npc_entity,
                text: response_text,
            });
        }
    });

    commands.insert_resource(LlmBridge {
        sender: req_tx,
        receiver: resp_rx,
    });
}

/// Bevy system: poll for completed LLM responses each frame
fn poll_llm_responses(
    bridge: Res<LlmBridge>,
    // mut query: Query<&mut NpcDialogue>,  // your NPC component
) {
    // Non-blocking: try_recv returns immediately
    while let Ok(response) = bridge.receiver.try_recv() {
        // Update the NPC's dialogue text
        println!("NPC {:?} says: {}", response.npc_entity, response.text);
        // if let Ok(mut dialogue) = query.get_mut(response.npc_entity) {
        //     dialogue.current_text = response.text;
        // }
    }
}
```

**Key design points:**
- Model is loaded once on the worker thread (takes 1-3 seconds)
- Requests are sent via `mpsc::Sender` (non-blocking from game thread)
- Responses are polled with `try_recv()` each frame (non-blocking)
- Only one model instance in memory; requests are queued and processed sequentially
- Could use `crossbeam` channels for better performance if needed

---

## Recommended Models

### Primary: Gemma 4 2B IT (Selected for Hollowreach)

| Property | Value |
|---|---|
| **Model** | google/gemma-4-E2B-it |
| **Quantization** | Q4_K_M |
| **File size** | 2.9 GB |
| **Parameters** | 2B |
| **VRAM required** | ~3 GB (full GPU offload) |
| **Download** | Available from HuggingFace as GGUF |
| **Chat template** | Gemma 4 (`<start_of_turn>user/model`) |

**Why this model:**
- Fits within 8 GB VRAM budget alongside Chatterbox TTS (~3 GB) and Bevy renderer (~2 GB)
- Gemma 4 architecture — strong instruction following for a 2B model
- Produces in-character, contextual NPC dialogue
- Fast inference on GPU via Vulkan (llama-cpp-2 crate)
- No CPU fallback — GPU only, 8 GB VRAM minimum

**VRAM budget (8 GB):**
- Bevy renderer: ~2 GB
- Gemma 4 2B LLM: ~3 GB
- Chatterbox TTS: ~3 GB
- Total: ~8 GB

### Previous candidates (not selected)

| Property | Value |
|---|---|
| **Model** | Llama-3.2-3B-Instruct |
| **Quantization** | Q4_K_M |
| **File size** | 2.02 GB |
| **Parameters** | 3B |
| **RAM required** | ~4-6 GB (CPU inference) |
| **VRAM required** | ~3 GB (full GPU offload) |
| **Download** | `huggingface-cli download hugging-quants/Llama-3.2-3B-Instruct-Q4_K_M-GGUF --include "llama-3.2-3b-instruct-q4_k_m.gguf" --local-dir ./` |
| **Chat template** | Llama 3 format |

**Trade-off:** Better dialogue quality but 2x the memory and slower inference.

### Ultra-lightweight fallback: SmolLM2 360M

- File size: ~200 MB (Q4_K_M)
- Adequate for very simple responses (greetings, directions)
- Useful as a fallback if system resources are constrained

---

## Memory and Performance Estimates

### SmolLM2 1.7B Q4_K_M (recommended)

| Scenario | RAM/VRAM | Tokens/sec (approx) |
|---|---|---|
| CPU only (modern x86) | ~2-3 GB RAM | 50-100 t/s |
| CPU only (older hardware) | ~2-3 GB RAM | 20-40 t/s |
| Full GPU offload (NVIDIA) | ~1.5 GB VRAM | 200-400 t/s |
| Full GPU offload (Vulkan) | ~1.5 GB VRAM | 150-300 t/s |

### Llama 3.2 3B Q4_K_M (alternative)

| Scenario | RAM/VRAM | Tokens/sec (approx) |
|---|---|---|
| CPU only (modern x86) | ~4-6 GB RAM | 25-50 t/s |
| Full GPU offload (NVIDIA) | ~3 GB VRAM | 100-200 t/s |

**Context for games:** A typical NPC response of 50-100 tokens would take:
- SmolLM2 on CPU: ~0.5-2 seconds (acceptable for dialogue popup)
- SmolLM2 on GPU: ~0.1-0.5 seconds (feels instant)
- Llama 3.2 3B on CPU: ~1-4 seconds (noticeable delay)

---

## GGUF Format Support

All recommended crates support GGUF natively. GGUF is the current standard format
for llama.cpp (replaced the older GGML format). Key advantages:

- Single-file model format (easy to distribute)
- Metadata embedded in file (tokenizer, architecture, etc.)
- Multiple quantization levels (Q4_K_M is the sweet spot for quality/size)
- Memory-mapped loading (fast startup)

---

## GPU Acceleration

`llama-cpp-2` supports GPU offloading via feature flags:

| Feature | Backend | Platform |
|---|---|---|
| `cuda` | NVIDIA CUDA | Linux, Windows |
| `vulkan` | Vulkan (any GPU) | Cross-platform |
| `metal` | Apple Metal | macOS |
| `rocm` | AMD ROCm | Linux |

For Hollowreach (which uses Bevy with wgpu/Vulkan), the `vulkan` feature is the
most natural fit since the GPU is already initialized for rendering. However,
`cuda` typically gives better performance on NVIDIA hardware.

**GPU layer offloading** can be configured via `LlamaModelParams` to control how
many transformer layers run on GPU vs CPU, allowing fine-tuning of VRAM usage.

---

## Integration Considerations for Hollowreach

1. **Ship model with game or download on first run?**
   - SmolLM2 1.7B Q4_K_M is only 1 GB - reasonable to ship or download once
   - Store in `assets/models/` directory

2. **NPC personality via system prompts:**
   ```
   You are Eldric, a grumpy blacksmith in a dark fantasy world.
   Keep responses under 3 sentences. Stay in character.
   Never break the fourth wall. Speak in a rough, direct manner.
   ```

3. **Response capping:** Limit `max_tokens` to 100-150 for snappy NPC dialogue.

4. **Caching:** Cache recent NPC responses to avoid re-inference for repeated questions.

5. **Fallback:** If inference takes too long or fails, fall back to pre-written dialogue trees.

6. **Build time:** llama.cpp compiles from C++ source via the sys crate. First build
   takes 2-5 minutes. Subsequent builds are cached.

7. **Binary size:** The llama.cpp library adds ~5-10 MB to the final binary.
