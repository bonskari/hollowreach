# Hollowreach — Technical Plan

Based on design discussion 2026-04-07. This is the implementation blueprint.

---

## 1. Data Model (JSON Schemas)

### 1.1 Entity Config

Every object in the world. Stored as `assets/entities/<id>.json`.

```json
{
  "id": "door_tavern",
  "type": "prop",
  "model": "kaykit_dungeon/wall_doorway.gltf",
  "state": "locked",
  "collider": { "type": "circle", "radius": 0.5 },
  "interactions": [ ... ],
  "use_positions": [ ... ]
}
```

**Fields:**
- `id` — unique string
- `type` — `"prop"`, `"item"`, `"npc"`, `"furniture"`
- `model` — path to GLTF/GLB
- `state` — current state string, free-form (e.g. `"locked"`, `"open"`, `"empty"`)
- `collider` — optional collision shape
- `interactions` — list of available interactions (see 1.2)
- `use_positions` — where a character stands/sits to use this entity (see 1.4)

### 1.2 Interaction

```json
{
  "id": "unlock",
  "label": "Unlock",
  "conditions": [
    { "type": "entity_state", "state": "locked" },
    { "type": "actor_has_item", "item": "iron_key" }
  ],
  "reaction": {
    "animation": "door_unlock",
    "sound": "lock_click",
    "state_change": "unlocked",
    "remove_item": "iron_key",
    "set_flag": "tavern_door_unlocked",
    "target_entity": null,
    "target_state": null,
    "dialogue_prompt": null
  }
}
```

**Condition types:**
- `entity_state` — this entity must be in state X
- `actor_has_item` — whoever does this must have item X
- `flag_set` — global flag must be set
- `flag_not_set` — global flag must not be set
- `other_entity_state` — another entity (by ID) must be in state X

**Reaction fields** (all optional, null = skip):
- `animation` — animation to play on the entity
- `actor_animation` — animation to play on the character doing it
- `sound` — sound effect to play
- `state_change` — new state for this entity
- `target_entity` + `target_state` — change another entity's state (lever → gate)
- `remove_item` — consume item from actor's inventory
- `spawn_item` — give item to actor
- `set_flag` — set a global flag
- `dialogue_prompt` — LLM prompt for generating a spoken response
- `info_text` — static text to show (for informational interactions like "Locked")

### 1.3 NPC Config

Extends entity config. Stored as `assets/npcs/<id>.json`.

```json
{
  "id": "sir_roland",
  "type": "npc",
  "model": "kaykit_characters/Knight.glb",
  "state": "idle",
  "collider": { "type": "circle", "radius": 0.5 },

  "personality": {
    "name": "Sir Roland",
    "role": "Village Guard",
    "traits": ["dutiful", "suspicious", "dry humor"],
    "backstory": "A knight who once served a fallen kingdom...",
    "speech_style": "Formal, short sentences. Never swears.",
    "voice_profile": "deep_male_01",
    "knowledge": ["The abyss has been growing", "Whisper steals"],
    "goals": ["Keep the village safe", "Find the supply thief"],
    "likes": ["order", "honesty"],
    "dislikes": ["thieves", "cowardice"]
  },

  "inventory": [],
  "memory": [],
  "interactions": [
    { "id": "talk", "label": "Talk", "conditions": [], "reaction": { "dialogue_prompt": "The player wants to talk to you. Respond in character." } },
    { "id": "give_item", "label": "Give item", "conditions": [{ "type": "actor_has_item", "item": "*" }], "reaction": { "dialogue_prompt": "The player gives you {item}. React in character." } }
  ]
}
```

### 1.4 Use Position (entity-driven animation)

Entities that characters can "use" (sit on, drink from, etc.) define use positions:

```json
{
  "use_positions": [
    {
      "id": "seat_left",
      "offset": [0.3, 0.0, 0.0],
      "rotation_y": 90,
      "actor_animation": "Sit_Idle",
      "enter_animation": "Sit_Down",
      "exit_animation": "Stand_Up",
      "occupied_by": null
    }
  ]
}
```

The entity tells the character where to go and what animation to play. The character doesn't know how to sit — the bench knows.

### 1.5 Context Area

Defines a region of the world. Stored as `assets/areas/<id>.json`.

```json
{
  "id": "tavern",
  "label": "The Tavern",
  "bounds": { "min": [-8, -4], "max": [-2, 2] },
  "description": "A warm corner with tables, stools, and the smell of ale.",
  "entities": ["table_medium_01", "stool_01", "stool_02", "mug_01", "candle_01"],
  "adjacent_areas": ["courtyard", "sleeping_quarters"]
}
```

At runtime, the system collects all entities in the area + all NPCs present and builds the LLM context.

---

## 2. Bevy ECS Architecture

### 2.1 Components

```rust
// Identity
struct EntityId(String);
struct EntityState(String);

// Interaction data (loaded from JSON)
struct InteractionList(Vec<Interaction>);
struct UsePositions(Vec<UsePosition>);

// NPC-specific
struct NpcPersonality { /* from JSON */ }
struct NpcMemory(Vec<MemoryEntry>);
struct NpcInventory(Vec<String>);
struct NpcDecisionState { current_action: Option<NpcAction>, cooldown: Timer }

// Player
struct PlayerInventory(Vec<String>);

// Context
struct InContextArea(String);  // which area this entity is in

// Runtime state
struct OccupiedBy(Option<Entity>);  // for use positions
struct GlobalFlags(HashSet<String>);  // resource, not component
```

### 2.2 Resources

```rust
struct GlobalFlags(HashSet<String>);
struct EntityConfigs(HashMap<String, EntityConfig>);  // loaded from JSON
struct AreaConfigs(HashMap<String, AreaConfig>);
struct LlmInterface { /* async channel to llama.cpp */ }
struct TtsInterface { /* async channel to Chatterbox */ }
```

### 2.3 Systems (execution order)

```
Startup:
  load_configs          — read all JSON from assets/entities/, assets/npcs/, assets/areas/
  spawn_world           — spawn entities from configs
  setup_audio           — load ambient, footsteps, UI sounds

Update (each frame):
  player_input          — WASD movement, mouse look
  player_collision      — collision against colliders and walls
  player_context        — determine which context area the player is in
  detect_nearby         — find interactable entities near player
  show_interactions_ui  — display available interactions for nearest entity
  handle_interaction    — player presses E → execute interaction reaction

  npc_context_update    — update each NPC's context area knowledge
  npc_decision_system   — poll LLM for NPC decisions (async, staggered)
  npc_execute_action    — execute NPC's current decided action
  npc_pathfinding       — move NPCs toward their target entity
  npc_animation         — play correct animation based on current action

  interaction_effects   — process state changes, flag sets, cross-entity effects
  audio_system          — footsteps, ambient, spatial NPC voice
  tts_system            — poll Chatterbox for completed audio, play it
  ui_system             — dialogue box, hints, fade animations
```

---

## 3. LLM Integration

### 3.1 Runtime

- **llama.cpp** via `llama-cpp-rs` Rust bindings
- Runs **in-process**, no external server
- GGUF model (~1.8 GB Q4 3B) bundled in `assets/models/`
- Async: game sends prompt via channel, LLM processes on background thread, result comes back

### 3.2 NPC Decision Prompt Structure

```
[SYSTEM]
You are {name}, {role} in the village of Hollowreach.
Personality: {traits}
Backstory: {backstory}
Speech style: {speech_style}
Goals: {goals}
Likes: {likes}
Dislikes: {dislikes}
Your inventory: {inventory}

[CONTEXT]
You are in: {area.label} — {area.description}
Entities here:
- Table (state: has_mug) — interactions: [use, examine]
- Mug (state: full) — interactions: [drink, pick_up]
- Door (state: locked) — interactions: [examine] (you don't have the key)
- Elara the Wise (NPC, sitting at table) — interactions: [talk, give_item]
- Player (standing near door, carrying: iron_key)

[MEMORY]
- Earlier today: Player gave you bread. You thanked them.
- Yesterday: You saw Whisper sneaking near the storage.

[INSTRUCTION]
Choose ONE action. Respond in this exact format:
ACTION: <action_type> <target_id> [optional speech]

Valid actions:
- INTERACT <entity_id> <interaction_id>
- MOVE_TO <entity_id>
- SPEAK "<text>"
- SPEAK_TO <npc_id> "<text>"
- GIVE <npc_id_or_player> <item_id>
- IDLE
```

### 3.3 Output Parsing

LLM output is parsed into a structured `NpcAction` enum. If parsing fails or action is invalid (conditions not met), NPC defaults to IDLE and optionally comments on the failure ("Locked... who has the key?").

### 3.4 Staggered Ticking

Not every NPC decides every frame. Decision loop is staggered:
- Each NPC has a cooldown timer (3-10 seconds depending on situation)
- NPCs near the player tick more frequently
- NPCs in other areas tick rarely or are paused
- Currently executing an action → don't query LLM until done

---

## 4. TTS Integration (Chatterbox)

- Runs in a Python subprocess via `chatterbox-venv`
- Game sends text + voice_profile via IPC (stdin/stdout or socket)
- Chatterbox generates WAV, returns path or audio data
- Bevy loads and plays as spatial audio at NPC position
- ~1.2 seconds per sentence, faster than real-time
- 3 GB VRAM on GPU; CPU fallback for low-end systems

---

## 5. Pathfinding

Simple approach for indoor/village environments:
- **Nav mesh** or **waypoint graph** per context area
- NPCs pathfind to target entity's position (or use_position)
- Walking animation while moving, transition to interaction animation on arrival
- Collision avoidance between NPCs (simple steering)

---

## 6. Animation System

All animation is entity-driven:

| Trigger | Animation source | Example |
|---------|-----------------|---------|
| NPC idle | NPC default | Idle_A from KayKit |
| NPC walks | NPC movement | Walk from KayKit |
| NPC uses entity | Entity's use_position.actor_animation | Sit (from bench) |
| Entity reacted to | Entity's reaction.animation | Door opens |
| NPC speaks | NPC talk animation | Wave or gesture |
| NPC fails action | NPC reaction | Shrug or comment |

KayKit animations available: Idle, Walk, Run, Sit (if available), Wave, Interact, Pick Up, Throw, Dance, Cheer, Defeat, and 20+ more.

---

## 7. Memory System

### Short-term (in LLM context)
- Recent events as text: "2 minutes ago: Grok drank from the mug"
- Conversation history: last N exchanges
- Limited by context window (~4K tokens for 3B model)

### Long-term (persisted)
- Saved to `saves/<npc_id>_memory.json`
- Older memories summarized by LLM periodically: "Last week: helped the player find the key"
- Important events flagged for permanent retention
- Loaded into context as condensed summary

---

## 8. Implementation Order

### Phase 1: Data Foundation
1. JSON loader for entity configs
2. Spawn entities from JSON (replace hardcoded setup_scene)
3. Context area system
4. Entity state component
5. Basic interaction system (player only, no LLM)

### Phase 2: Player Interactions
6. Inventory system (pick up, carry, use)
7. Condition evaluation (state checks, item checks, flags)
8. Reaction execution (state change, animation, sound, cross-entity)
9. Interaction UI (list of available interactions)
10. Entity-driven use positions and animations

### Phase 3: LLM NPC Brain
11. llama.cpp integration (async, in-process)
12. NPC decision prompt builder (context + persona + memory)
13. NPC action parser (structured output)
14. NPC decision loop (staggered ticking)
15. NPC pathfinding to target entity

### Phase 4: Voice
16. Chatterbox TTS integration (Python subprocess)
17. Per-NPC voice profiles
18. Spatial audio for NPC speech
19. Lip sync or talk animation trigger

### Phase 5: Living World
20. NPC memory persistence
21. NPC-to-NPC interaction
22. NPC opinion/relationship tracking
23. Day/night cycle affecting NPC behavior
24. World event system (emergent situations)

---

## 9. Target Specs

| Tier | RAM | VRAM | Notes |
|------|-----|------|-------|
| Minimum | 8 GB | 4 GB | LLM on CPU, TTS on CPU (slower) |
| Recommended | 16 GB | 8 GB | LLM on GPU, TTS on GPU |
| Optimal | 32 GB | 16 GB | Everything on GPU, fast responses |

Budget breakdown (recommended tier):
- Game + Bevy: ~1 GB RAM
- LLM (3B Q4 GGUF): ~1.8 GB (GPU VRAM or RAM)
- Chatterbox TTS: ~3 GB VRAM
- Assets: ~500 MB
- Total: ~6.3 GB

---

*This plan will evolve as implementation reveals new constraints.*
