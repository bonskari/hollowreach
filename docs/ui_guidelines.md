# Hollowreach UI Guidelines

This document defines how all UI elements must be built in Hollowreach. Follow these rules exactly when creating or modifying any UI.

---

## Panel Image

All panels use a single shared image:

```
Panel/panel-012.png
```

Load with **nearest neighbor** sampling (`ImageSamplerDescriptor::nearest()`). This preserves the pixel-art aesthetic and prevents blurry edges.

## 9-Slice Configuration

Every panel uses 9-slice (via `ImageNode` + `ImageScaleMode::Sliced`) with these exact settings:

```rust
ImageScaleMode::Sliced(TextureSlicer {
    border: BorderRect::square(18.0),
    center_scale_mode: SliceScaleMode::Stretch,
    sides_scale_mode: SliceScaleMode::Tile { stretch_value: 3.0 },
    max_corner_scale: 2.0,
})
```

- `BorderRect::square(18.0)` -- 18px inset on all four sides for the border region.
- Center stretches to fill.
- Sides tile with a stretch value of 3.0 to avoid visible seams.
- Corner scaling capped at 2.0 to prevent oversized corners on small panels.

## Panel Tint (Dark Backgrounds)

Panels used as dark backgrounds (dialogue boxes, NPC interaction panels, pause menus) get a semi-transparent black tint:

```rust
Color::srgba(0.0, 0.0, 0.0, 0.8)
```

This darkens the panel image to make overlaid text readable.

## Button Panels

Buttons reuse the same `Panel/panel-012.png` image with the same 9-slice config, but with **no tint** -- leave the color at the default white (no modification). The panel image's natural appearance serves as the button background.

## Text Colors

### Button Text

Black text on the untinted button panel:

```rust
Color::srgba(0.0, 0.0, 0.0, 1.0)
```

### Panel Text (Body / General)

Light white for readability on dark-tinted panels:

```rust
Color::srgba(0.9, 0.9, 0.9, 1.0)
```

### Title Text

Gold for headings, NPC names, menu titles:

```rust
Color::srgb(0.95, 0.82, 0.4)
```

### Subtitle / Role Text

Grey for secondary information (NPC roles, descriptions, hints):

```rust
Color::srgb(0.7, 0.7, 0.7)
```

## Padding

All panels must have **minimum** internal padding of:

- **Horizontal**: 32px
- **Vertical**: 24px

```rust
UiRect {
    left: Val::Px(32.0),
    right: Val::Px(32.0),
    top: Val::Px(24.0),
    bottom: Val::Px(24.0),
}
```

This ensures text and buttons never touch the panel border or the 9-slice edge region.

## Divider

Use the divider fade image for visual separation within panels (e.g., between title and body, between body and buttons):

```
ui/Divider Fade/divider-fade-003.png
```

Do **not** stretch the divider too wide. Constrain its width so it does not span the full panel width -- it should feel like a subtle accent, not a hard rule. A max width of roughly 60-80% of the panel works well.

## Z-Ordering

UI layers are ordered by `ZIndex::Global(n)`. Higher values render on top:

| Layer             | Z-Index | Purpose                              |
|-------------------|---------|--------------------------------------|
| Pause menu        | 200     | Always on top of everything          |
| NPC panel         | 100     | Interaction/dialogue panels with NPCs|
| Text input        | 50      | Any text input fields                |
| Dialogue          | 10      | Dialogue boxes, narrative text       |
| Interaction       | 5       | "Press E to interact" prompt         |
| Hint              | 0       | Passive hints, tooltips              |

## Mutual Exclusion

**Only one UI panel may be visible at a time.** When a new panel opens, all other panels must be hidden (`Visibility::Hidden`). This prevents overlapping UI and ensures the player always has a single clear focus.

For example: if the pause menu opens, the NPC interaction panel and any dialogue must be hidden. When the pause menu closes, restore the previously active panel if appropriate.

## Interaction Flow

The standard interaction pattern is:

1. Player approaches an interactable entity.
2. An "interaction hint" appears (e.g., "Press E to interact") at z-index 5.
3. Player presses **E**.
4. A **centered panel** appears with context (NPC name, description) and a list of **buttons** for available actions.
5. Player clicks a button to perform the action.
6. The panel updates or closes based on the result.

## "Nevermind" Button

**Every interaction panel must include a "Nevermind" button as the last option.** This allows the player to back out of any interaction without committing to an action. It simply closes the panel and returns to gameplay.

---

## Quick Reference

```rust
// Panel setup
let panel_image = "Panel/panel-012.png";  // nearest neighbor sampling
let slicer = TextureSlicer {
    border: BorderRect::square(18.0),
    center_scale_mode: SliceScaleMode::Stretch,
    sides_scale_mode: SliceScaleMode::Tile { stretch_value: 3.0 },
    max_corner_scale: 2.0,
};

// Colors
let panel_tint       = Color::srgba(0.0, 0.0, 0.0, 0.8);   // dark bg panels
let button_tint      = Color::WHITE;                          // no tint
let button_text      = Color::srgba(0.0, 0.0, 0.0, 1.0);   // black
let panel_text       = Color::srgba(0.9, 0.9, 0.9, 1.0);   // white
let title_text       = Color::srgb(0.95, 0.82, 0.4);        // gold
let subtitle_text    = Color::srgb(0.7, 0.7, 0.7);          // grey

// Divider
let divider_image = "ui/Divider Fade/divider-fade-003.png";

// Padding (minimum)
let padding = UiRect {
    left: Val::Px(32.0),
    right: Val::Px(32.0),
    top: Val::Px(24.0),
    bottom: Val::Px(24.0),
};
```
