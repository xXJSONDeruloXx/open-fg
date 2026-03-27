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
    "bfi": [
        "first generated black-frame present succeeded",
        "black frame present=120",
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
    "adaptive-blend": [
        "adaptive-blend primed previous frame history",
        "first adaptive blended generated-frame present succeeded",
        "adaptive blended frame present=60",
    ],
    "multi-blend": [
        "multi-blend primed previous frame history",
        "first multi blended generated-frame present succeeded",
        "multi blended frame present=120",
    ],
    "adaptive-multi-blend": [
        "adaptive-multi-blend primed previous frame history",
        "first adaptive multi blended generated-frame present succeeded",
        "adaptive multi blended frame present=1",
    ],
    "search-blend": [
        "search-blend primed previous frame history",
        "first search blended generated-frame present succeeded",
        "search blended frame present=60",
    ],
    "search-adaptive-blend": [
        "search-adaptive-blend primed previous frame history",
        "first search adaptive blended generated-frame present succeeded",
        "search adaptive blended frame present=60",
    ],
    "reproject-blend": [
        "reproject-blend primed previous frame history",
        "first reproject blended generated-frame present succeeded",
        "reproject blended frame present=60",
    ],
    "reproject-adaptive-blend": [
        "reproject-adaptive-blend primed previous frame history",
        "first reproject adaptive blended generated-frame present succeeded",
        "reproject adaptive blended frame present=60",
    ],
    "optflow-blend": [
        "optflow-blend primed previous frame history",
        "first optical-flow blended generated-frame present succeeded",
        "optical-flow blended frame present=60",
    ],
    "optflow-adaptive-blend": [
        "optflow-adaptive-blend primed previous frame history",
        "first optical-flow adaptive blended generated-frame present succeeded",
        "optical-flow adaptive blended frame present=60",
    ],
    "optflow-multi-blend": [
        "optflow-multi-blend primed previous frame history",
        "first optical-flow multi blended generated-frame present succeeded",
        "optical-flow multi blended frame present=120",
    ],
    "optflow-adaptive-multi-blend": [
        "optflow-adaptive-multi-blend primed previous frame history",
        "first optical-flow adaptive multi blended generated-frame present succeeded",
        "optical-flow adaptive multi blended frame present=1",
    ],
    "reproject-multi-blend": [
        "reproject-multi-blend primed previous frame history",
        "first reproject multi blended generated-frame present succeeded",
        "reproject multi blended frame present=120",
    ],
    "reproject-adaptive-multi-blend": [
        "reproject-adaptive-multi-blend primed previous frame history",
        "first reproject adaptive multi blended generated-frame present succeeded",
        "reproject adaptive multi blended frame present=1",
    ],
}


def main() -> int:
    parser = argparse.ArgumentParser(description="Assert expected OMFG vkcube log markers.")
    parser.add_argument("--mode", required=True, choices=sorted(MODE_MARKERS))
    parser.add_argument("--log", required=True)
    parser.add_argument(
        "--expect-text",
        action="append",
        default=[],
        help="Additional raw text markers that must appear in the log.",
    )
    parser.add_argument(
        "--skip-mode-markers",
        action="store_true",
        help="Only assert common markers plus any extra expected text.",
    )
    args = parser.parse_args()

    log_path = pathlib.Path(args.log)
    if not log_path.exists():
        print(f"missing log: {log_path}", file=sys.stderr)
        return 1

    text = log_path.read_text(encoding="utf-8", errors="replace")
    expected_markers = COMMON_MARKERS + args.expect_text
    if not args.skip_mode_markers:
        expected_markers += MODE_MARKERS[args.mode]
    missing = [marker for marker in expected_markers if marker not in text]
    if missing:
        print(f"log assertion failed for {log_path}", file=sys.stderr)
        for marker in missing:
            print(f"  missing: {marker}", file=sys.stderr)
        return 1

    print(f"log assertion passed: {log_path} ({args.mode})")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
