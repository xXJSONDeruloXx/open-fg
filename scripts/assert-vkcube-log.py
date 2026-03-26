#!/usr/bin/env python3
import argparse
import pathlib
import sys

COMMON_MARKERS = [
    "vkNegotiateLoaderLayerInterfaceVersion ok",
    "vkCreateInstance ok",
    "vkCreateDevice ok",
    "vkCreateSwapchainKHR ok",
    "vkDestroySwapchainKHR",
    "vkDestroyDevice",
    "vkDestroyInstance",
]

MODE_MARKERS = {
    "passthrough": [
        "vkQueuePresentKHR passthrough frame=1",
        "vkQueuePresentKHR passthrough frame=120",
    ],
    "clear": [
        "first generated clear-frame present succeeded",
        "generated frame present=120",
    ],
    "copy": [
        "first duplicated-frame present succeeded",
        "duplicated frame present=120",
    ],
    "history-copy": [
        "history-copy primed previous frame history",
        "first previous-frame insertion present succeeded",
        "history-copy generated frame present=60",
    ],
    "blend": [
        "blend primed previous frame history",
        "first blended generated-frame present succeeded",
        "blended frame present=60",
    ],
}


def main() -> int:
    parser = argparse.ArgumentParser(description="Assert expected PPFG vkcube log markers.")
    parser.add_argument("--mode", required=True, choices=sorted(MODE_MARKERS))
    parser.add_argument("--log", required=True)
    args = parser.parse_args()

    log_path = pathlib.Path(args.log)
    if not log_path.exists():
        print(f"missing log: {log_path}", file=sys.stderr)
        return 1

    text = log_path.read_text(encoding="utf-8", errors="replace")
    missing = [marker for marker in COMMON_MARKERS + MODE_MARKERS[args.mode] if marker not in text]
    if missing:
        print(f"log assertion failed for {log_path}", file=sys.stderr)
        for marker in missing:
            print(f"  missing: {marker}", file=sys.stderr)
        return 1

    print(f"log assertion passed: {log_path} ({args.mode})")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
