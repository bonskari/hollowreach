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

#[derive(Component)]
pub struct DebugLlmText;

/// Latest LLM prompt/context stats, updated each inference.
/// Uses atomics so the LLM worker thread can update without locking.
use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Resource, Default)]
pub struct LlmStats {
    pub last_prompt_tokens: AtomicUsize,
    pub last_ctx_tokens: AtomicUsize,
    pub total_requests: AtomicUsize,
    pub total_rejections: AtomicUsize,
}

/// Global handle shared with the LLM worker thread.
pub static LLM_STATS: std::sync::OnceLock<std::sync::Arc<LlmStats>> = std::sync::OnceLock::new();

pub fn llm_stats() -> &'static std::sync::Arc<LlmStats> {
    LLM_STATS.get_or_init(|| std::sync::Arc::new(LlmStats::default()))
}

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
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
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
                font.clone(),
                color.clone(),
            ));
            parent.spawn((
                DebugLlmText,
                Text::new("LLM: --"),
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
    mut fps_q: Query<&mut Text, (With<DebugFpsText>, Without<DebugVramText>, Without<DebugRamText>, Without<DebugLlmText>)>,
    mut vram_q: Query<&mut Text, (With<DebugVramText>, Without<DebugFpsText>, Without<DebugRamText>, Without<DebugLlmText>)>,
    mut ram_q: Query<&mut Text, (With<DebugRamText>, Without<DebugFpsText>, Without<DebugVramText>, Without<DebugLlmText>)>,
    mut llm_q: Query<&mut Text, (With<DebugLlmText>, Without<DebugFpsText>, Without<DebugVramText>, Without<DebugRamText>)>,
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
                let mut text = fps_q.single_mut().unwrap();
                **text = format!("FPS: {:.0}", fps_val);
            }
        }
        return;
    }

    // --- FPS ---
    if let Some(fps_diag) = diagnostics.get(&bevy::diagnostic::FrameTimeDiagnosticsPlugin::FPS) {
        if let Some(fps_val) = fps_diag.smoothed() {
            let mut text = fps_q.single_mut().unwrap();
            **text = format!("FPS: {:.0}", fps_val);
        }
    }

    // --- RAM from /proc/meminfo ---
    {
        let mut text = ram_q.single_mut().unwrap();
        **text = read_ram_info();
    }

    // --- VRAM from nvidia-smi ---
    {
        let mut text = vram_q.single_mut().unwrap();
        **text = read_vram_info();
    }

    // --- LLM stats ---
    {
        let stats = llm_stats();
        let mut text = llm_q.single_mut().unwrap();
        let total = stats.total_requests.load(Ordering::Relaxed);
        if total == 0 {
            **text = "LLM: idle (no requests)".to_string();
        } else {
            **text = format!(
                "LLM: prompt={}/{} tok (req {} / rej {})",
                stats.last_prompt_tokens.load(Ordering::Relaxed),
                stats.last_ctx_tokens.load(Ordering::Relaxed),
                total,
                stats.total_rejections.load(Ordering::Relaxed),
            );
        }
    }
}

// --- Helpers ---

fn read_ram_info() -> String {
    // Read this process's RSS from /proc/self/status
    let Ok(status) = fs::read_to_string("/proc/self/status") else {
        return "RAM: N/A".to_string();
    };

    for line in status.lines() {
        if let Some(val) = line.strip_prefix("VmRSS:") {
            let rss_kb = val.trim().trim_end_matches(" kB").trim().parse::<u64>().unwrap_or(0);
            let rss_mb = rss_kb / 1024;
            return format!("RAM: {} MB", rss_mb);
        }
    }

    "RAM: N/A".to_string()
}

fn read_vram_info() -> String {
    // Get this process's GPU memory usage via nvidia-smi
    let pid = std::process::id();
    let output = Command::new("nvidia-smi")
        .args(["--query-compute-apps=pid,used_gpu_memory", "--format=csv,noheader,nounits"])
        .output();

    match output {
        Ok(out) if out.status.success() => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            for line in stdout.lines() {
                let parts: Vec<&str> = line.split(',').map(|s| s.trim()).collect();
                if parts.len() == 2 {
                    if let Ok(p) = parts[0].parse::<u32>() {
                        if p == pid {
                            return format!("VRAM: {} MB", parts[1]);
                        }
                    }
                }
            }
            // Process not in compute apps, try graphics apps
            let output2 = Command::new("nvidia-smi")
                .args(["pmon", "-c", "1", "-s", "m"])
                .output();
            if let Ok(out2) = output2 {
                let stdout2 = String::from_utf8_lossy(&out2.stdout);
                for line in stdout2.lines() {
                    if line.contains(&pid.to_string()) {
                        let parts: Vec<&str> = line.split_whitespace().collect();
                        if parts.len() >= 4 {
                            return format!("VRAM: {} MB", parts[3]);
                        }
                    }
                }
            }
            "VRAM: N/A".to_string()
        }
        _ => "VRAM: N/A".to_string(),
    }
}
