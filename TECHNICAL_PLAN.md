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
- `position` — [x, y, z] world position
- `rotation_y` — facing direction in degrees
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
  "reaction": [
    { "type": "sound", "asset": "lock_click" },
    { "type": "animation", "anim": "door_unlock" },
    { "type": "state_change", "new_state": "unlocked" },
    { "type": "remove_item", "item": "iron_key" },
    { "type": "set_flag", "flag": "tavern_door_unlocked" }
  ]
}
```

**Condition types:**
- `entity_state` — this entity must be in state X
- `actor_has_item` — whoever does this must have item X
- `flag_set` — global flag must be set
- `flag_not_set` — global flag must not be set
- `other_entity_state` — another entity (by ID) must be in state X

**Reaction** is a list of effects, executed in order. Each effect has a `type`:
- `animation` — play animation on this entity (`anim` field)
- `actor_animation` — play animation on the character doing it (`anim` field)
- `sound` — play sound effect (`asset` field)
- `state_change` — set new state for this entity (`new_state` field)
- `target_state_change` — change another entity's state (`entity` + `new_state` fields)
- `remove_item` — consume item from actor's inventory (`item` field)
- `spawn_item` — give item to actor (`item` field)
- `set_flag` — set a global flag (`flag` field)
- `clear_flag` — clear a global flag (`flag` field)
- `dialogue_prompt` — LLM generates a spoken response (`prompt` field)
- `info_text` — show static text (`text` field)
- `collider_change` — modify entity's collider (`enabled` field, for doors opening)

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
      "_note": "occupied_by is runtime ECS state, not in JSON"
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
  "adjacent_areas": ["courtyard", "sleeping_quarters"],
  "ambient_sound": "tavern_ambience"
}
```

**No static entity list.** At runtime, the system queries all entities whose position falls within `bounds` and all NPCs currently in the area. This is fully dynamic — moving an entity into the area automatically includes it in the context. The context area only defines the region, its description, and adjacency for NPC movement.

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
Choose ONE action. You may also say something while acting.
Respond in this exact format:

SAY: "<optional speech, or empty>"
ACTION: <action_type> <target_id>

Valid actions:
- INTERACT <entity_id> <interaction_id>
- MOVE_TO <entity_id>
- GIVE <target_id> <item_id>
- IDLE

Examples:
SAY: "Let me check this door..."
ACTION: INTERACT door_01 unlock

SAY: ""
ACTION: MOVE_TO table_01

SAY: "Here, take this."
ACTION: GIVE player iron_key
```

### 3.3 Output Parsing

LLM output is parsed into a structured `NpcAction` enum. If parsing fails or action is invalid (conditions not met), NPC defaults to IDLE and optionally comments on the failure ("Locked... who has the key?").

### 3.4 NPC Tick Model

Each NPC does one **area scan tick** per decision cycle:

1. **Scan** — collect everything in current context area (entities, states, NPCs, player)
2. **Build prompt** — persona + memory + scanned context
3. **Query LLM** — get one decision (SAY + ACTION)
4. **Execute** — pathfind, interact, speak
5. **Wait** — cooldown until next tick (action duration + thinking pause)

**Only one NPC acts at a time.** NPCs take turns in a round-robin queue:

- One LLM query at a time — no concurrency issues
- World state is consistent during each decision (no race conditions)
- Player always sees one NPC doing something, then another — feels natural

**Context hash for dirty detection:**

Each NPC stores a `last_context_hash`. On its turn:

1. Build context (area entities, states, NPCs, player)
2. Hash the context
3. Compare to `last_context_hash`
4. **Same** → skip, stay in current state. No LLM query.
5. **Different (dirty)** → full LLM query, update hash

This drastically reduces LLM load. A quiet village with nothing happening = zero queries. Something changes = affected NPCs react on their next turn.

---

## 4. TTS Integration (Chatterbox)

- Runs in a Python subprocess via `chatterbox-venv`
- Game sends text + voice_profile via IPC (stdin/stdout or socket)
- Chatterbox generates WAV, returns path or audio data
- Bevy loads and plays as spatial audio at NPC position
- ~1.2 seconds per sentence, faster than real-time
- 3 GB VRAM on GPU; CPU fallback for low-end systems

---

## 5. Player Interaction UI

When player is near an entity with available interactions:

1. **Single interaction** — show `[E] Open` directly, press E to execute
2. **Multiple interactions** — show list, scroll with mouse wheel or 1/2/3 keys, press E to execute
3. **No available interactions** — show informational text if any (e.g. "Locked")
4. **NPC "Say"** — always available for NPCs. Opens a **text input field**. Player types freely. LLM generates NPC response. Chatterbox speaks it.

The interaction list filters in real-time: only interactions whose conditions are met appear. Others are hidden (not greyed out — invisible).

"Say" is the player's primary tool. The entire game — building alliances, spreading rumors, overthrowing the king — is done through free text conversation.

---

## 6. Pathfinding (renumbered)

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
- Each memory has an **importance score** (assigned by LLM when event happens)
- High importance: permanent ("Player saved my life")
- Low importance: decays and eventually forgotten ("Player walked past")
- Medium: summarized over time by LLM ("Last week: several conversations with the player about the abyss")
- Loaded into context as condensed summary, most important first

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
