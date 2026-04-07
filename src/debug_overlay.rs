use bevy::diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin};
use bevy::prelude::*;
use std::fs;
use std::process::Command;

// --- Components & Resources ---

#[derive(Component)]
pub struct DebugOverlay;

#[derive(Component)]
pub struct DebugFpsText;

#[derive(Component)]
pub struct DebugVramText;

#[derive(Component)]
pub struct DebugRamText;

#[derive(Resource)]
pub struct DebugOverlayState {
    pub visible: bool,
    pub refresh_timer: Timer,
}

impl Default for DebugOverlayState {
    fn default() -> Self {
        Self {
            visible: false,
            refresh_timer: Timer::from_seconds(0.5, TimerMode::Repeating),
        }
    }
}

// --- Plugin ---

pub struct DebugOverlayPlugin;

impl Plugin for DebugOverlayPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(FrameTimeDiagnosticsPlugin::default())
            .init_resource::<DebugOverlayState>()
            .add_systems(Startup, setup_debug_overlay)
            .add_systems(Update, (toggle_debug_overlay, update_debug_overlay));
    }
}

// --- Systems ---

pub fn setup_debug_overlay(mut commands: Commands) {
    commands
        .spawn((
            DebugOverlay,
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(8.0),
                left: Val::Px(8.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(6.0)),
                row_gap: Val::Px(2.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
            BorderRadius::all(Val::Px(4.0)),
            Visibility::Hidden,
            GlobalZIndex(100),
        ))
        .with_children(|parent| {
            let font = TextFont {
                font_size: 13.0,
                ..default()
            };
            let color = TextColor(Color::srgba(0.8, 0.9, 0.8, 0.9));

            parent.spawn((
                DebugFpsText,
                Text::new("FPS: --"),
                font.clone(),
                color.clone(),
            ));
            parent.spawn((
                DebugVramText,
                Text::new("VRAM: --"),
                font.clone(),
                color.clone(),
            ));
            parent.spawn((
                DebugRamText,
                Text::new("RAM: --"),
                font,
                color,
            ));
        });
}

pub fn toggle_debug_overlay(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut state: ResMut<DebugOverlayState>,
    mut query: Query<&mut Visibility, With<DebugOverlay>>,
) {
    if keyboard.just_pressed(KeyCode::Tab) {
        state.visible = !state.visible;
        for mut vis in &mut query {
            *vis = if state.visible {
                Visibility::Visible
            } else {
                Visibility::Hidden
            };
        }
    }
}

pub fn update_debug_overlay(
    time: Res<Time>,
    diagnostics: Res<DiagnosticsStore>,
    mut state: ResMut<DebugOverlayState>,
    mut fps_q: Query<&mut Text, (With<DebugFpsText>, Without<DebugVramText>, Without<DebugRamText>)>,
    mut vram_q: Query<&mut Text, (With<DebugVramText>, Without<DebugFpsText>, Without<DebugRamText>)>,
    mut ram_q: Query<&mut Text, (With<DebugRamText>, Without<DebugFpsText>, Without<DebugVramText>)>,
) {
    if !state.visible {
        return;
    }

    state.refresh_timer.tick(time.delta());
    if !state.refresh_timer.just_finished() {
        // Always update FPS (cheap)
        if let Some(fps_diag) = diagnostics.get(&bevy::diagnostic::FrameTimeDiagnosticsPlugin::FPS)
        {
            if let Some(fps_val) = fps_diag.smoothed() {
                let mut text = fps_q.single_mut();
                **text = format!("FPS: {:.0}", fps_val);
            }
        }
        return;
    }

    // --- FPS ---
    if let Some(fps_diag) = diagnostics.get(&bevy::diagnostic::FrameTimeDiagnosticsPlugin::FPS) {
        if let Some(fps_val) = fps_diag.smoothed() {
            let mut text = fps_q.single_mut();
            **text = format!("FPS: {:.0}", fps_val);
        }
    }

    // --- RAM from /proc/meminfo ---
    {
        let mut text = ram_q.single_mut();
        **text = read_ram_info();
    }

    // --- VRAM from nvidia-smi ---
    {
        let mut text = vram_q.single_mut();
        **text = read_vram_info();
    }
}

// --- Helpers ---

fn read_ram_info() -> String {
    let Ok(meminfo) = fs::read_to_string("/proc/meminfo") else {
        return "RAM: N/A".to_string();
    };

    let mut total_kb: u64 = 0;
    let mut available_kb: u64 = 0;

    for line in meminfo.lines() {
        if let Some(val) = line.strip_prefix("MemTotal:") {
            total_kb = parse_meminfo_kb(val);
        } else if let Some(val) = line.strip_prefix("MemAvailable:") {
            available_kb = parse_meminfo_kb(val);
        }
    }

    if total_kb == 0 {
        return "RAM: N/A".to_string();
    }

    let used_mb = (total_kb - available_kb) / 1024;
    let total_mb = total_kb / 1024;
    format!("RAM: {} / {} MB", used_mb, total_mb)
}

fn parse_meminfo_kb(val: &str) -> u64 {
    val.trim()
        .trim_end_matches("kB")
        .trim()
        .parse::<u64>()
        .unwrap_or(0)
}

fn read_vram_info() -> String {
    let output = Command::new("nvidia-smi")
        .args([
            "--query-gpu=memory.used,memory.total",
            "--format=csv,noheader,nounits",
        ])
        .output();

    match output {
        Ok(out) if out.status.success() => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            let line = stdout.trim();
            let parts: Vec<&str> = line.split(',').map(|s| s.trim()).collect();
            if parts.len() == 2 {
                format!("VRAM: {} / {} MB", parts[0], parts[1])
            } else {
                "VRAM: N/A".to_string()
            }
        }
        _ => "VRAM: N/A".to_string(),
    }
}
