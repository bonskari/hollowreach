use bevy::prelude::*;

// ---------------------------------------------------------------------------
// Components
// ---------------------------------------------------------------------------

/// Items the player is currently carrying (list of item ID strings).
#[derive(Component, Default, Debug, Clone)]
pub struct PlayerInventory {
    pub items: Vec<String>,
}

impl PlayerInventory {
    pub fn has(&self, item_id: &str) -> bool {
        self.items.iter().any(|i| i == item_id)
    }

    pub fn add(&mut self, item_id: String) {
        self.items.push(item_id);
    }

    pub fn remove(&mut self, item_id: &str) -> bool {
        if let Some(pos) = self.items.iter().position(|i| i == item_id) {
            self.items.remove(pos);
            true
        } else {
            false
        }
    }
}

/// Items an NPC is currently carrying (list of item ID strings).
#[derive(Component, Default, Debug, Clone)]
pub struct NpcInventory {
    pub items: Vec<String>,
}

impl NpcInventory {
    pub fn has(&self, item_id: &str) -> bool {
        self.items.iter().any(|i| i == item_id)
    }

    pub fn add(&mut self, item_id: String) {
        self.items.push(item_id);
    }

    pub fn remove(&mut self, item_id: &str) -> bool {
        if let Some(pos) = self.items.iter().position(|i| i == item_id) {
            self.items.remove(pos);
            true
        } else {
            false
        }
    }
}

// ---------------------------------------------------------------------------
// Events
// ---------------------------------------------------------------------------

/// Player picks up an item from the world.
///
/// `entity_id` — the ECS entity string identifier of the world item being picked up.
/// `item_id`   — the inventory string ID to add to the player's inventory.
#[derive(Event, Debug, Clone)]
pub struct PickUpEvent {
    pub entity_id: String,
    pub item_id: String,
}

/// Transfer an item from one actor's inventory to another.
///
/// Both `from` and `to` must have either a `PlayerInventory` or `NpcInventory`.
#[derive(Event, Debug, Clone)]
pub struct GiveItemEvent {
    pub from: Entity,
    pub to: Entity,
    pub item_id: String,
}

// ---------------------------------------------------------------------------
// UI marker
// ---------------------------------------------------------------------------

/// Marker component for the inventory UI root node (top-right corner).
#[derive(Component)]
pub struct InventoryUI;

/// Marker for the text node inside the inventory UI that lists items.
#[derive(Component)]
pub struct InventoryUIText;

// ---------------------------------------------------------------------------
// Systems
// ---------------------------------------------------------------------------

/// Handles `PickUpEvent`: removes the item entity from the world and adds the
/// item ID to the player's inventory.
///
/// Finds the world entity whose `Name` component matches `entity_id`, despawns
/// it, then pushes `item_id` into the player's `PlayerInventory`.
pub fn inventory_pickup_system(
    mut commands: Commands,
    mut events: EventReader<PickUpEvent>,
    mut player_q: Query<&mut PlayerInventory>,
    names_q: Query<(Entity, &Name)>,
) {
    for ev in events.read() {
        // Find the world entity by name and despawn it.
        let mut found = false;
        for (entity, name) in &names_q {
            if name.as_str() == ev.entity_id {
                commands.entity(entity).despawn();
                found = true;
                break;
            }
        }

        if !found {
            warn!(
                "PickUpEvent: could not find world entity with name '{}'",
                ev.entity_id
            );
        }

        // Add item to the player's inventory.
        let mut inv = player_q.single_mut();
        inv.add(ev.item_id.clone());
        info!("Player picked up '{}'", ev.item_id);
    }
}

/// Handles `GiveItemEvent`: moves an item from one actor's inventory to another.
///
/// Both actors may have either `PlayerInventory` or `NpcInventory`. The system
/// tries player inventories first, then NPC inventories.
pub fn inventory_give_system(
    mut events: EventReader<GiveItemEvent>,
    mut player_q: Query<&mut PlayerInventory>,
    mut npc_q: Query<&mut NpcInventory>,
) {
    for ev in events.read() {
        // --- Remove from sender ---
        let removed = if let Ok(mut inv) = player_q.get_mut(ev.from) {
            inv.remove(&ev.item_id)
        } else if let Ok(mut inv) = npc_q.get_mut(ev.from) {
            inv.remove(&ev.item_id)
        } else {
            warn!(
                "GiveItemEvent: sender {:?} has no inventory",
                ev.from
            );
            false
        };

        if !removed {
            warn!(
                "GiveItemEvent: sender {:?} does not have item '{}'",
                ev.from, ev.item_id
            );
            continue;
        }

        // --- Add to receiver ---
        if let Ok(mut inv) = player_q.get_mut(ev.to) {
            inv.add(ev.item_id.clone());
        } else if let Ok(mut inv) = npc_q.get_mut(ev.to) {
            inv.add(ev.item_id.clone());
        } else {
            warn!(
                "GiveItemEvent: receiver {:?} has no inventory",
                ev.to
            );
            // Put the item back in the sender so it isn't lost.
            if let Ok(mut inv) = player_q.get_mut(ev.from) {
                inv.add(ev.item_id.clone());
            } else if let Ok(mut inv) = npc_q.get_mut(ev.from) {
                inv.add(ev.item_id.clone());
            }
            continue;
        }

        info!(
            "Item '{}' transferred from {:?} to {:?}",
            ev.item_id, ev.from, ev.to
        );
    }
}

/// Spawns the inventory UI panel in the top-right corner.
pub fn setup_inventory_ui(mut commands: Commands) {
    commands
        .spawn((
            InventoryUI,
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(16.0),
                right: Val::Px(16.0),
                min_width: Val::Px(160.0),
                padding: UiRect::all(Val::Px(10.0)),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::FlexStart,
                ..default()
            },
            BackgroundColor(Color::srgba(0.06, 0.05, 0.1, 0.75)),
        ))
        .with_children(|parent| {
            // Title
            parent.spawn((
                Text::new("Inventory"),
                TextFont {
                    font_size: 16.0,
                    ..default()
                },
                TextColor(Color::srgb(0.95, 0.82, 0.4)),
            ));

            // Item list (updated every frame by inventory_ui_update_system)
            parent.spawn((
                InventoryUIText,
                Text::new("(empty)"),
                TextFont {
                    font_size: 14.0,
                    ..default()
                },
                TextColor(Color::srgba(0.9, 0.9, 0.9, 0.9)),
                Node {
                    margin: UiRect::top(Val::Px(6.0)),
                    ..default()
                },
            ));
        });
}

/// Updates the inventory UI text to reflect the player's current items.
pub fn inventory_ui_update_system(
    player_q: Query<&PlayerInventory>,
    mut text_q: Query<&mut Text, With<InventoryUIText>>,
) {
    let inv = player_q.single();
    let mut text = text_q.single_mut();

    if inv.items.is_empty() {
        **text = "(empty)".to_string();
    } else {
        let listing: Vec<String> = inv
            .items
            .iter()
            .map(|id| format!("- {}", id))
            .collect();
        **text = listing.join("\n");
    }
}

// ---------------------------------------------------------------------------
// Plugin
// ---------------------------------------------------------------------------

/// Registers inventory components, events, and systems.
pub struct InventoryPlugin;

impl Plugin for InventoryPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<PickUpEvent>()
            .add_event::<GiveItemEvent>()
            .add_systems(Startup, setup_inventory_ui)
            .add_systems(
                Update,
                (
                    inventory_pickup_system,
                    inventory_give_system,
                    inventory_ui_update_system,
                ),
            );
    }
}
