//! Interaction system for Hollowreach.
//!
//! Handles condition evaluation, reaction execution, and the Bevy event pipeline
//! for entity interactions. Add to lib.rs with `mod interactions;` and register
//! `InteractionPlugin` in the app.

use bevy::prelude::*;
use std::collections::{HashMap, HashSet};

use crate::inventory::{NpcInventory, PlayerInventory};
use crate::EntityState;

// ---------------------------------------------------------------------------
// Data types — mirror the JSON schema from TECHNICAL_PLAN.md §1.2
// When serde is added to Cargo.toml, add #[derive(Serialize, Deserialize)]
// to each of these.
// ---------------------------------------------------------------------------

/// The kind of condition that gates an interaction's availability.
#[derive(Debug, Clone, PartialEq)]
pub enum ConditionType {
    /// This entity must be in a specific state.
    EntityState,
    /// The actor (player or NPC performing the interaction) must have an item.
    ActorHasItem,
    /// A global flag must be set.
    FlagSet,
    /// A global flag must NOT be set.
    FlagNotSet,
    /// Another entity (by id) must be in a specific state.
    OtherEntityState,
}

/// A single condition on an interaction.
#[derive(Debug, Clone)]
pub struct Condition {
    pub condition_type: ConditionType,
    /// For `EntityState` / `OtherEntityState`: the required state string.
    pub state: Option<String>,
    /// For `ActorHasItem`: the required item id. `"*"` means "any item".
    pub item: Option<String>,
    /// For `FlagSet` / `FlagNotSet`: the flag name.
    pub flag: Option<String>,
    /// For `OtherEntityState`: the entity id to check.
    pub entity: Option<String>,
}

/// The kind of effect that a reaction step produces.
#[derive(Debug, Clone, PartialEq)]
pub enum ReactionEffectType {
    /// Play an animation on the target entity.
    Animation,
    /// Play an animation on the actor (player/NPC performing the interaction).
    ActorAnimation,
    /// Play a sound effect.
    Sound,
    /// Change the target entity's state.
    StateChange,
    /// Change another entity's state (cross-entity effect).
    TargetStateChange,
    /// Remove an item from the actor's inventory.
    RemoveItem,
    /// Give an item to the actor.
    SpawnItem,
    /// Set a global flag.
    SetFlag,
    /// Clear a global flag.
    ClearFlag,
    /// Trigger LLM dialogue generation.
    DialoguePrompt,
    /// Show static informational text.
    InfoText,
    /// Enable or disable the entity's collider (e.g. door opening).
    ColliderChange,
}

/// A single effect within a reaction sequence.
#[derive(Debug, Clone)]
pub struct ReactionEffect {
    pub effect_type: ReactionEffectType,
    /// Animation name (for `Animation` / `ActorAnimation`).
    pub anim: Option<String>,
    /// Sound asset name (for `Sound`).
    pub asset: Option<String>,
    /// New state string (for `StateChange` / `TargetStateChange`).
    pub new_state: Option<String>,
    /// Target entity id (for `TargetStateChange`).
    pub entity: Option<String>,
    /// Item id (for `RemoveItem` / `SpawnItem`).
    pub item: Option<String>,
    /// Flag name (for `SetFlag` / `ClearFlag`).
    pub flag: Option<String>,
    /// LLM prompt (for `DialoguePrompt`).
    pub prompt: Option<String>,
    /// Static text (for `InfoText`).
    pub text: Option<String>,
    /// Whether collider is enabled (for `ColliderChange`).
    pub enabled: Option<bool>,
}

/// A single interaction definition, loaded from entity JSON config.
#[derive(Debug, Clone)]
pub struct Interaction {
    /// Unique id within the entity (e.g. "unlock", "open", "talk").
    pub id: String,
    /// Human-readable label shown in UI (e.g. "Unlock", "Open Door").
    pub label: String,
    /// All conditions must be met for this interaction to be available.
    pub conditions: Vec<Condition>,
    /// Effects executed in order when the interaction is performed.
    pub reaction: Vec<ReactionEffect>,
}

/// Full entity configuration loaded from JSON.
#[derive(Debug, Clone)]
pub struct EntityConfig {
    pub id: String,
    pub entity_type: String,
    pub model: String,
    pub state: String,
    pub interactions: Vec<Interaction>,
}

// ---------------------------------------------------------------------------
// ECS Components and Resources
// ---------------------------------------------------------------------------

/// Global flags resource. Interactions can set/clear flags, and conditions can
/// check them. Shared across all entities.
#[derive(Resource, Debug, Clone, Default)]
pub struct GlobalFlags(pub HashSet<String>);

/// Lookup from entity id string to Bevy Entity, so cross-entity effects
/// (e.g. `TargetStateChange`) can find the right entity.
#[derive(Resource, Debug, Clone, Default)]
pub struct EntityIdMap(pub HashMap<String, Entity>);

// ---------------------------------------------------------------------------
// Events
// ---------------------------------------------------------------------------

/// Fired when an actor triggers an interaction on a target entity.
#[derive(Message, Debug, Clone)]
pub struct InteractionEvent {
    /// The Bevy entity being interacted with.
    pub target: Entity,
    /// The entity id of the target (for logging / cross-entity lookups).
    pub target_id: String,
    /// The actor performing the interaction (player entity or NPC entity).
    pub actor: Entity,
    /// The interaction id that was chosen (e.g. "unlock").
    pub interaction_id: String,
    /// The resolved list of effects to apply. Pre-computed so the effect system
    /// does not need to re-evaluate conditions.
    pub effects: Vec<ReactionEffect>,
}

/// Fired by the effect system when an info text or dialogue prompt should
/// be displayed to the player.
#[derive(Message, Debug, Clone)]
pub struct ShowTextEvent {
    pub text: String,
    /// If true, this is a dialogue prompt that should be forwarded to the LLM.
    pub is_dialogue_prompt: bool,
}

/// Fired when a sound effect should be played.
#[derive(Message, Debug, Clone)]
pub struct PlaySoundEvent {
    pub asset: String,
}

/// Fired when an animation should be played on an entity.
#[derive(Message, Debug, Clone)]
pub struct PlayAnimationEvent {
    pub entity: Entity,
    pub anim: String,
}

// ---------------------------------------------------------------------------
// Condition Evaluation
// ---------------------------------------------------------------------------

/// Evaluate whether ALL conditions for an interaction are met.
///
/// - `entity_state`: current state of the entity being interacted with.
/// - `actor_inventory`: items held by whoever is performing the interaction.
/// - `global_flags`: the shared global flag set.
/// - `all_entity_states`: map from entity id → current state, for `OtherEntityState`.
pub fn evaluate_conditions(
    conditions: &[Condition],
    entity_state: &str,
    actor_inventory: &[String],
    global_flags: &HashSet<String>,
    all_entity_states: Option<&HashMap<String, String>>,
) -> bool {
    conditions.iter().all(|cond| {
        match cond.condition_type {
            ConditionType::EntityState => {
                if let Some(ref required) = cond.state {
                    entity_state == required
                } else {
                    // No state specified means condition is vacuously true.
                    true
                }
            }
            ConditionType::ActorHasItem => {
                if let Some(ref required_item) = cond.item {
                    if required_item == "*" {
                        // Wildcard: actor must have at least one item.
                        !actor_inventory.is_empty()
                    } else {
                        actor_inventory.contains(required_item)
                    }
                } else {
                    true
                }
            }
            ConditionType::FlagSet => {
                if let Some(ref flag) = cond.flag {
                    global_flags.contains(flag)
                } else {
                    true
                }
            }
            ConditionType::FlagNotSet => {
                if let Some(ref flag) = cond.flag {
                    !global_flags.contains(flag)
                } else {
                    true
                }
            }
            ConditionType::OtherEntityState => {
                if let (Some(eid), Some(required_state)) = (&cond.entity, &cond.state) {
                    if let Some(states) = all_entity_states {
                        states.get(eid.as_str()).map_or(false, |s| s == required_state)
                    } else {
                        // No entity state map provided — cannot verify, treat as unmet.
                        false
                    }
                } else {
                    true
                }
            }
        }
    })
}

/// Return all interactions on an entity whose conditions are currently met.
pub fn get_available_interactions(
    interactions: &[Interaction],
    entity_state: &str,
    actor_inventory: &[String],
    global_flags: &HashSet<String>,
    all_entity_states: Option<&HashMap<String, String>>,
) -> Vec<Interaction> {
    interactions
        .iter()
        .filter(|interaction| {
            evaluate_conditions(
                &interaction.conditions,
                entity_state,
                actor_inventory,
                global_flags,
                all_entity_states,
            )
        })
        .cloned()
        .collect()
}

// ---------------------------------------------------------------------------
// Effect execution (pure logic, returns effects to apply)
// ---------------------------------------------------------------------------

/// Validate and return the effects from a reaction. This is intentionally a
/// pass-through right now — all effects in the reaction list are returned.
/// In the future this can filter out effects that don't apply or transform
/// parameterized effects (e.g. substituting `{item}` in dialogue prompts).
pub fn execute_reaction(reaction_effects: &[ReactionEffect]) -> Vec<ReactionEffect> {
    reaction_effects.to_vec()
}

// ---------------------------------------------------------------------------
// Bevy Systems
// ---------------------------------------------------------------------------

/// Processes `InteractionEvent`s and applies each effect to the world.
///
/// This system handles all effect types defined in the technical plan:
/// state changes, flag mutations, inventory changes, and emitting downstream
/// events for sound/animation/text that other systems consume.
pub fn interaction_effect_system(
    mut events: MessageReader<InteractionEvent>,
    mut global_flags: ResMut<GlobalFlags>,
    entity_id_map: Res<EntityIdMap>,
    mut entity_states: Query<&mut EntityState>,
    mut player_inventories: Query<&mut PlayerInventory>,
    mut npc_inventories: Query<&mut NpcInventory>,
    mut text_events: MessageWriter<ShowTextEvent>,
    mut sound_events: MessageWriter<PlaySoundEvent>,
    mut anim_events: MessageWriter<PlayAnimationEvent>,
) {
    for event in events.read() {
        for effect in &event.effects {
            match effect.effect_type {
                ReactionEffectType::StateChange => {
                    if let Some(ref new_state) = effect.new_state {
                        if let Ok(mut state) = entity_states.get_mut(event.target) {
                            state.0 = new_state.clone();
                        }
                    }
                }

                ReactionEffectType::TargetStateChange => {
                    if let (Some(eid), Some(new_state)) =
                        (&effect.entity, &effect.new_state)
                    {
                        if let Some(&target_entity) = entity_id_map.0.get(eid.as_str()) {
                            if let Ok(mut state) = entity_states.get_mut(target_entity) {
                                state.0 = new_state.clone();
                            }
                        }
                    }
                }

                ReactionEffectType::SetFlag => {
                    if let Some(ref flag) = effect.flag {
                        global_flags.0.insert(flag.clone());
                    }
                }

                ReactionEffectType::ClearFlag => {
                    if let Some(ref flag) = effect.flag {
                        global_flags.0.remove(flag.as_str());
                    }
                }

                ReactionEffectType::RemoveItem => {
                    if let Some(ref item) = effect.item {
                        // Try player inventory first, then NPC inventory.
                        if let Ok(mut inv) = player_inventories.get_mut(event.actor) {
                            if let Some(pos) = inv.items.iter().position(|i| i == item) {
                                inv.items.remove(pos);
                            }
                        } else if let Ok(mut inv) = npc_inventories.get_mut(event.actor) {
                            if let Some(pos) = inv.items.iter().position(|i| i == item) {
                                inv.items.remove(pos);
                            }
                        }
                    }
                }

                ReactionEffectType::SpawnItem => {
                    if let Some(ref item) = effect.item {
                        if let Ok(mut inv) = player_inventories.get_mut(event.actor) {
                            inv.items.push(item.clone());
                        } else if let Ok(mut inv) = npc_inventories.get_mut(event.actor) {
                            inv.items.push(item.clone());
                        }
                    }
                }

                ReactionEffectType::Sound => {
                    if let Some(ref asset) = effect.asset {
                        sound_events.write(PlaySoundEvent {
                            asset: asset.clone(),
                        });
                    }
                }

                ReactionEffectType::Animation => {
                    if let Some(ref anim) = effect.anim {
                        anim_events.write(PlayAnimationEvent {
                            entity: event.target,
                            anim: anim.clone(),
                        });
                    }
                }

                ReactionEffectType::ActorAnimation => {
                    if let Some(ref anim) = effect.anim {
                        anim_events.write(PlayAnimationEvent {
                            entity: event.actor,
                            anim: anim.clone(),
                        });
                    }
                }

                ReactionEffectType::InfoText => {
                    if let Some(ref text) = effect.text {
                        text_events.write(ShowTextEvent {
                            text: text.clone(),
                            is_dialogue_prompt: false,
                        });
                    }
                }

                ReactionEffectType::DialoguePrompt => {
                    if let Some(ref prompt) = effect.prompt {
                        text_events.write(ShowTextEvent {
                            text: prompt.clone(),
                            is_dialogue_prompt: true,
                        });
                    }
                }

                ReactionEffectType::ColliderChange => {
                    // Collider enable/disable will be handled when the collider
                    // component system is implemented. For now, log intent.
                    if let Some(enabled) = effect.enabled {
                        info!(
                            "ColliderChange on {:?}: enabled={}",
                            event.target_id, enabled
                        );
                    }
                }
            }
        }
    }
}

/// Builds a snapshot of all entity states keyed by their EntityId.
/// Useful for passing to `evaluate_conditions` for `OtherEntityState` checks.
pub fn build_entity_state_map(
    query: &Query<(&crate::EntityId, &crate::EntityState)>,
) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for (eid, state) in query.iter() {
        map.insert(eid.0.clone(), state.0.clone());
    }
    map
}

// ---------------------------------------------------------------------------
// Conversion from JSON config types (defined in lib.rs) to runtime types
// ---------------------------------------------------------------------------

/// Convert a JSON condition type string to the runtime enum.
pub fn parse_condition_type(s: &str) -> Option<ConditionType> {
    match s {
        "entity_state" => Some(ConditionType::EntityState),
        "actor_has_item" => Some(ConditionType::ActorHasItem),
        "flag_set" => Some(ConditionType::FlagSet),
        "flag_not_set" => Some(ConditionType::FlagNotSet),
        "other_entity_state" => Some(ConditionType::OtherEntityState),
        _ => None,
    }
}

/// Convert a JSON effect type string to the runtime enum.
pub fn parse_effect_type(s: &str) -> Option<ReactionEffectType> {
    match s {
        "animation" => Some(ReactionEffectType::Animation),
        "actor_animation" => Some(ReactionEffectType::ActorAnimation),
        "sound" => Some(ReactionEffectType::Sound),
        "state_change" => Some(ReactionEffectType::StateChange),
        "target_state_change" => Some(ReactionEffectType::TargetStateChange),
        "remove_item" => Some(ReactionEffectType::RemoveItem),
        "spawn_item" => Some(ReactionEffectType::SpawnItem),
        "set_flag" => Some(ReactionEffectType::SetFlag),
        "clear_flag" => Some(ReactionEffectType::ClearFlag),
        "dialogue_prompt" => Some(ReactionEffectType::DialoguePrompt),
        "info_text" => Some(ReactionEffectType::InfoText),
        "collider_change" => Some(ReactionEffectType::ColliderChange),
        _ => None,
    }
}

/// Convert a lib.rs JSON `Condition` to a runtime `Condition`.
pub fn convert_condition(json_cond: &crate::Condition) -> Condition {
    Condition {
        condition_type: parse_condition_type(&json_cond.condition_type)
            .unwrap_or(ConditionType::EntityState),
        state: json_cond.state.clone(),
        item: json_cond.item.clone(),
        flag: json_cond.flag.clone(),
        entity: json_cond.entity.clone(),
    }
}

/// Convert a serde_json::Value reaction array into runtime `ReactionEffect`s.
pub fn convert_reaction(reaction: &serde_json::Value) -> Vec<ReactionEffect> {
    let arr = match reaction.as_array() {
        Some(a) => a,
        None => return Vec::new(),
    };
    arr.iter()
        .filter_map(|v| {
            let type_str = v.get("type")?.as_str()?;
            let effect_type = parse_effect_type(type_str)?;
            Some(ReactionEffect {
                effect_type,
                anim: v.get("anim").and_then(|v| v.as_str()).map(String::from),
                asset: v.get("asset").and_then(|v| v.as_str()).map(String::from),
                new_state: v.get("new_state").and_then(|v| v.as_str()).map(String::from),
                entity: v.get("entity").and_then(|v| v.as_str()).map(String::from),
                item: v.get("item").and_then(|v| v.as_str()).map(String::from),
                flag: v.get("flag").and_then(|v| v.as_str()).map(String::from),
                prompt: v.get("prompt").and_then(|v| v.as_str()).map(String::from),
                text: v.get("text").and_then(|v| v.as_str()).map(String::from),
                enabled: v.get("enabled").and_then(|v| v.as_bool()),
            })
        })
        .collect()
}

/// Convert a lib.rs JSON `Interaction` to a runtime `Interaction`.
pub fn convert_interaction(json_inter: &crate::Interaction) -> Interaction {
    Interaction {
        id: json_inter.id.clone(),
        label: json_inter.label.clone(),
        conditions: json_inter.conditions.iter().map(convert_condition).collect(),
        reaction: convert_reaction(&json_inter.reaction),
    }
}

/// Convert a lib.rs `InteractionList` to a Vec of runtime `Interaction`s.
pub fn convert_interaction_list(list: &crate::InteractionList) -> Vec<Interaction> {
    list.0.iter().map(convert_interaction).collect()
}

// ---------------------------------------------------------------------------
// Selected interaction tracking
// ---------------------------------------------------------------------------

/// Tracks which interaction the player has selected when near an entity with
/// multiple available interactions. Reset when the player moves away.
#[derive(Resource, Debug, Default)]
pub struct SelectedInteraction {
    /// The target entity currently in proximity (if any).
    pub target: Option<Entity>,
    /// Index into the available interactions list.
    pub index: usize,
}

// ---------------------------------------------------------------------------
// Plugin
// ---------------------------------------------------------------------------

/// Registers the interaction events, resources, and systems.
/// Add with `app.add_plugins(InteractionPlugin)`.
pub struct InteractionPlugin;

impl Plugin for InteractionPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GlobalFlags>()
            .init_resource::<EntityIdMap>()
            .init_resource::<SelectedInteraction>()
            .add_message::<InteractionEvent>()
            .add_message::<ShowTextEvent>()
            .add_message::<PlaySoundEvent>()
            .add_message::<PlayAnimationEvent>()
            .add_systems(Update, interaction_effect_system);
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_flag_set() -> HashSet<String> {
        HashSet::new()
    }

    fn make_inventory(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    // -- evaluate_conditions --

    #[test]
    fn empty_conditions_always_pass() {
        assert!(evaluate_conditions(&[], "locked", &[], &make_flag_set(), None));
    }

    #[test]
    fn entity_state_match() {
        let conds = vec![Condition {
            condition_type: ConditionType::EntityState,
            state: Some("locked".into()),
            item: None,
            flag: None,
            entity: None,
        }];
        assert!(evaluate_conditions(&conds, "locked", &[], &make_flag_set(), None));
        assert!(!evaluate_conditions(&conds, "open", &[], &make_flag_set(), None));
    }

    #[test]
    fn actor_has_item_specific() {
        let conds = vec![Condition {
            condition_type: ConditionType::ActorHasItem,
            state: None,
            item: Some("iron_key".into()),
            flag: None,
            entity: None,
        }];
        assert!(evaluate_conditions(
            &conds,
            "locked",
            &make_inventory(&["iron_key"]),
            &make_flag_set(),
            None,
        ));
        assert!(!evaluate_conditions(
            &conds,
            "locked",
            &make_inventory(&["gold_key"]),
            &make_flag_set(),
            None,
        ));
        assert!(!evaluate_conditions(&conds, "locked", &[], &make_flag_set(), None));
    }

    #[test]
    fn actor_has_item_wildcard() {
        let conds = vec![Condition {
            condition_type: ConditionType::ActorHasItem,
            state: None,
            item: Some("*".into()),
            flag: None,
            entity: None,
        }];
        assert!(evaluate_conditions(
            &conds,
            "any",
            &make_inventory(&["bread"]),
            &make_flag_set(),
            None,
        ));
        assert!(!evaluate_conditions(&conds, "any", &[], &make_flag_set(), None));
    }

    #[test]
    fn flag_set_and_not_set() {
        let mut flags = make_flag_set();
        flags.insert("tavern_door_unlocked".into());

        let set_cond = vec![Condition {
            condition_type: ConditionType::FlagSet,
            state: None,
            item: None,
            flag: Some("tavern_door_unlocked".into()),
            entity: None,
        }];
        assert!(evaluate_conditions(&set_cond, "", &[], &flags, None));

        let not_set_cond = vec![Condition {
            condition_type: ConditionType::FlagNotSet,
            state: None,
            item: None,
            flag: Some("tavern_door_unlocked".into()),
            entity: None,
        }];
        assert!(!evaluate_conditions(&not_set_cond, "", &[], &flags, None));

        // Flag not present — FlagNotSet should pass.
        let missing_cond = vec![Condition {
            condition_type: ConditionType::FlagNotSet,
            state: None,
            item: None,
            flag: Some("nonexistent".into()),
            entity: None,
        }];
        assert!(evaluate_conditions(&missing_cond, "", &[], &flags, None));
    }

    #[test]
    fn other_entity_state() {
        let mut states = HashMap::new();
        states.insert("lever_01".into(), "pulled".into());

        let conds = vec![Condition {
            condition_type: ConditionType::OtherEntityState,
            state: Some("pulled".into()),
            item: None,
            flag: None,
            entity: Some("lever_01".into()),
        }];
        assert!(evaluate_conditions(&conds, "", &[], &make_flag_set(), Some(&states)));
        assert!(!evaluate_conditions(&conds, "", &[], &make_flag_set(), None));

        // Wrong state.
        let mut wrong = HashMap::new();
        wrong.insert("lever_01".into(), "default".into());
        assert!(!evaluate_conditions(&conds, "", &[], &make_flag_set(), Some(&wrong)));
    }

    #[test]
    fn multiple_conditions_all_must_pass() {
        let conds = vec![
            Condition {
                condition_type: ConditionType::EntityState,
                state: Some("locked".into()),
                item: None,
                flag: None,
                entity: None,
            },
            Condition {
                condition_type: ConditionType::ActorHasItem,
                state: None,
                item: Some("iron_key".into()),
                flag: None,
                entity: None,
            },
        ];
        // Both met.
        assert!(evaluate_conditions(
            &conds,
            "locked",
            &make_inventory(&["iron_key"]),
            &make_flag_set(),
            None,
        ));
        // State met but no item.
        assert!(!evaluate_conditions(
            &conds,
            "locked",
            &[],
            &make_flag_set(),
            None,
        ));
        // Item met but wrong state.
        assert!(!evaluate_conditions(
            &conds,
            "open",
            &make_inventory(&["iron_key"]),
            &make_flag_set(),
            None,
        ));
    }

    // -- get_available_interactions --

    #[test]
    fn filters_interactions_by_conditions() {
        let interactions = vec![
            Interaction {
                id: "examine".into(),
                label: "Examine".into(),
                conditions: vec![],
                reaction: vec![],
            },
            Interaction {
                id: "unlock".into(),
                label: "Unlock".into(),
                conditions: vec![
                    Condition {
                        condition_type: ConditionType::EntityState,
                        state: Some("locked".into()),
                        item: None,
                        flag: None,
                        entity: None,
                    },
                    Condition {
                        condition_type: ConditionType::ActorHasItem,
                        state: None,
                        item: Some("iron_key".into()),
                        flag: None,
                        entity: None,
                    },
                ],
                reaction: vec![],
            },
            Interaction {
                id: "open".into(),
                label: "Open".into(),
                conditions: vec![Condition {
                    condition_type: ConditionType::EntityState,
                    state: Some("unlocked".into()),
                    item: None,
                    flag: None,
                    entity: None,
                }],
                reaction: vec![],
            },
        ];

        // Door is locked, player has no key — only "examine" available.
        let available = get_available_interactions(
            &interactions,
            "locked",
            &[],
            &make_flag_set(),
            None,
        );
        assert_eq!(available.len(), 1);
        assert_eq!(available[0].id, "examine");

        // Door is locked, player has key — "examine" and "unlock".
        let available = get_available_interactions(
            &interactions,
            "locked",
            &make_inventory(&["iron_key"]),
            &make_flag_set(),
            None,
        );
        assert_eq!(available.len(), 2);
        assert!(available.iter().any(|i| i.id == "examine"));
        assert!(available.iter().any(|i| i.id == "unlock"));

        // Door is unlocked — "examine" and "open".
        let available = get_available_interactions(
            &interactions,
            "unlocked",
            &[],
            &make_flag_set(),
            None,
        );
        assert_eq!(available.len(), 2);
        assert!(available.iter().any(|i| i.id == "examine"));
        assert!(available.iter().any(|i| i.id == "open"));
    }

    // -- execute_reaction --

    #[test]
    fn execute_reaction_returns_all_effects() {
        let effects = vec![
            ReactionEffect {
                effect_type: ReactionEffectType::Sound,
                anim: None,
                asset: Some("lock_click".into()),
                new_state: None,
                entity: None,
                item: None,
                flag: None,
                prompt: None,
                text: None,
                enabled: None,
            },
            ReactionEffect {
                effect_type: ReactionEffectType::StateChange,
                anim: None,
                asset: None,
                new_state: Some("unlocked".into()),
                entity: None,
                item: None,
                flag: None,
                prompt: None,
                text: None,
                enabled: None,
            },
        ];
        let result = execute_reaction(&effects);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].effect_type, ReactionEffectType::Sound);
        assert_eq!(result[1].effect_type, ReactionEffectType::StateChange);
    }
}
