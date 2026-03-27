# 481510 — Night in the Woods

## Identity
- AppID: `481510`
- Installed title: `Night in the Woods`
- Observed runtime path on Deck:
  - `/home/deck/.local/share/Steam/steamapps/common/Night in the Woods/Night in the Woods.x86_64`

## Why this title matters
- installed representative outside the current Beyond / RE Village sentinels
- useful to distinguish Vulkan-targetable titles from meaningful-but-out-of-scope non-Vulkan titles in the compatibility queue

## Current status
Status: **meaningful title, but likely not OMFG-targetable on the current Vulkan path**.

## Evidence
Initial generic smoke command:
- `OMFG_LAYER_MODE=passthrough OMFG_GAME_WAIT_SEC=45 ./scripts/test-steamdeck-steam-game.sh 481510 night-in-the-woods 'Night in the Woods' 'Night in the Woods.x86'`

Artifacts:
- `artifacts/steamdeck/rust/real-games/night-in-the-woods/passthrough/session.txt`

Observed process evidence:
```text
SteamLaunch AppId=481510 ... /home/deck/.local/share/Steam/steamapps/common/Night in the Woods/Night in the Woods.x86_64
/home/deck/.local/share/Steam/steamapps/common/Night in the Woods/Night in the Woods.x86_64
```

Observed OMFG evidence:
```text
=== title=Night in the Woods appid=481510 mode=passthrough omfg log ===
missing-log
```

Binary inspection evidence:
- `file ~/.local/share/Steam/steamapps/common/'Night in the Woods'/'Night in the Woods.x86_64'`
  - `ELF 64-bit LSB executable, x86-64`
- `strings ~/.local/share/Steam/steamapps/common/'Night in the Woods'/'Night in the Woods.x86_64' | grep -i -m 5 vulkan`
  - no Vulkan hits observed

## Interpretation
- the game definitely launches as a native Linux executable on the Deck
- the OMFG Vulkan layer did not produce any log file during the passthrough run
- quick binary-string inspection did not show Vulkan linkage/signals
- current working conclusion: this title is probably using a non-Vulkan rendering path on Deck and is therefore low-value for OMFG compatibility hardening right now

## Queue impact
- keep recorded as a meaningful owned title
- deprioritize for further OMFG compatibility work unless later evidence shows a Vulkan-capable path worth testing
