# App icon

A pixel-art teddy bear hugging a shipping container, one colour (accent `#6240E0`) on a white rounded tile.

- Master: `assets/icon/telima.png`, 1024 × 1024, 824 px tile (macOS icon grid). Everything else is derived by `scripts/icons.sh`.
- Made on 2026-11-11 with the image tool built into Codex CLI (OpenAI's image model; the version was not verified locally). The C2PA record says `ChatGPT / gpt-image`.
- The raw output was cleaned to two colours (accent pixels kept, everything else removed), cropped, and placed on the tile.

Prompt:

```text
Use case: logo-brand
Asset type: macOS/Linux desktop app icon for "telima", a small open-source app that manages Colima container VMs.
Subject: a small teddy bear sitting and hugging a single shipping container (a simple box with vertical ribs) against its chest with both arms. Friendly, calm, compact silhouette.
Style/medium: simple, modern pixel-art / digital look. Chunky square pixels on a visible low-resolution grid (about 32x32 pixel grid scaled up crisply), no anti-aliasing, flat shapes. Strictly monochrome: one single violet color #6240E0 on a plain white background; use only that one color (solid pixels) and white (negative space) to separate the bear's arms, ears, face and the container ribs. No gradients, no shading, no outlines in other colors.
Composition/framing: square 1:1, centered, the bear and container fill about 70% of the canvas with even margins, readable at 32 px.
Text: none.
Avoid: any text or letters, watermarks, extra objects, background scenery, rounded-rectangle tile, drop shadows, 3D, second colors, blur.
```
