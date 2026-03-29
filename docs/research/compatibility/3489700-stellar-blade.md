# 3489700 — Stellar Blade™

## Identity
- AppID: `3489700`
- Installed title: `Stellar Blade™`
- Observed install path:
  - `~/.local/share/Steam/steamapps/common/StellarBlade/SB.exe`

## Why this title matters
- Tier 1 flagship-sized installed title
- modern Windows game under Proton is exactly the kind of high-value real-game stress case this sweep should prioritize

## Current status
Status: **launch reached live Windows process, but OMFG layer engagement still unproven**.

## Evidence
Manifest evidence:
- `appmanifest_3489700.acf` exists
- `installdir` = `StellarBlade`
- `SizeOnDisk` ≈ `64.7 GB`
- `LastPlayed` populated

Binary evidence:
- `file ~/.local/share/Steam/steamapps/common/StellarBlade/SB.exe`
  - `PE32+ executable for MS Windows`

First probe after harness fix:
- `OMFG_LAYER_MODE=passthrough OMFG_GAME_WAIT_SEC=45 ./scripts/test-steamdeck-steam-game.sh 3489700 stellar-blade 'Stellar Blade™' 'SB.exe'`

Artifacts:
- `artifacts/steamdeck/rust/real-games/stellar-blade/passthrough/session.txt`

Observed process evidence:
```text
SteamLaunch AppId=3489700 ... Proton 10.0/proton waitforexitandrun /home/deck/.local/share/Steam/steamapps/common/StellarBlade/SB.exe -DistributionPlatform=Steam
Z:\home\deck\.local\share\Steam\steamapps\common\StellarBlade\SB.exe -DistributionPlatform=Steam
```

Observed OMFG evidence:
```text
=== title=Stellar Blade™ appid=3489700 mode=passthrough omfg log ===
missing-log
```

## Interpretation
- the safer generic harness now survives the run and captures reliable live process evidence
- Stellar Blade definitely reaches a live Proton/Windows executable on Deck
- but OMFG still did not produce any Vulkan-layer log during this passthrough probe
- current possibilities include:
  - the title did not yet reach Vulkan device/swapchain work inside the 45-second window
  - the title is blocked earlier in startup
  - the title is using a path that is not reaching the layer under these conditions

## Cross-contamination note

The GRIS probe immediately after this run captured the Stellar Blade OMFG layer output (showing
`app=SB-Win64-Shipping.exe`), which proves the OMFG layer **did** load successfully inside Stellar
Blade during that first probe window. The layer output was captured by the next game's harness
because the process was still alive.

This means the "missing-log" in the Stellar Blade probe itself was a log-path timing issue:
- Stellar Blade is a large title with a slower startup/Vulkan initialization path
- the 45-second wait window may not have been enough for the layer to write to the log file
  before the probe's `tail` ran
- the OMFG log is created at first Vulkan entry (device creation), which may happen later in the
  startup sequence for large titles

## Revised interpretation
- The OMFG layer **does** load in Stellar Blade (proven by the GRIS cross-contamination artifact)
- The missing-log in the Stellar Blade probe was a **wait-window timing artifact**, not a layer failure
- A longer wait window (`OMFG_GAME_WAIT_SEC >= 75`) should produce a captured OMFG log

## Next follow-up
- rerun Stellar Blade with `OMFG_GAME_WAIT_SEC=90` after the harness broad pre-kill fix
- verify that the OMFG log shows `app=SB-Win64-Shipping.exe` and clean `vkCreateDevice ok` / `vkCreateSwapchainKHR ok`
- if confirmed, classify as: **passthrough works, long startup** → promote to next mode family (clear/bfi)
- note: the harness `kill_all_steam_games()` fix prevents Stellar Blade from contaminating subsequent probes
