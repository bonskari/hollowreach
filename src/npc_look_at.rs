//! NPC look-at system — NPCs smoothly turn their HEAD to face interaction targets.
//!
//! Runs in PostUpdate so the look-at rotation is applied AFTER the animation
//! system has written the bone's animated pose.  The look-at is an additive
//! Y-rotation multiplied on top of the animated rotation, so it blends
//! naturally with idle/walk animations.

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

/// Stores the current additive yaw offset applied on top of the animated pose.
/// This lets us smoothly interpolate the offset each frame rather than
/// recalculating from the bone's (animation-written) rotation.
#[derive(Component)]
pub struct HeadLookAtOffset {
    pub yaw: f32,
}

pub struct NpcLookAtPlugin;

impl Plugin for NpcLookAtPlugin {
    fn build(&self, app: &mut App) {
        // find_head_bones and clear_distant_targets run in Update (fine, no ordering issue)
        app.add_systems(Update, (find_head_bones, clear_distant_targets).run_if(in_state(crate::GameState::Playing)));
        app.add_systems(PostUpdate, npc_look_at_system.run_if(in_state(crate::GameState::Playing)));
    }
}

/// Finds "head" named entities and walks up the Parent hierarchy to find which
/// NPC (entity with NpcLookAt) they belong to, then marks them with NpcHeadBone.
fn find_head_bones(
    mut commands: Commands,
    npc_q: Query<(), With<NpcLookAt>>,
    name_q: Query<(Entity, &Name), (Without<NpcHeadBone>, Without<NpcLookAt>)>,
    parent_q: Query<&ChildOf>,
    existing_heads: Query<&NpcHeadBone>,
) {
    for (bone_entity, name) in &name_q {
        if name.as_str() != "head" {
            continue;
        }

        // Walk up the Parent chain to find the NPC root entity
        let mut ancestor = bone_entity;
        let npc_root = loop {
            let Ok(parent) = parent_q.get(ancestor) else {
                break None;
            };
            ancestor = parent.parent();
            if npc_q.get(ancestor).is_ok() {
                break Some(ancestor);
            }
        };

        let Some(npc_entity) = npc_root else { continue };

        // Skip if this NPC already has a head bone assigned
        let already_found = existing_heads.iter().any(|h| h.npc_root == npc_entity);
        if already_found {
            continue;
        }

        commands.entity(bone_entity).insert((
            NpcHeadBone { npc_root: npc_entity },
            HeadLookAtOffset { yaw: 0.0 },
        ));
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

/// Additive head rotation applied AFTER animations.
///
/// Instead of setting an absolute rotation (which the animation would
/// overwrite), we:
///   1. Read the NPC's look-at target and compute the desired yaw offset.
///   2. Smoothly interpolate `HeadLookAtOffset::yaw` toward that desired offset.
///   3. Multiply the offset rotation onto whatever rotation the animation
///      system has already written into the bone's `Transform`.
fn npc_look_at_system(
    time: Res<Time>,
    npc_q: Query<(&GlobalTransform, &NpcLookAt)>,
    mut head_q: Query<(&NpcHeadBone, &mut Transform, &mut HeadLookAtOffset)>,
    target_q: Query<&GlobalTransform>,
) {
    for (head_bone, mut head_tf, mut offset) in &mut head_q {
        let Ok((npc_global, look_at)) = npc_q.get(head_bone.npc_root) else {
            continue;
        };

        let npc_pos = npc_global.translation();

        // Compute desired yaw offset
        let desired_yaw = if let Some(target_entity) = look_at.target {
            if let Ok(target_global) = target_q.get(target_entity) {
                let target_pos = target_global.translation();

                let dx = target_pos.x - npc_pos.x;
                let dz = target_pos.z - npc_pos.z;

                if dx.abs() < 0.01 && dz.abs() < 0.01 {
                    0.0
                } else {
                    // Target yaw relative to NPC's forward direction
                    let world_yaw = dx.atan2(dz);
                    let npc_yaw = npc_global
                        .to_scale_rotation_translation()
                        .1
                        .to_euler(EulerRot::YXZ)
                        .0;
                    let mut relative_yaw = world_yaw - npc_yaw;

                    // Normalize to -PI..PI
                    while relative_yaw > PI {
                        relative_yaw -= 2.0 * PI;
                    }
                    while relative_yaw < -PI {
                        relative_yaw += 2.0 * PI;
                    }

                    // Clamp to max angle
                    relative_yaw.clamp(-look_at.max_angle, look_at.max_angle)
                }
            } else {
                // Target entity gone — drift back to neutral
                0.0
            }
        } else {
            0.0
        };

        // Smooth interpolation of the offset
        let mut diff = desired_yaw - offset.yaw;
        while diff > PI {
            diff -= 2.0 * PI;
        }
        while diff < -PI {
            diff += 2.0 * PI;
        }

        let max_step = look_at.speed * time.delta_secs();
        let step = diff.clamp(-max_step, max_step);
        offset.yaw += step;

        // Apply as additive rotation on top of the animated pose.
        // The animation system has already written the bone's transform,
        // so we multiply our offset onto it.
        if offset.yaw.abs() > 0.001 {
            let additive = Quat::from_rotation_y(offset.yaw);
            head_tf.rotation = head_tf.rotation * additive;
        }
    }
}
