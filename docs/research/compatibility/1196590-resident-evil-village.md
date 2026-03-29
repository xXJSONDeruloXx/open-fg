# 1196590 — Resident Evil Village

## Identity
- AppID: `1196590`
- Executable seen in logs: `re8.exe`
- Engines seen in logs:
  - `DXVK`
  - `vkd3d`

## Main outcome
Status: **working with OMFG after the timing-injection fix**.

Validated repo snapshot:
- commit `273d171` and later

## Original failure signature
Before the fix, the key symptoms were:
- user-visible error: `D3D12CreateDeviceFailed`
- OMFG log reached instance creation, then failed in device creation

Bad log snippets that informed the diagnosis:

```text
app=re8.exe; engine=DXVK
app=re8.exe; engine=vkd3d
vkCreateDevice returned -13
```

Interpretation:
- the layer was loading into the game
- the failure happened after `vkCreateInstance`
- the failure happened before swapchain/present
- both DXVK and VKD3D paths were affected

## Root-cause experiment that proved the issue
Temporary wrapper-only experiment:
- `OMFG_CREATE_DEVICE_APPEND_TIMING_EXTENSIONS=0`
- `OMFG_CREATE_DEVICE_APPEND_TIMING_FEATURES=0`

That immediately changed the outcome from device-create failure to successful device creation and present activity.

Good log snippets from the successful experiment:

```text
app=re8.exe; engine=DXVK
vkCreateDevice ok
app=re8.exe; engine=vkd3d
vkCreateDevice ok
vkCreateSwapchainKHR ok
first reproject blended generated-frame present succeeded
reproject blended frame present=...
```

Interpretation:
- the failure was tied to OMFG timing extension / feature injection during `vkCreateDevice`
- the base interception, swapchain mutation, and generated-frame path were not the root problem

## Final fix
Committed in:
- `273d171` — `fix: gate timing injection for real game compatibility`

Behavior after the fix:
- timing extension/feature injection is **off by default** for real games
- explicit timing validation scripts opt back in when needed

## Confirmed post-fix behavior
Dedicated Deck rerun showed:

```text
app=re8.exe; engine=DXVK; ... vkCreateDevice ok
app=re8.exe; engine=vkd3d; ... vkCreateDevice ok
vkCreateSwapchainKHR ok
first reproject blended generated-frame present succeeded
reproject blended frame present=300
```

Additional runtime evidence:
- live `re8.exe` process observed during validation
- live Proton chain observed during validation
- live `wineserver` observed during validation

## Practical note
As of the validated fix, RE Village is the current proof point that OMFG can:
- load in a real Proton game
- survive both DXVK and VKD3D startup paths
- create the swapchain
- generate and present interpolated frames in-game

## Later revalidation after Beyond-focused code changes
After the Beyond-focused sequencing / acquire-timeout investigation, RE Village was rerun again to ensure no regression on the default path.

Observed Deck evidence:

```text
app=re8.exe; engine=DXVK; apiVersion=1.3.0
app=re8.exe; engine=vkd3d; apiVersion=1.3.0
vkCreateDevice ok
vkCreateSwapchainKHR ok
first reproject blended generated-frame present succeeded
reproject blended frame present=60
reproject blended frame present=120
reproject blended frame present=300
```

Additional note:
- RE Village still showed occasional `AcquireNextImageKHR timed out for blend frame; skipping injection this present` warnings during the live rerun
- but the game remained alive and OMFG continued producing generated presents through the run

Interpretation:
- the Beyond troubleshooting changes did not regress the previously working RE Village path

## Later revalidation after blend-sequencing code changes
After adding blend-path original-first sequencing support for Beyond (`OMFG_BLEND_ORIGINAL_PRESENT_FIRST`), RE Village was rerun again to check for regressions on the default path.

Observed Deck evidence:

```text
app=re8.exe; engine=DXVK; apiVersion=1.3.0
app=re8.exe; engine=vkd3d; apiVersion=1.3.0
vkCreateDevice ok
vkCreateSwapchainKHR ok
first reproject blended generated-frame present succeeded
reproject blended frame present=1; generatedImageIndex=2; currentImageIndex=1; originalFirst=0
```

Interpretation:
- the new blend-path sequencing support did not break the existing default RE Village path in the quick live rerun
- the live log still shows default behavior (`originalFirst=0`) for RE Village unless the new Beyond-oriented blend knob is explicitly enabled

## Later revalidation after adaptive acquire-timeout fallback
After replacing the fixed generated-acquire fallback with an adaptive policy based on observed present interval, RE Village was rerun again.

Observed Deck evidence:

```text
app=re8.exe; engine=DXVK; apiVersion=1.3.0
app=re8.exe; engine=vkd3d; apiVersion=1.3.0
vkCreateDevice ok
vkCreateSwapchainKHR ok
first reproject blended generated-frame present succeeded
reproject blended frame present=60; generatedImageIndex=4; currentImageIndex=0; originalFirst=0
reproject blended frame present=180; generatedImageIndex=0; currentImageIndex=3; originalFirst=0
reproject blended frame present=300; generatedImageIndex=0; currentImageIndex=3; originalFirst=0
```

Interpretation:
- the adaptive acquire-timeout fallback did not regress the default RE Village path in the live rerun
- RE Village continued to generate frames without needing the Beyond-specific original-first blend knob
