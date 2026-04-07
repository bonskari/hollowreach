# Hollowreach -- Game Design Document

Working document. Last updated 2026-04-07.

---

## 1. Concept

Hollowreach is a first-person 3D exploration game set in a medieval fantasy village perched on the edge of a vast abyss. The ground beneath the village remembers ancient truths -- ley lines pulse with forgotten energy, and something stirs in the depths below.

The player arrives as a traveler and discovers a small, lived-in settlement populated by NPCs who think, speak, and act through local large language models. There is no scripted quest log. The story emerges from conversations, discoveries, and the NPCs' own evolving goals.

**One-line pitch:** A village on the edge of an abyss, where every NPC has a mind of its own.

---

## 2. Core Pillars

**LLM-driven NPCs.** Every NPC reasons through a local LLM. They have personalities, memories, goals, and opinions that evolve over time. Dialogue is not a branching tree -- it is generated in real time.

**Voice.** NPC dialogue is spoken aloud via Chatterbox TTS, giving each character a distinct voice. Combined with LLM reasoning, conversations feel unrehearsed and alive.

**Exploration.** The village, surrounding wilderness, and the abyss below reward curiosity. Lore is embedded in the environment -- tattered maps, ancient banners, ley line markings -- not delivered through cutscenes.

**Emergent storytelling.** With NPCs that think and remember, stories arise from the simulation rather than from authored scripts. The rogue might betray the guard. The mage might discover something she shouldn't have. The player witnesses and participates.

---

## 3. Gameplay Loop

The player can do exactly four things:

1. **Talk** to NPCs
2. **Interact** with objects
3. **Carry** items (pick up and hold)
4. **Use** items on other items or NPCs (including giving items to NPCs)

That's it. There is **no combat**. The player cannot hit anyone. There are no health bars, no skill trees, no crafting, no leveling. This is a peaceful game.

The LLM-driven NPCs **are** the game. Every conversation is unique. NPCs remember what you said, what you gave them, and what happened. Emergent storytelling comes from NPC personalities, goals, and reactions to player actions. An NPC might ask for an item, react with surprise when given something unexpected, or tell another NPC about what you did.

There are no scripted quests. No quest markers. The player discovers what to do by talking to people, exploring, and experimenting with items.

---

## 4. NPC System

### Architecture

Each NPC is defined by a data-driven configuration:

- **Name and role** (e.g., "Sir Roland", village guard)
- **Personality traits** (e.g., dutiful, suspicious, dry humor)
- **Goals** (e.g., protect the village, uncover who's been stealing supplies)
- **Memory** (past interactions with the player and other NPCs)
- **Knowledge** (what they know about the world, which may be incomplete or wrong)

### LLM Integration

- The LLM runs **inside the game process** -- no external server, no Ollama, no cloud API
- Likely approach: llama.cpp via Rust bindings (llama-cpp-rs) with a GGUF model bundled in assets
- Target specs (Steam survey top 3):
  - Minimum: 8 GB RAM, 4 GB VRAM
  - Recommended: 16 GB RAM, 8 GB VRAM
  - Optimal: 32 GB RAM, 16 GB VRAM
- A 3B parameter Q4 GGUF model uses ~1.8GB -- fits all tiers
- Chatterbox TTS uses ~3GB VRAM -- needs fallback for 4GB cards
- NPC systems send async queries: "Given this personality, these memories, and this situation, what does this character say/do?"
- The LLM handles both dialogue generation and behavioral decisions

### Voice

- Chatterbox TTS converts generated dialogue text to audio
- Each NPC has a distinct voice profile
- Audio is streamed into Bevy's audio system as it is generated

### Current NPCs

| Name | Class | Role | Location |
|------|-------|------|----------|
| Sir Roland | Knight | Village guard | Near entrance |
| Elara the Wise | Mage | Scholar of ley lines | Tavern area |
| Whisper | Rogue (Hooded) | Information broker | Storage corner |
| Grok | Barbarian | Reluctant visitor | Center of courtyard |
| Sylva | Ranger | Wilderness tracker | Lookout post |

Each currently has a single hardcoded dialogue line. The LLM system will replace these with dynamic conversation.

---

## 5. World

### The Village

Hollowreach is a walled settlement built from old stone. The current playable area is a courtyard enclosed on three sides by walls, with an open southern edge. Inside:

- A decorated central pillar (possibly a ley line marker)
- A tavern area with table, stools, and bench
- Storage area with barrels, crates, and shelves
- A sleeping quarters with beds
- A treasure chest containing a tattered map
- Wall-mounted torches, colored banners
- A gabled roof overhead

The village is built from modular dungeon pieces (KayKit Dungeon Remastered) arranged to feel open and lived-in rather than claustrophobic.

### Beyond the Walls (planned)

- **The Forest:** Grows darker each night. Sylva tracks something unnatural within it.
- **The Ruins:** Remnants of whatever civilization came before Hollowreach.
- **The Abyss:** The defining geographic feature. A vast chasm at the village's edge. What's down there is the central mystery.

### The Ley Lines

Beneath the village, ley lines pulse with ancient energy. Elara studies them. They may be connected to the abyss, to the village's founding, or to something else entirely. This is the lore backbone.

---

## 6. Audio Design

**Ambient-first.** The soundscape should feel like a real place before it feels like a game. Priority layers:

1. **Environmental ambience** -- wind through stone walls, distant forest sounds, the low hum of the abyss
2. **Footsteps** -- surface-aware (stone, grass, wood)
3. **NPC voices** -- Chatterbox TTS, spatially positioned
4. **Prop sounds** -- torches crackling, chest hinges, door creaks
5. **Music** -- sparse and contextual, not a constant loop. Triggered by discovery, tension, or stillness. Think isolated instruments (a distant flute, a low drone) rather than orchestral score.

---

## 7. Visual Style

**Stylized low-poly with gradient atlas textures.** The art direction prioritizes consistency over fidelity.

- **Characters:** KayKit Adventurers (Kay Lousberg). Six archetypes (Barbarian, Knight, Mage, Ranger, Rogue, Rogue_Hooded). 24-joint skeleton, 26 included animations. CC0 licensed.
- **Environment:** KayKit Dungeon Remastered. 211 modular pieces (walls, floors, pillars, doors, props). Same gradient atlas style as characters.
- **Props:** Currently Quaternius Fantasy Props MegaKit (chests, barrels, torches, beds, shelves, benches, potions). Migration to KayKit dungeon props is in progress.
- **UI:** Kenney Fantasy UI planned for dialogue boxes and menus.
- **Ground/terrain:** Simple colored materials for now. Stylized terrain textures planned.
- **Lighting:** Warm torch light inside the village, cooler tones beyond the walls. Directional sun with soft shadows.

**Style rule:** No mixing realistic PBR with stylized models. All new assets must match the KayKit gradient atlas look. Visual consistency is non-negotiable.

**Atmosphere:** Warm and inviting inside the village walls. Mysterious and unsettling at the edges. The abyss should feel vast and unknowable.

---

## 8. Technical Architecture

### Engine

Bevy (Rust ECS game engine). Current version: 0.15.

### Code Structure

- `src/lib.rs` -- all components, systems, resources, and setup. Everything is public for test access.
- `src/main.rs` -- entry point only. Constructs the Bevy app and adds `HollowreachPlugin`.
- `tests/` -- end-to-end integration tests with real renderer and screenshot capture.

### Key Systems

| System | Purpose |
|--------|---------|
| `setup_scene` | Spawns the entire world: ground, walls, props, NPCs, lights, camera |
| `player_movement` | WASD first-person movement |
| `player_collision` | AABB + circle collision against walls, props, NPCs, world bounds |
| `player_look` | Mouse-driven camera with pitch/yaw |
| `interact_system` | E-key interaction with nearby Interactable entities |
| `proximity_hint_system` | Shows "[E] Talk/Interact" when near interactables |
| `dialogue_fade_system` | Timed fade-out of dialogue box |
| `start_npc_animations` | Plays idle animations on NPC character models |
| `hide_unwanted_meshes` | Hides entities named `_hidden*` (Blender convention) |
| `intro_system` | Title card fade-in on game start |

### Design Principles

- **Data-driven.** NPC configs, asset paths, outfit parts should come from data, not hardcoded strings. The code should be generic enough to generate different characters at runtime from configuration.
- **Convention over configuration.** Use naming conventions (e.g., `_hidden` prefix) rather than asset-specific logic.
- **Test everything.** Every feature needs an e2e test with screenshots, run under xvfb (virtual display). Tests are adversarial -- they try to break things, not just verify happy paths.

### Planned Integrations

- **llama.cpp** (via llama-cpp-rs) -- LLM bundled inside the game binary. No external server. GGUF model in assets/.
- **Chatterbox TTS** -- local text-to-speech for NPC voice. Integrates with Bevy audio.

### Asset Pipeline

Characters use a two-step pipeline:

1. **Blender export** (headless script) -- builds character meshes from modular parts, strips unwanted geometry, exports GLB with skeleton but no animations.
2. **Animation merge** (Python script) -- binary-level merge of animation data from a universal animation library into the character GLB, remapping bone indices by name.

This two-step process exists because Blender 5.x's action slot system doesn't properly handle animations imported from different armatures during GLTF export.

---

## 9. Current State

### What Works (as of 2026-04-06)

- Full lib.rs/main.rs split with HollowreachPlugin
- First-person movement and mouse look
- AABB collision system (walls, pillar, NPCs, world boundaries)
- On-screen dialogue UI with proximity hints and interaction cooldown
- Five animated NPCs (Knight, Mage, Rogue_Hooded, Barbarian, Ranger) with idle loops
- Furnished village courtyard with props (chests, barrels, torches, beds, tables, banners, etc.)
- Gabled roof structure
- Intro title card with fade-in
- 20 e2e tests running under xvfb virtual display
- Character pipeline (Blender export + binary animation merge)

### Known Issues

- Black sky (no skybox or atmosphere)
- Chest prop incorrectly receives idle animation (no visual effect since bone IDs don't match, but wasteful)
- Asset sources are mixed (KayKit characters/environment + Quaternius props) -- migration incomplete

---

## 10. Roadmap

Priority order. Each item builds on the previous.

### Phase 1: Foundation

1. **Skybox / atmosphere** -- replace the black void with a sky. Sets the mood for everything else.
2. **Complete KayKit migration** -- replace remaining Quaternius props with KayKit Dungeon Remastered equivalents for visual consistency.
3. **Terrain and ground** -- replace flat colored plane with textured, slightly uneven terrain.
4. **Ambient audio** -- wind, torch crackle, environmental atmosphere. Audio exists before NPCs speak.

### Phase 2: NPC Intelligence

5. **LLM integration** -- llama.cpp bundled in-process via Rust bindings. Async inference from Bevy systems.
6. **NPC personality system** -- data-driven NPC configs (personality, goals, knowledge, memory) loaded from files.
7. **Dynamic dialogue** -- replace hardcoded dialogue strings with LLM-generated responses based on NPC state and conversation context.
8. **NPC memory** -- NPCs remember past interactions and reference them in future conversations.

### Phase 3: Voice

9. **Chatterbox TTS integration** -- convert generated dialogue to spoken audio.
10. **Per-NPC voice profiles** -- distinct voice characteristics for each character.
11. **Spatial audio** -- NPC voices positioned in 3D space, attenuating with distance.

### Phase 4: World Expansion

12. **Village exterior** -- forests, paths, the edge of the abyss.
13. **The abyss** -- visual and audio design for the central geographic mystery.
14. **Additional locations** -- ruins, caves, forest clearings.
15. **More NPCs** -- expand the cast as the world grows.

### Phase 5: Emergent Systems

16. **NPC-to-NPC interaction** -- characters talk to each other, form opinions, make plans.
17. **NPC behavior / routines** -- characters move through the world, perform tasks, sleep, eat.
18. **Consequence system** -- NPC actions have visible effects on the world.
19. **Day/night cycle** -- time passage affects NPC behavior and world atmosphere.

---

## Appendix: Asset Sources

| Category | Source | License | Format |
|----------|--------|---------|--------|
| Characters | KayKit Adventurers (Kay Lousberg) | CC0 | GLB |
| Environment | KayKit Dungeon Remastered | CC0 | GLTF |
| Props (current) | Quaternius Fantasy Props MegaKit | CC0 | GLTF |
| Animations | KayKit (included with Adventurers) | CC0 | GLB |
| UI (planned) | Kenney Fantasy UI | CC0 | PNG |
