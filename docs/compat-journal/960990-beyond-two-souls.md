# 960990 — Beyond: Two Souls

## Identity
- AppID: `960990`
- Executable seen in logs: `BeyondTwoSouls_Steam.exe`
- Engine seen in logs:
  - `DXVK`

## Status
Status: **partially improved, but still not good enough**.

What is fixed:
- the old hard startup/device-creation failure is gone after commit `273d171`

What is still wrong:
- with active OMFG generated-frame modes, the game appears to sit on a black/frozen screen and does not make sustained forward progress

## Original failure signature
Before the timing-injection fix, the key failure was:

```text
app=BeyondTwoSouls_Steam.exe; engine=DXVK
vkCreateDevice returned -13
```

That matched the same family of startup failure seen in RE Village before the fix.

## What improved after the fix
After commit `273d171`, the game now gets past the old startup blocker:

```text
app=BeyondTwoSouls_Steam.exe; engine=DXVK
vkCreateDevice ok
```

So the first compatibility fix was real and necessary.

## New evidence from direct Deck investigation
### With default active OMFG mode (`reproject-blend`)
Observed in OMFG log after ~45s while the process stayed alive:

```text
app=BeyondTwoSouls_Steam.exe; engine=DXVK; apiVersion=1.3.0
vkCreateDevice ok; gpu=AMD Custom GPU 0932 (RADV VANGOGH)
vkCreateSwapchainKHR ok; extent=1280x800; format=37; presentMode=MAILBOX; minImages=4->6; usage=TRANSFER_SRC|TRANSFER_DST|STORAGE|COLOR_ATTACHMENT (0x1b) -> TRANSFER_SRC|TRANSFER_DST|SAMPLED|STORAGE|COLOR_ATTACHMENT (0x1f); images=6; mode=reproject-blend-test
vkQueuePresentKHR frame=1; queueFamily=0; imageIndex=0; waitSemaphores=1
reproject-blend primed previous frame history
vkQueuePresentKHR frame=2; queueFamily=0; imageIndex=1; waitSemaphores=1
first reproject blended generated-frame present succeeded
reproject blended frame present=1; generatedImageIndex=2; currentImageIndex=1
vkQueuePresentKHR frame=3; queueFamily=0; imageIndex=3; waitSemaphores=1
reproject blended frame present=2; generatedImageIndex=4; currentImageIndex=3
```

At the same time, the process tree remained alive:
- live Steam `AppId=960990`
- live Proton process chain
- live `wineserver`
- live `BeyondTwoSouls_Steam.exe`

Interpretation:
- the game is no longer crashing at startup
- OMFG is no longer blocked at `vkCreateDevice`
- the game reaches swapchain creation and the first few presents
- but it does **not** continue into sustained present activity under active FG mode

### With OMFG passthrough
A controlled wrapper test forcing `OMFG_LAYER_MODE="passthrough"` showed sustained present activity for the same game:

```text
vkCreateSwapchainKHR ok; extent=1280x800; format=37; presentMode=MAILBOX; minImages=4->4; usage=TRANSFER_SRC|TRANSFER_DST|STORAGE|COLOR_ATTACHMENT (0x1b) -> TRANSFER_SRC|TRANSFER_DST|STORAGE|COLOR_ATTACHMENT (0x1b); images=4; mode=passthrough
vkQueuePresentKHR passthrough frame=1
...
vkQueuePresentKHR passthrough frame=60
...
vkQueuePresentKHR passthrough frame=300
...
vkQueuePresentKHR passthrough frame=1620
```

Interpretation:
- the wrapper itself is not the problem
- the layer loading into Beyond is not the problem
- the game can sustain present traffic with OMFG loaded when OMFG is not mutating/inserting generated frames
- the remaining issue is tied to **active generated-frame behavior**, not to basic layer presence

### With another active FG mode (`multi-blend`)
A second controlled wrapper test forcing `OMFG_LAYER_MODE="multi-blend"` showed the same general early-progress-then-stall pattern:

```text
vkCreateSwapchainKHR ok; extent=1280x800; format=37; presentMode=MAILBOX; minImages=4->7; usage=TRANSFER_SRC|TRANSFER_DST|STORAGE|COLOR_ATTACHMENT (0x1b) -> TRANSFER_SRC|TRANSFER_DST|SAMPLED|STORAGE|COLOR_ATTACHMENT (0x1f); images=7; mode=multi-blend-test
vkQueuePresentKHR frame=1
multi-blend primed previous frame history
vkQueuePresentKHR frame=2
first multi blended generated-frame present succeeded
multi blended frame present=2; generatedImageIndices=[2, 3]; currentImageIndex=1
vkQueuePresentKHR frame=3
multi blended frame present=4; generatedImageIndices=[5, 6]; currentImageIndex=4
```

Interpretation:
- this is probably **not only** a reprojection-specific shader problem
- the shared failure pattern appears across more than one active FG mode
- likely suspects now include:
  - generated-frame insertion/present sequencing on this title
  - swapchain image-count mutation (`4 -> 6` / `4 -> 7`)
  - added sampled usage on this swapchain path
  - MAILBOX-specific behavior in this title under active FG insertion

## New isolation evidence from simple generated modes
A repeatable per-mode harness now exists locally:
- `scripts/test-steamdeck-beyond-two-souls.sh`

It safely:
- backs up `~/omfg.sh`
- swaps in a mode-specific wrapper for the run
- launches AppID `960990`
- captures process snapshot + OMFG log
- restores the original wrapper afterward
- saves local artifacts under `artifacts/steamdeck/rust/real-games/beyond-two-souls/<mode>/`

### `copy`
Observed:

```text
vkCreateSwapchainKHR ok; extent=1280x800; format=37; presentMode=MAILBOX; minImages=4->6; usage=TRANSFER_SRC|TRANSFER_DST|STORAGE|COLOR_ATTACHMENT (0x1b) -> TRANSFER_SRC|TRANSFER_DST|STORAGE|COLOR_ATTACHMENT (0x1b); images=6; mode=copy-test
vkQueuePresentKHR frame=1
first duplicated-frame present succeeded
duplicated frame present=1; sourceImageIndex=0; generatedImageIndex=1
vkQueuePresentKHR frame=2
duplicated frame present=2; sourceImageIndex=2; generatedImageIndex=3
vkQueuePresentKHR frame=3
duplicated frame present=3; sourceImageIndex=4; generatedImageIndex=5
```

Interpretation:
- active FG still appears to stall very early
- no sampled-usage mutation was involved here
- this points away from shader sampling as the only cause

### `history-copy`
Observed:

```text
vkCreateSwapchainKHR ok; extent=1280x800; format=37; presentMode=MAILBOX; minImages=4->6; usage=TRANSFER_SRC|TRANSFER_DST|STORAGE|COLOR_ATTACHMENT (0x1b) -> TRANSFER_SRC|TRANSFER_DST|STORAGE|COLOR_ATTACHMENT (0x1b); images=6; mode=history-copy-test
vkQueuePresentKHR frame=1
history-copy primed previous frame history
vkQueuePresentKHR frame=2
first previous-frame insertion present succeeded
history-copy generated frame present=1; previousFrameSourceStored=1; generatedImageIndex=2; currentImageIndex=1
vkQueuePresentKHR frame=3
history-copy generated frame present=2; previousFrameSourceStored=1; generatedImageIndex=4; currentImageIndex=3
```

Interpretation:
- another non-sampled simple mode shows the same early-progress-then-stall pattern
- this further weakens the hypothesis that the main issue is reprojection or sampled image usage

### `clear`
Observed sustained progress deep into the run:

```text
vkCreateSwapchainKHR ok; extent=1280x800; format=37; presentMode=MAILBOX; minImages=4->5; usage=TRANSFER_SRC|TRANSFER_DST|STORAGE|COLOR_ATTACHMENT (0x1b) -> TRANSFER_SRC|TRANSFER_DST|STORAGE|COLOR_ATTACHMENT (0x1b); images=5; mode=clear-test
first generated clear-frame present succeeded
generated frame present=60
generated frame present=300
generated frame present=900
generated frame present=1680
```

User-observed behavior during this test:
- flashing green output, but the game seems visible underneath / partially progressing

Interpretation:
- extra generated presents themselves are **not** the root problem
- Beyond can keep running while OMFG inserts simple synthetic frames that do not depend on copying real frame contents
- the remaining issue is more likely tied to use of real app image contents in the generated path (copy/history/reprojection/multi-blend), not mere insertion cadence

### `bfi`
A cleaned-up rerun now also shows sustained progress under another simple non-content-reading inserted-frame mode:

```text
vkCreateSwapchainKHR ok; extent=1280x800; format=37; presentMode=MAILBOX; minImages=4->5; usage=TRANSFER_SRC|TRANSFER_DST|STORAGE|COLOR_ATTACHMENT (0x1b) -> TRANSFER_SRC|TRANSFER_DST|STORAGE|COLOR_ATTACHMENT (0x1b); images=5; mode=bfi-test
bfi settings; period=1; holdMs=0
first generated black-frame present succeeded
black frame present=60
black frame present=300
black frame present=900
black frame present=1680
```

Interpretation:
- Beyond tolerates both `clear` and `bfi` for long-running inserted-frame presentation
- that further supports the idea that the failing condition is tied to reading/copying real app image contents, not merely extra present cadence

### Swapchain-image-count override experiment
A targeted diagnostic toggle was added in code:
- `OMFG_SWAPCHAIN_IMAGE_BUMP_OVERRIDE`

Intent:
- override the default swapchain image-count bump during `vkCreateSwapchainKHR` for isolation experiments

Important harness fix:
- the Beyond harness now embeds override values directly into the temporary wrapper, instead of relying on inherited Steam-launch environment
- that made the override observable on Deck

Observed results:

#### `copy` with `OMFG_SWAPCHAIN_IMAGE_BUMP_OVERRIDE=0`
```text
swapchain image bump override applied; originalMinImages=4; overriddenMinImages=4; mode=copy-test
vkCreateSwapchainKHR ok; ... minImages=4->4; ... mode=copy-test
```

#### `copy` with `OMFG_SWAPCHAIN_IMAGE_BUMP_OVERRIDE=1`
```text
swapchain image bump override applied; originalMinImages=4; overriddenMinImages=5; mode=copy-test
vkCreateSwapchainKHR ok; ... minImages=4->5; ... mode=copy-test
```

#### `history-copy` with `OMFG_SWAPCHAIN_IMAGE_BUMP_OVERRIDE=0`
```text
swapchain image bump override applied; originalMinImages=4; overriddenMinImages=4; mode=history-copy-test
vkCreateSwapchainKHR ok; ... minImages=4->4; ... mode=history-copy-test
```

Interpretation:
- the override path is now confirmed working
- reducing swapchain image count from the default bumped values (`4->6`) down to `4->4` or `4->5` does **not** resolve the early-stall behavior for copy/history-copy
- so simple swapchain headroom growth is very unlikely to be the primary root cause of the Beyond issue

### History-refresh freeze experiment
A second targeted diagnostic toggle was added in code:
- `OMFG_HISTORY_COPY_FREEZE_HISTORY`

Intent:
- after the first valid history frame, stop refreshing history from the current app image each frame
- keep history-copy presenting from the already-captured private history image
- isolate whether the Beyond problem comes specifically from repeated current-image readback every frame

Observed result on Deck with:
- `OMFG_LAYER_MODE=history-copy`
- `OMFG_SWAPCHAIN_IMAGE_BUMP_OVERRIDE=0`
- `OMFG_HISTORY_COPY_FREEZE_HISTORY=1`

```text
swapchain image bump override applied; originalMinImages=4; overriddenMinImages=4; mode=history-copy-test
vkCreateSwapchainKHR ok; ... minImages=4->4; ... mode=history-copy-test
history-copy primed previous frame history
history-copy freeze-history enabled
first previous-frame insertion present succeeded
history-copy generated frame present=1; ... freezeHistory=1
history-copy generated frame present=2; ... freezeHistory=1
```

Interpretation:
- even when repeated history refresh from the current app image is disabled after priming, Beyond still remains in the same early-progress-then-stall class
- that weakens the hypothesis that the issue is only the *repeated* per-frame current-image readback step
- current strongest common factor is now even narrower: presenting generated frames whose contents come from copied real frame data appears to be the problematic class, whether the source is the current frame or preserved history

### Copy original-first sequencing experiment
A third targeted diagnostic toggle was added in code:
- `OMFG_COPY_ORIGINAL_PRESENT_FIRST`

Intent:
- make copy mode behave more like the `clear`/`bfi` success class by letting the original app present go through first
- then perform the duplicate-frame copy/injection path afterward
- isolate whether Beyond is sensitive to the original present being semaphore-gated behind OMFG's duplicate-frame submit path

Observed result on Deck with:
- `OMFG_LAYER_MODE=copy`
- `OMFG_SWAPCHAIN_IMAGE_BUMP_OVERRIDE=0`
- `OMFG_COPY_ORIGINAL_PRESENT_FIRST=1`

```text
swapchain image bump override applied; originalMinImages=4; overriddenMinImages=4; mode=copy-test
vkCreateSwapchainKHR ok; ... minImages=4->4; ... mode=copy-test
copy original-first mode enabled
first duplicated-frame present succeeded
duplicated frame present=1; ... originalFirst=1
duplicated frame present=2; ... originalFirst=1
duplicated frame present=3; ... originalFirst=1
duplicated frame present=4; ... originalFirst=1
duplicated frame present=5; ... originalFirst=1
AcquireNextImageKHR timed out for duplicate frame; skipping injection this present
```

Interpretation:
- this did **not** produce a full fix by itself, but it was the first tested change that visibly moved the failure boundary
- previous copy runs typically stopped after only the first few generated presents; with original-first sequencing the run reached at least five generated presents before the first explicit timeout warning
- that made present sequencing / acquire interaction a much stronger lead

### Generated-acquire-timeout experiment
A fourth targeted diagnostic / mitigation toggle was added in code:
- `OMFG_GENERATED_ACQUIRE_TIMEOUT_NS`

Intent:
- give generated-image acquisition more time in cases where original-first sequencing improves stability but still intermittently starves OMFG of a free generated image
- test whether Beyond is specifically sensitive to short acquire windows under active copied-content presentation

Observed result on Deck with:
- `OMFG_LAYER_MODE=copy`
- `OMFG_SWAPCHAIN_IMAGE_BUMP_OVERRIDE=0`
- `OMFG_COPY_ORIGINAL_PRESENT_FIRST=1`
- `OMFG_GENERATED_ACQUIRE_TIMEOUT_NS=500000000`

```text
swapchain image bump override applied; originalMinImages=4; overriddenMinImages=4; mode=copy-test
vkCreateSwapchainKHR ok; ... minImages=4->4; ... mode=copy-test
copy original-first mode enabled
duplicated frame present=5; ... originalFirst=1
duplicated frame present=60; ... originalFirst=1
duplicated frame present=300; ... originalFirst=1
duplicated frame present=900; ... originalFirst=1
duplicated frame present=1620; ... originalFirst=1
```

Interpretation:
- this is the first Beyond run in an active copied-content mode that clearly enters the same long-running success class as the earlier `clear` / `bfi` runs
- the current best evidence-backed compatibility recipe for Beyond is:
  - original app present first
  - no extra swapchain-image bump (`4->4` worked)
  - much longer generated-image acquire timeout (`500ms`)
- this strongly suggests the practical blocker was not copying pixels alone, but copied-content generation combined with overly aggressive generated-image acquire timing / sequencing
- importantly, this is still a **config-dependent compatibility path**, not yet proof that stock/default OMFG behavior for Beyond is fixed, because earlier default/near-default `copy` runs remained in the early-stall class until these explicit knobs were applied

### Blend follow-up audit
Question:
- can the longer generated-image acquire timeout alone promote Beyond blend modes into the same long-running success class?

Tested on Deck with:
- `OMFG_LAYER_MODE=reproject-blend`
- `OMFG_GENERATED_ACQUIRE_TIMEOUT_NS=500000000`

Observed log excerpt:

```text
vkCreateSwapchainKHR ok; ... minImages=4->6; ... mode=reproject-blend-test
reproject-blend primed previous frame history
first reproject blended generated-frame present succeeded
reproject blended frame present=1; generatedImageIndex=2; currentImageIndex=1
reproject blended frame present=2; generatedImageIndex=4; currentImageIndex=3
```

Tested on Deck with:
- `OMFG_LAYER_MODE=multi-blend`
- `OMFG_GENERATED_ACQUIRE_TIMEOUT_NS=500000000`

Observed log excerpt:

```text
vkCreateSwapchainKHR ok; ... minImages=4->7; ... mode=multi-blend-test
multi-blend primed previous frame history
first multi blended generated-frame present succeeded
multi blended frame present=2; generatedImageIndices=[2, 3]; currentImageIndex=1
multi blended frame present=4; generatedImageIndices=[5, 6]; currentImageIndex=4
```

Interpretation:
- the longer acquire timeout by itself does **not** yet put `reproject-blend` or `multi-blend` into the same long-running success class that `copy` now reaches with original-first sequencing plus the longer timeout
- at least in this iteration, both blend-derived modes still showed only their initial generated-frame progress before falling back into the same early-stop pattern
- this makes the current evidence-backed classification clearer:
  - `copy` has a validated **config-dependent compatibility recipe**
  - `reproject-blend` and `multi-blend` still need additional sequencing work, likely analogous to the copy-mode original-first experiment rather than timeout-only tuning

### Blend original-first follow-up
A fifth targeted sequencing knob was added in code:
- `OMFG_BLEND_ORIGINAL_PRESENT_FIRST`

Intent:
- apply the same successful idea from `copy` mode to blend-derived modes
- let the game’s original present complete first, then do the generated blend presents afterward
- combine that with the already-useful longer generated-image acquire timeout

Tested on Deck with:
- `OMFG_LAYER_MODE=reproject-blend`
- `OMFG_BLEND_ORIGINAL_PRESENT_FIRST=1`
- `OMFG_GENERATED_ACQUIRE_TIMEOUT_NS=500000000`

Observed log excerpt:

```text
vkCreateSwapchainKHR ok; ... minImages=4->6; ... mode=reproject-blend-test
first reproject blended generated-frame present succeeded
blend original-first mode enabled
reproject blended frame present=5; ... originalFirst=1
reproject blended frame present=60; ... originalFirst=1
reproject blended frame present=300; ... originalFirst=1
reproject blended frame present=600; ... originalFirst=1
reproject blended frame present=1140; ... originalFirst=1
```

Tested on Deck with:
- `OMFG_LAYER_MODE=multi-blend`
- `OMFG_BLEND_ORIGINAL_PRESENT_FIRST=1`
- `OMFG_GENERATED_ACQUIRE_TIMEOUT_NS=500000000`

Observed log excerpt:

```text
vkCreateSwapchainKHR ok; ... minImages=4->7; ... mode=multi-blend-test
first multi blended generated-frame present succeeded
multi-blend original-first mode enabled
multi blended frame present=6; ... originalFirst=1
multi blended frame present=60; ... originalFirst=1
multi blended frame present=300; ... originalFirst=1
multi blended frame present=1620; ... originalFirst=1
multi blended frame present=3060; ... originalFirst=1
```

Interpretation:
- this is the strongest generalization result so far: the same **original-first sequencing** idea that rescued `copy` also rescues both tested blend-derived Beyond modes when paired with the longer acquire timeout
- `reproject-blend` now reaches the same long-running active-FG success class in Beyond
- `multi-blend` also reaches a long-running active-FG success class in Beyond and sustains repeated generated presents far beyond the old early-stall boundary
- the evidence now points much more strongly to a shared root cause across these modes:
  - generated-image acquire / present sequencing pressure before the app’s original present
  - rather than a mode-specific incompatibility with sampling or blending itself

## Current best understanding
Most evidence-backed statement right now:
- **Beyond: Two Souls can sustain passthrough and clear-style insertion, but stalls once OMFG starts generating from real app image contents.**

That is much narrower and better than the old situation.

## Next evidence-based debugging directions
Prefer these over blind guesses:
1. compare simple generated modes (`copy`, `history-copy`, `clear`, `bfi`) against Beyond to separate:
   - extra present insertion
   - swapchain mutation
   - shader sampling/reprojection
2. isolate whether the stall is tied to:
   - extra images only
   - sampled-usage mutation only
   - generated presents before original present ordering
3. capture per-run notes with:
   - exact wrapper contents
   - exact mode
   - exact commit
   - log snippet showing last successful present

## Repo snapshot
Key compatibility fix already landed in:
- `273d171` — `fix: gate timing injection for real game compatibility`

The Beyond-specific black-screen/stall issue remains open after that fix.
