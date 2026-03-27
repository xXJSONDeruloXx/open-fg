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

## Next follow-up
- rerun Stellar Blade with a longer wait window now that the harness is stable
- if still no OMFG log, classify as a startup/live-but-no-layer case and move on to another Tier 1/Tier 2 Windows title
