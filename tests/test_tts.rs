//! Integration test for the full ONNX TTS pipeline.
//!
//! Loads all 4 ONNX models, runs the Chatterbox pipeline on "Hello there"
//! with the "grok" voice profile, and writes the output WAV to disk.

use hollowreach::tts::TtsEngine;
use ort::session::Session;
use tokenizers::Tokenizer;

fn main() {
    eprintln!("=== TTS Pipeline Integration Test ===\n");

    // Step 1: Load tokenizer
    eprintln!("[1/5] Loading tokenizer...");
    let tokenizer = Tokenizer::from_file("assets/models/tts/tokenizer.json")
        .expect("Failed to load tokenizer");
    eprintln!("  OK: tokenizer loaded\n");

    // Step 2: Load ONNX sessions
    eprintln!("[2/5] Loading ONNX models with CUDA...");
    let cuda = ort::ep::CUDA::default();

    eprintln!("  Loading embed_tokens_fp16...");
    let mut embed_tokens = Session::builder()
        .unwrap()
        .with_execution_providers([cuda.clone().build()])
        .unwrap()
        .commit_from_file("assets/models/tts/embed_tokens_fp16.onnx")
        .expect("Failed to load embed_tokens");
    eprintln!("  OK: embed_tokens loaded");

    eprintln!("  Loading speech_encoder_fp16...");
    let mut speech_encoder = Session::builder()
        .unwrap()
        .with_execution_providers([cuda.clone().build()])
        .unwrap()
        .commit_from_file("assets/models/tts/speech_encoder_fp16.onnx")
        .expect("Failed to load speech_encoder");
    eprintln!("  OK: speech_encoder loaded");

    eprintln!("  Loading language_model_fp16...");
    let mut lm = Session::builder()
        .unwrap()
        .with_execution_providers([cuda.clone().build()])
        .unwrap()
        .commit_from_file("assets/models/tts/language_model_fp16.onnx")
        .expect("Failed to load language_model");
    eprintln!("  OK: language_model loaded");

    eprintln!("  Loading conditional_decoder_fp16...");
    let mut decoder = Session::builder()
        .unwrap()
        .with_execution_providers([cuda.build()])
        .unwrap()
        .commit_from_file("assets/models/tts/conditional_decoder_fp16.onnx")
        .expect("Failed to load conditional_decoder");
    eprintln!("  OK: all models loaded\n");

    // Step 3: Run the pipeline
    eprintln!("[3/5] Running TTS pipeline with text=\"Hello there\", voice=\"grok\"...");
    let start = std::time::Instant::now();

    let result = TtsEngine::run_pipeline(
        "Hello there",
        "grok",
        &tokenizer,
        &mut embed_tokens,
        &mut speech_encoder,
        &mut lm,
        &mut decoder,
    );

    let elapsed = start.elapsed();
    eprintln!("  Pipeline completed in {:.2}s\n", elapsed.as_secs_f64());

    // Step 4: Check result
    eprintln!("[4/5] Checking result...");
    match result {
        Ok(wav_data) => {
            eprintln!("  OK: got {} bytes of WAV data", wav_data.len());
            assert!(!wav_data.is_empty(), "WAV data should not be empty");
            assert!(wav_data.len() > 44, "WAV data should be larger than just a header");

            // Parse WAV header to report duration
            if wav_data.len() >= 44 {
                let sample_rate =
                    u32::from_le_bytes(wav_data[24..28].try_into().unwrap());
                let bits_per_sample =
                    u16::from_le_bytes(wav_data[34..36].try_into().unwrap());
                let data_size =
                    u32::from_le_bytes(wav_data[40..44].try_into().unwrap());
                let num_samples =
                    data_size / (bits_per_sample as u32 / 8);
                let duration_s = num_samples as f64 / sample_rate as f64;
                eprintln!(
                    "  WAV: {}Hz, {}bit, {} samples, {:.2}s duration",
                    sample_rate, bits_per_sample, num_samples, duration_s
                );
            }

            // Step 5: Write output
            eprintln!("\n[5/5] Writing output WAV...");
            std::fs::create_dir_all("test_screenshots")
                .expect("Failed to create test_screenshots dir");
            std::fs::write("test_screenshots/tts_test_output.wav", &wav_data)
                .expect("Failed to write WAV file");
            eprintln!("  OK: written to test_screenshots/tts_test_output.wav");

            eprintln!("\n=== TTS Pipeline Test PASSED ===");
        }
        Err(e) => {
            eprintln!("  FAILED: {}", e);
            eprintln!("\n=== TTS Pipeline Test FAILED ===");
            std::process::exit(1);
        }
    }
}
