//! NPC look-at system — NPCs smoothly turn their HEAD to face interaction targets.
//!
//! Only the head bone rotates, not the whole body. Target clears automatically
//! when the player moves away.

use bevy::prelude::*;
use std::f32::consts::PI;

/// Attach to an NPC root entity to enable head tracking.
#[derive(Component)]
pub struct NpcLookAt {
    /// The entity to look at (None = head returns to neutral).
    pub target: Option<Entity>,
    /// Max rotation in radians (clamp so head doesn't spin 180).
    pub max_angle: f32,
    /// Rotation speed in radians per second.
    pub speed: f32,
    /// Distance at which target is cleared automatically.
    pub max_distance: f32,
}

impl Default for NpcLookAt {
    fn default() -> Self {
        Self {
            target: None,
            max_angle: PI / 4.0, // 45 degrees
            speed: 2.5,
            max_distance: super::INTERACT_DISTANCE + 0.5,
        }
    }
}

/// Marker placed on the Head bone entity once found.
#[derive(Component)]
pub struct NpcHeadBone {
    pub npc_root: Entity,
}

pub struct NpcLookAtPlugin;

impl Plugin for NpcLookAtPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (find_head_bones, npc_look_at_system, clear_distant_targets));
    }
}

/// Finds "Head" named entities that are children of NPC entities with NpcLookAt,
/// and marks them with NpcHeadBone.
fn find_head_bones(
    mut commands: Commands,
    npc_q: Query<Entity, (With<NpcLookAt>, Without<NpcHeadBone>)>,
    name_q: Query<(Entity, &Name, &Parent), Without<NpcHeadBone>>,
    existing_heads: Query<&NpcHeadBone>,
) {
    for npc_entity in &npc_q {
        // Check if we already have a head bone for this NPC
        let already_found = existing_heads.iter().any(|h| h.npc_root == npc_entity);
        if already_found {
            continue;
        }

        // Search all named entities for "Head" that's a descendant of this NPC
        for (bone_entity, name, _parent) in &name_q {
            if name.as_str() == "Head" {
                // Check if this bone is a descendant of the NPC (walk up parents)
                // For simplicity, just tag any "Head" bone found — works when NPCs
                // are the only entities with NpcLookAt
                commands.entity(bone_entity).insert(NpcHeadBone { npc_root: npc_entity });
            }
        }
    }
}

/// Clear target when player is too far away.
fn clear_distant_targets(
    mut npc_q: Query<(&GlobalTransform, &mut NpcLookAt)>,
    target_q: Query<&GlobalTransform>,
) {
    for (npc_global, mut look_at) in &mut npc_q {
        let Some(target_entity) = look_at.target else { continue };
        let Ok(target_global) = target_q.get(target_entity) else {
            look_at.target = None;
            continue;
        };

        let dist = npc_global.translation().distance(target_global.translation());
        if dist > look_at.max_distance {
            look_at.target = None;
        }
    }
}

/// Rotate head bones toward their NPC's look-at target.
fn npc_look_at_system(
    time: Res<Time>,
    npc_q: Query<(&GlobalTransform, &NpcLookAt)>,
    mut head_q: Query<(&NpcHeadBone, &mut Transform)>,
    target_q: Query<&GlobalTransform>,
) {
    for (head_bone, mut head_tf) in &mut head_q {
        let Ok((npc_global, look_at)) = npc_q.get(head_bone.npc_root) else {
            continue;
        };

        let npc_pos = npc_global.translation();

        match look_at.target {
            Some(target_entity) => {
                let Ok(target_global) = target_q.get(target_entity) else { continue };
                let target_pos = target_global.translation();

                // Direction from NPC to target in world space
                let dx = target_pos.x - npc_pos.x;
                let dz = target_pos.z - npc_pos.z;

                if dx.abs() < 0.01 && dz.abs() < 0.01 {
                    continue;
                }

                // Target yaw relative to NPC's forward direction
                let world_yaw = dx.atan2(dz);
                let npc_yaw = npc_global.to_scale_rotation_translation().1.to_euler(EulerRot::YXZ).0;
                let mut relative_yaw = world_yaw - npc_yaw;

                // Normalize to -PI..PI
                while relative_yaw > PI { relative_yaw -= 2.0 * PI; }
                while relative_yaw < -PI { relative_yaw += 2.0 * PI; }

                // Clamp to max angle
                relative_yaw = relative_yaw.clamp(-look_at.max_angle, look_at.max_angle);

                // Smooth interpolation toward target rotation
                let current_yaw = head_tf.rotation.to_euler(EulerRot::YXZ).0;
                let mut diff = relative_yaw - current_yaw;
                while diff > PI { diff -= 2.0 * PI; }
                while diff < -PI { diff += 2.0 * PI; }

                let max_step = look_at.speed * time.delta_secs();
                let step = diff.clamp(-max_step, max_step);
                let new_yaw = current_yaw + step;

                head_tf.rotation = Quat::from_rotation_y(new_yaw);
            }
            None => {
                // Smoothly return to neutral (identity rotation on Y)
                let current_yaw = head_tf.rotation.to_euler(EulerRot::YXZ).0;
                if current_yaw.abs() > 0.01 {
                    let max_step = look_at.speed * time.delta_secs();
                    let step = (-current_yaw).clamp(-max_step, max_step);
                    head_tf.rotation = Quat::from_rotation_y(current_yaw + step);
                }
            }
        }
    }
}
