#!/usr/bin/env python3
import argparse
import json
import sys
from pathlib import Path

ORDER = [
    "label",
    "mode",
    "samples",
    "generatedFrames",
    "gpuSamples",
    "avgCpuAcquireMs",
    "avgCpuSetupMs",
    "avgCpuRecordMs",
    "avgCpuSubmitMs",
    "avgCpuSubmitWaitMs",
    "avgCpuGeneratedPresentMs",
    "avgCpuOriginalPresentMs",
    "avgCpuQueueIdleMs",
    "avgCpuTotalMs",
    "avgCpuPerGeneratedFrameMs",
    "maxCpuTotalMs",
    "avgGpuCmdMs",
    "avgGpuPerGeneratedFrameMs",
    "maxGpuCmdMs",
]


def parse_value(value: str):
    value = value.strip()
    if value == "":
        return value
    for caster in (int, float):
        try:
            return caster(value)
        except ValueError:
            pass
    return value


def parse_summary(path: Path):
    summary_line = None
    for line in path.read_text().splitlines():
        if "benchmark summary;" in line:
            summary_line = line
    if summary_line is None:
        raise SystemExit(f"No benchmark summary found in {path}")

    tail = summary_line.split("benchmark summary;", 1)[1].strip()
    data = {}
    for part in tail.split(";"):
        part = part.strip()
        if not part:
            continue
        if "=" not in part:
            continue
        key, value = part.split("=", 1)
        data[key.strip()] = parse_value(value)
    return data


def csv_escape(value):
    text = str(value)
    if any(ch in text for ch in [",", '"', "\n"]):
        text = '"' + text.replace('"', '""') + '"'
    return text


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("log_path", type=Path, nargs="?")
    parser.add_argument("--json", action="store_true", dest="as_json")
    parser.add_argument("--csv", action="store_true")
    parser.add_argument("--header", action="store_true")
    args = parser.parse_args()

    if args.header:
        print(",".join(ORDER))
        return

    if args.log_path is None:
        parser.error("log_path is required unless --header is used")

    data = parse_summary(args.log_path)

    if args.as_json:
        print(json.dumps(data, indent=2, sort_keys=True))
        return

    if args.csv:
        print(",".join(csv_escape(data.get(key, "")) for key in ORDER))
        return

    print(
        "label={label} mode={mode} samples={samples} generatedFrames={generatedFrames} "
        "avgCpuTotalMs={avgCpuTotalMs} avgCpuPerGeneratedFrameMs={avgCpuPerGeneratedFrameMs} "
        "avgGpuCmdMs={avgGpuCmdMs} avgGpuPerGeneratedFrameMs={avgGpuPerGeneratedFrameMs} "
        "maxCpuTotalMs={maxCpuTotalMs} maxGpuCmdMs={maxGpuCmdMs}".format(**data)
    )


if __name__ == "__main__":
    main()
