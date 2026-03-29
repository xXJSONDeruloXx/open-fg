# 362890 — Black Mesa

## Identity
- AppID: `362890`
- Expected title: `Black Mesa`
- Executable regex used in the first generic harness probe:
  - `bms_linux|hl2_linux|blackmesa|bms.exe`

## Why this title matters
- substantial 3D game already installed on the Deck
- useful non-sentinel representative beyond Beyond / RE Village
- likely to exercise a different real-game path than the current sentinels

## Current status
Status: **meaningful title, but likely not OMFG-targetable on the current Vulkan path**.

The first generic-harness passthrough probe missed the live process. A longer follow-up confirmed the real executable path, but still produced no OMFG log evidence.

## First smoke probe
Repo snapshot during probe:
- local HEAD `e3b8829`

Harness used:
- `scripts/test-steamdeck-steam-game.sh 362890 black-mesa 'Black Mesa' 'bms_linux|hl2_linux|blackmesa|bms.exe'`

Mode/env used:
- `OMFG_LAYER_MODE=passthrough`
- `OMFG_GAME_WAIT_SEC=35`

Artifacts:
- `artifacts/steamdeck/rust/real-games/black-mesa/passthrough/session.txt`
- `artifacts/steamdeck/rust/real-games/black-mesa/passthrough/`

Observed evidence:
```text
=== title=Black Mesa appid=362890 mode=passthrough process ===
=== title=Black Mesa appid=362890 mode=passthrough omfg log ===
missing-log
```

Interpretation:
- the generic Steam launch path itself executed
- but this probe did not capture proof that the game actually reached the wrapper/layer
- possible explanations include:
  - wrong executable/process regex for this title
  - the game needed longer than the initial 35-second window
  - the title did not actually transition into a live launch despite being installed
  - the title may need a more specific wrapper/logging approach like Beyond received

## Longer follow-up
Follow-up command:
- `OMFG_LAYER_MODE=passthrough OMFG_GAME_WAIT_SEC=70 ./scripts/test-steamdeck-steam-game.sh 362890 black-mesa 'Black Mesa' 'bms_linux'`

Additional observed process evidence:
```text
SteamLaunch AppId=362890 ... /home/deck/.local/share/Steam/steamapps/common/Black Mesa/bms.sh -game bms +developer 0 -steam -newgameui
/home/deck/.local/share/Steam/steamapps/common/Black Mesa/bms_linux -disableboneuniformbuffers -game bms +developer 0 -steam -newgameui
```

Additional OMFG evidence:
```text
=== title=Black Mesa appid=362890 mode=passthrough omfg log ===
missing-log
```

Binary inspection evidence:
- `file ~/.local/share/Steam/steamapps/common/'Black Mesa'/bms_linux`
  - `ELF 32-bit LSB executable, Intel i386`
- `strings ~/.local/share/Steam/steamapps/common/'Black Mesa'/bms_linux | grep -i -m 5 vulkan`
  - no Vulkan hits observed

## Interpretation
- Black Mesa definitely launches and stays alive on Deck under the generic harness
- it launches natively via `bms_linux`, not through Proton
- OMFG still does not log anything for the title
- quick binary inspection did not show Vulkan linkage/signals
- current working conclusion: this title is likely using a non-Vulkan path on Deck and should be deprioritized for OMFG hardening unless later evidence shows a Vulkan-capable route

## Next follow-up
- no urgent OMFG follow-up unless a Vulkan path is discovered
- keep recorded in the compatibility queue as meaningful-but-currently-low-value for Vulkan FG coverage
