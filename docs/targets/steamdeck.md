# Remote target: Steam Deck

## Access

Configured remote target:
- user: `deck`
- host: `192.168.0.241`

Credentials are **not** stored in-repo.
Use either:
- environment variable: `STEAMDECK_PASS`
- or a local gitignored file: `.env.steamdeck.local`

Helper files:
- `.env.steamdeck.local.example`
- `scripts/steamdeck-run.sh`
- `scripts/steamdeck-scp-to.sh`
- `scripts/steamdeck-scp-from.sh`
- `scripts/list-steamdeck-owned-apps.sh`
- `scripts/install-steamdeck-steam-app.sh`
- `scripts/uninstall-steamdeck-steam-app.sh`

Example usage:

```bash
export STEAMDECK_PASS='...'
./scripts/steamdeck-run.sh 'uname -a'
./scripts/steamdeck-scp-to.sh ./local-file /home/deck/local-file
./scripts/steamdeck-scp-from.sh /home/deck/output.log ./artifacts/output.log
./scripts/list-steamdeck-owned-apps.sh | head
./scripts/install-steamdeck-steam-app.sh 20
./scripts/uninstall-steamdeck-steam-app.sh 20
```

---

## Detected environment

Connection verified on 2026-03-26.

### OS
- `SteamOS 3.7.19`
- codename: `holo`
- variant: `steamdeck`

### Kernel
- `6.11.11-valve26-1-neptune-611-gb3afa9aa9ae7`

### Architecture
- `x86_64`

### Vulkan stack
From `vulkaninfo`:
- Vulkan instance version: `1.4.303`
- GPU: `AMD Custom GPU 0932 (RADV VANGOGH)`
- driver: `Mesa 24.3.0-devel (git-aef01ebd12)`
- Vulkan driver: `radv`

### Gamescope
- `gamescope version 3.16.14.5`

### Vulkan layers observed
- `VK_LAYER_FROG_gamescope_wsi_x86`
- `VK_LAYER_FROG_gamescope_wsi_x86_64`
- `VK_LAYER_MANGOHUD_overlay_x86`
- `VK_LAYER_MANGOHUD_overlay_x86_64`
- Steam overlay / fossilize layers
- RenderDoc capture layers

---

## Why this target matters

The Steam Deck is a very good early Linux target for this project because it gives us:
- a real native Linux Vulkan environment
- AMD / RADV behavior
- real `gamescope` availability
- realistic handheld / LSFG-adjacent use cases

It is especially useful for:
- pass-through Vulkan layer validation
- swapchain interception tests
- frame insertion smoke tests
- observing interaction with `gamescope`

It is less useful for:
- NVIDIA-specific acceleration paths
- proprietary driver behavior comparisons

---

## Recommended first tests on this target

1. Pass-through layer smoke test
   - `vkcube`
   - `vkgears`

2. Placeholder frame insertion test
   - duplicate-frame insertion
   - blend-frame insertion

3. Nested `gamescope` compatibility test
   - compare direct run vs nested `gamescope`

4. Proton sample title
   - verify layer still loads and presents correctly

---

## Steam client entitlement / install automation

### What is available remotely
The Deck has enough local Steam metadata to let us infer a large owned-entitlement set without needing browser automation or Steam account credentials in-repo.

Useful local paths:
- owned-library cache: `~/.local/share/Steam/appcache/librarycache/`
- installed manifests: `~/.local/share/Steam/steamapps/appmanifest_*.acf`
- install/update log: `~/.local/share/Steam/logs/content_log.txt`

Practical meaning:
- if an AppID exists under `appcache/librarycache/<appid>/`, Steam knows about it in the account library cache
- if `steamapps/appmanifest_<appid>.acf` exists, Steam considers it installed or partially installed locally

Current helper:
- `./scripts/list-steamdeck-owned-apps.sh`

Example:
```bash
./scripts/list-steamdeck-owned-apps.sh | head -40
```

### Install automation (validated)
Current best autonomous install path uses the **Steam client console** opened inside the Deck desktop session, then types:
- `app_install <appid>`

Helper:
- `./scripts/install-steamdeck-steam-app.sh <appid> [timeout-seconds]`

What it does:
1. attaches to the live Deck desktop session (`DISPLAY=:0` + `gamescope-environment`)
2. opens the Steam console with `steam://open/console`
3. finds the Steam window via `xdotool`
4. types `app_install <appid>` and presses Enter
5. polls for `appmanifest_<appid>.acf`
6. prints manifest fields plus relevant `content_log.txt` lines

Validated example:
```bash
./scripts/install-steamdeck-steam-app.sh 20
```

Observed evidence for AppID `20` (`Team Fortress Classic`):
- `manifest-present`
- manifest fields included:
  - `appid=20`
  - `name=Team Fortress Classic`
  - `SizeOnDisk=125934169`
  - `BytesDownloaded=350947168`
- `content_log.txt` showed:
  - `AppID 20 state changed : Update Required`
  - `AppID 20 update started`
  - `AppID 20 finished update`

Important caveat:
- `Team Fortress Classic` also pulled shared Half-Life content (`AppID 70`) during install, so very old Valve titles may bring dependency depots with them.

### Uninstall automation (best-effort, not yet fully validated for every title)
Current helper:
- `./scripts/uninstall-steamdeck-steam-app.sh <appid> [timeout-seconds]`

What it currently tries:
1. trigger `steam://uninstall/<appid>`
2. open the Steam console
3. send Enter-confirmation keystrokes to Steam windows
4. type `app_uninstall -complete <appid>`
5. poll for `appmanifest_<appid>.acf` disappearing

This is the current best reusable autonomous path, but there is an important caveat:
- **install is validated**
- **uninstall is still best-effort**
- I have **not yet fully validated uninstall for dependency-sharing legacy titles**

Concrete example:
- after installing `AppID 20`, uninstall attempts did **not** remove `appmanifest_20.acf`
- that title appears to share Half-Life depots/content, so it is not a clean uninstall validation target

Interpretation:
- for future loops, I can reliably and autonomously:
  - enumerate owned-vs-installed AppIDs
  - trigger installs through the Steam client in the Deck desktop session
- uninstall automation exists as a reusable path now, but we should validate it on a more self-contained app before treating it as fully proven for every title

### Recommended workflow for future loops
1. list owned-but-uninstalled AppIDs:
   ```bash
   ./scripts/list-steamdeck-owned-apps.sh | rg 'owned-uninstalled'
   ```
2. pick a likely small/self-contained title
3. install it:
   ```bash
   ./scripts/install-steamdeck-steam-app.sh <appid>
   ```
4. confirm manifest/log state
5. when cleanup is needed, try:
   ```bash
   ./scripts/uninstall-steamdeck-steam-app.sh <appid>
   ```
6. if uninstall behaves strangely, prefer documenting the app’s dependency/shared-content behavior before reusing it as an automation canary
