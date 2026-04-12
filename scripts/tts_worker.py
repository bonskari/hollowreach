#!/usr/bin/env python3
"""Chatterbox TTS worker — persistent subprocess, JSON lines over stdin/stdout.

Protocol:
  Startup: prints {"status": "loading", "device": "cuda"} then {"status": "ready"}
  Request: {"text": "...", "voice_profile": "grok", "output_path": "/tmp/..."}
  Response: {"status": "done", "path": "/tmp/..."} or {"status": "error", "error": "..."}
"""

import json
import sys
import os

# Disable torchcodec to avoid libnppicc dependency
os.environ["TORCHCODEC_DISABLED"] = "1"

import torch
import torchaudio

# Monkeypatch Chatterbox to use torchaudio instead of torchcodec for audio loading
import chatterbox.tts
_original_generate = None

def main():
    device = "cuda" if torch.cuda.is_available() else "cpu"
    send({"status": "loading", "device": device})

    try:
        from chatterbox.tts import ChatterboxTTS
        model = ChatterboxTTS.from_pretrained(device=device)
    except Exception as e:
        send({"status": "error", "error": str(e)})
        return

    # Cache loaded voice references
    voice_cache = {}

    send({"status": "ready", "device": device})

    for line in sys.stdin:
        line = line.strip()
        if not line:
            continue
        try:
            req = json.loads(line)
        except json.JSONDecodeError as e:
            send({"status": "error", "error": f"Invalid JSON: {e}"})
            continue

        text = req.get("text", "")
        voice_profile = req.get("voice_profile", "")
        output_path = req.get("output_path", "/tmp/tts_output.wav")

        try:
            # Load voice reference (cached)
            audio_prompt = None
            if voice_profile:
                if voice_profile not in voice_cache:
                    voice_path = f"assets/voices/{voice_profile}.wav"
                    if os.path.exists(voice_path):
                        voice_cache[voice_profile] = voice_path
                    else:
                        voice_cache[voice_profile] = None
                ref_path = voice_cache.get(voice_profile)
                if ref_path:
                    audio_prompt = ref_path

            # Generate speech
            wav = model.generate(text, audio_prompt_path=audio_prompt)

            # Ensure output directory exists
            os.makedirs(os.path.dirname(output_path), exist_ok=True)

            # Save WAV using Python's wave module (avoids torchcodec)
            import wave, numpy as np
            wav_np = wav.cpu().squeeze().numpy()
            wav_int16 = (wav_np * 32767).clip(-32768, 32767).astype(np.int16)
            with wave.open(output_path, 'wb') as wf:
                wf.setnchannels(1)
                wf.setsampwidth(2)
                wf.setframerate(24000)
                wf.writeframes(wav_int16.tobytes())

            send({"status": "done", "path": output_path})
        except Exception as e:
            send({"status": "error", "error": str(e)})


def send(obj):
    """Send JSON line to stdout (Rust reads this)."""
    print(json.dumps(obj), flush=True)


if __name__ == "__main__":
    main()
