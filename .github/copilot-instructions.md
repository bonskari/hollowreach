# Hollowreach — System Design Rules

## No Workarounds

Never use fallback, placeholder, or hack alternatives. If the intended approach doesn't work, fix the root cause. A workaround that behaves differently from the original intent is always wrong.

## Code Sharing

All panels must use the **same shared code**. No duplicating panel logic per panel type. If a behavior applies to all panels (animation, styling, layout pattern), it must be implemented once and reused.

- Panel images: `panel_image_node()` from `ui_helpers.rs`
- Buttons: `spawn_button()` from `ui_helpers.rs`
- Colors: constants from `ui_helpers.rs` (`COLOR_GOLD`, `COLOR_GREY`, `COLOR_BODY`, etc.)
- Panel appear animation: `panel_appear_animation_system` + `UiScaleIn`
- Dialogue slide-in: `UiSlideIn`

If you add a new panel, it must use these shared building blocks. If you add a new feature to panels, add it to the shared system so all panels get it.

## Panel Architecture

Every interaction panel follows this structure:

1. **Outer wrapper** — `PositionType::Absolute`, full-width (`left: 0, right: 0`), used only for centering. Has the panel marker component (`NpcInteractionPanel`, `PropInteractionPanel`, etc.) and `Visibility::Hidden`.
2. **Inner panel** — `panel_image_node()` with flex-column layout, padding, `width: Auto`. This is the visible panel.
3. **Content** — title row, body text, divider, button row inside the inner panel.

Animation (`UiScaleIn`) targets the **inner panel** (first child of wrapper).

## Data-Driven Design

- Entity configs, NPC configs, and area configs come from JSON files in `assets/data/`.
- The code must be generic. No entity-specific logic — behavior comes from data.
- Adding a new entity = adding a JSON file, not changing code.

## Animation & UI

- All UI animations use shared components: `UiFadeIn`, `UiFadeOut`, `UiSlideIn`, `UiScaleIn`.
- Follow the specs in `docs/ui_guidelines.md` for colors, padding, z-ordering, 9-slice config.
- Only one panel visible at a time (mutual exclusion).
- Every interaction panel has a "Nevermind" button.

## Audio

- Spatial/3D audio for in-world sound sources (torches, NPCs, props).
- Global audio for ambient loops and music.
- All audio respects `AudioSettings` volume multipliers.

## Testing

- Every feature needs an e2e test with screenshots under xvfb.
- Tests should be adversarial — try to break things.
