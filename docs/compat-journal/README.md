# Real-game compatibility journal

Tracked notes for Steam Deck real-game OMFG runs.

Purpose:
- keep per-game/appid history of what was tried
- record the repo snapshot / commit involved
- preserve good and bad log snippets that informed decisions
- capture wrapper contents / launch assumptions that mattered
- avoid repeating dead-end experiments

## Games
- `960990` — [Beyond: Two Souls](./960990-beyond-two-souls.md)
- `1196590` — [Resident Evil Village](./1196590-resident-evil-village.md)
- `362890` — [Black Mesa](./362890-black-mesa.md)
- `481510` — [Night in the Woods](./481510-night-in-the-woods.md)
- `683320` — [GRIS](./683320-gris.md)
- `3489700` — [Stellar Blade™](./3489700-stellar-blade.md)

## Current curated queue

### Tier 1 — flagship / highest-value next coverage
- `960990` — Beyond: Two Souls
  - valuable known-problem sentinel for active content-reading modes
- `1196590` — Resident Evil Village
  - valuable known-good sentinel spanning both DXVK and VKD3D startup paths
- `3489700` — Stellar Blade™
  - likely substantial modern stress case; installed on Deck and worth first-pass validation
- `362890` — Black Mesa
  - substantial 3D Proton-era representative installed on Deck

### Tier 2 — useful representative coverage
- `1263240` — Skate Story
- `1582650` — Caravan Sandwitch
- `1880620` — Once Upon A KATAMARI
- `2201320` — Date Everything!
- `207610` — The Walking Dead

### Tier 3 — lower-value but still potentially useful signal
- `481510` — Night in the Woods
- `683320` — GRIS
- `8400` — Geometry Wars: Retro Evolved

### Skip / non-game / not meaningful for compatibility matrix
- `1070560` — Steam Linux Runtime 1.0 (scout)
- `1391110` — Steam Linux Runtime 2.0 (soldier)
- `1628350` — Steam Linux Runtime 3.0 (sniper)
- `228980` — Steamworks Common Redistributables
- `3658110` — Proton 10.0
- `3873750` — Clair Obscur: Expedition 33 – Nos Vies En Lumière (Bonus Edition)

## Core real-game matrix
- baseline startup:
  - install state confirmed
  - safe Steam launch
  - OMFG layer load evidence
- baseline mode:
  - `passthrough`
  - note: after early sentinel/harness validation, broad passthrough-only sweeps can be skipped for additional titles unless needed for startup/layer-load isolation or sentinel regression checks
- low-risk insertion:
  - `clear`
  - `bfi`
- copied-content / history:
  - `copy`
  - `history-copy`
- transformed-content:
  - `blend`
  - `reproject-blend`
- multi/high-intensity where practical:
  - `multi-blend`
  - `reproject-multi-blend`
  - adaptive variants if the title survives the simpler families
- generation intensity targets where practical:
  - 1 generated frame
  - 2 generated frames
  - 3 generated frames
  - 4 generated frames

## Harnesses
- per-title specific harness:
  - `scripts/test-steamdeck-beyond-two-souls.sh`
- generic Steam title harness:
  - `scripts/test-steamdeck-steam-game.sh <appid> <slug> <title> [exe-regex]`

## Current shared wrapper shape
Deck-side wrapper path:
- `~/omfg.sh`

Known working core contents during the real-game compatibility investigation:

```bash
#!/usr/bin/env bash
set -euo pipefail
BASE=/home/deck/post-proc-fg-research
if [[ -n "${PRESSURE_VESSEL_FILESYSTEMS_RW:-}" ]]; then
  export PRESSURE_VESSEL_FILESYSTEMS_RW="${BASE}:${PRESSURE_VESSEL_FILESYSTEMS_RW}"
else
  export PRESSURE_VESSEL_FILESYSTEMS_RW="${BASE}"
fi
export VK_LAYER_PATH="${BASE}/deploy/vk-layer-rust"
export VK_INSTANCE_LAYERS="VK_LAYER_OMFG_rust"
export ENABLE_OMFG_RUST=1
export OMFG_LAYER_MODE="${OMFG_LAYER_MODE:-reproject-blend}"
export OMFG_REPROJECT_SEARCH_RADIUS="${OMFG_REPROJECT_SEARCH_RADIUS:-2}"
export OMFG_REPROJECT_PATCH_RADIUS="${OMFG_REPROJECT_PATCH_RADIUS:-1}"
export OMFG_REPROJECT_CONFIDENCE_SCALE="${OMFG_REPROJECT_CONFIDENCE_SCALE:-4.0}"
export OMFG_REPROJECT_DISOCCLUSION_CURRENT_BIAS="${OMFG_REPROJECT_DISOCCLUSION_CURRENT_BIAS:-0.75}"
export OMFG_LAYER_LOG_FILE="${BASE}/logs/re8-omfg.log"
mkdir -p "${BASE}/logs"
rm -f "${OMFG_LAYER_LOG_FILE}"
exec "$@"
```

## Important compatibility finding
Commit:
- `273d171` — `fix: gate timing injection for real game compatibility`

Meaning:
- OMFG no longer appends timing extensions/features during `vkCreateDevice` by default
- explicit timing validation still opts back in when needed
- this fixed the earlier real-game `vkCreateDevice returned -13` failures in Proton titles
