//! NPC look-at system — NPCs smoothly turn to face their interaction target.
//!
//! When an NPC interacts with something (player talks, NPC uses entity),
//! they smoothly rotate to face the target. When idle, they face their default direction.

use bevy::prelude::*;
use std::f32::consts::PI;

/// Attach to an NPC to make them smoothly look at a target.
#[derive(Component)]
pub struct NpcLookAt {
    /// The entity to look at (None = face default direction).
    pub target: Option<Entity>,
    /// Rotation speed in radians per second.
    pub speed: f32,
}

impl Default for NpcLookAt {
    fn default() -> Self {
        Self {
            target: None,
            speed: 3.0,
        }
    }
}

pub struct NpcLookAtPlugin;

impl Plugin for NpcLookAtPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, npc_look_at_system);
    }
}

fn npc_look_at_system(
    time: Res<Time>,
    mut npc_q: Query<(&mut Transform, &NpcLookAt), Without<super::Player>>,
    target_q: Query<&GlobalTransform>,
) {
    for (mut npc_tf, look_at) in &mut npc_q {
        let Some(target_entity) = look_at.target else {
            continue;
        };

        let Ok(target_global) = target_q.get(target_entity) else {
            continue;
        };

        let npc_pos = npc_tf.translation;
        let target_pos = target_global.translation();

        // Only rotate on XZ plane (don't tilt up/down)
        let dx = target_pos.x - npc_pos.x;
        let dz = target_pos.z - npc_pos.z;

        if dx.abs() < 0.01 && dz.abs() < 0.01 {
            continue; // Too close, skip
        }

        let target_yaw = dx.atan2(dz);
        let current_yaw = npc_tf.rotation.to_euler(EulerRot::YXZ).0;

        // Shortest angle difference
        let mut diff = target_yaw - current_yaw;
        if diff > PI {
            diff -= 2.0 * PI;
        }
        if diff < -PI {
            diff += 2.0 * PI;
        }

        // Smooth interpolation
        let max_step = look_at.speed * time.delta_secs();
        let step = diff.clamp(-max_step, max_step);
        let new_yaw = current_yaw + step;

        npc_tf.rotation = Quat::from_rotation_y(new_yaw);
    }
}
