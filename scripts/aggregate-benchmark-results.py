#!/usr/bin/env python3
import argparse
import csv
import json
import statistics
from collections import OrderedDict
from pathlib import Path


NON_NUMERIC_FIELDS = {"label", "mode"}
SUMMARY_METRICS = [
    "avgCpuTotalMs",
    "avgCpuPerGeneratedFrameMs",
    "avgGpuCmdMs",
    "avgGpuPerGeneratedFrameMs",
]


def parse_value(raw: str):
    raw = raw.strip()
    if raw == "":
        return raw
    for caster in (int, float):
        try:
            return caster(raw)
        except ValueError:
            pass
    return raw


def resolve_csv_path(path_text: str) -> Path:
    path = Path(path_text)
    if path.is_dir():
        aggregate_csv = path / "aggregate.csv"
        results_csv = path / "results.csv"
        if aggregate_csv.exists():
            return aggregate_csv
        if results_csv.exists():
            return results_csv
        raise SystemExit(f"No aggregate.csv or results.csv found in {path}")
    return path


def load_rows(path: Path):
    with path.open(newline="") as handle:
        reader = csv.DictReader(handle)
        rows = []
        for row in reader:
            parsed = {key: parse_value(value) for key, value in row.items()}
            rows.append(parsed)
        return reader.fieldnames or [], rows


def aggregate_rows(paths):
    fieldnames = None
    rows_by_label = OrderedDict()

    for index, path in enumerate(paths):
        current_fieldnames, rows = load_rows(path)
        if fieldnames is None:
            fieldnames = current_fieldnames
        elif current_fieldnames != fieldnames:
            raise SystemExit(
                f"CSV header mismatch between {paths[0]} and {path}"
            )
        for row in rows:
            rows_by_label.setdefault(row["label"], []).append(row)

    if fieldnames is None:
        raise SystemExit("No CSV data loaded")

    numeric_fields = [field for field in fieldnames if field not in NON_NUMERIC_FIELDS]
    aggregated = []

    for label, rows in rows_by_label.items():
        base = OrderedDict()
        base["label"] = label
        base["mode"] = rows[0]["mode"]
        base["runCount"] = len(rows)
        for field in numeric_fields:
            values = [float(row[field]) for row in rows]
            base[field] = statistics.mean(values)
            base[f"stdev_{field}"] = statistics.pstdev(values) if len(values) > 1 else 0.0
            base[f"min_{field}"] = min(values)
            base[f"max_{field}"] = max(values)
        aggregated.append(base)

    return fieldnames, aggregated


def format_number(value):
    if isinstance(value, (int, float)):
        return f"{value:.3f}"
    return str(value)


def write_csv(path: Path, aggregated, base_fieldnames):
    extra_fields = ["runCount"]
    numeric_fields = [field for field in base_fieldnames if field not in NON_NUMERIC_FIELDS]
    for field in numeric_fields:
        extra_fields.extend([f"stdev_{field}", f"min_{field}", f"max_{field}"])
    header = ["label", "mode", "runCount"] + numeric_fields + extra_fields[1:]

    with path.open("w", newline="") as handle:
        writer = csv.DictWriter(handle, fieldnames=header)
        writer.writeheader()
        for row in aggregated:
            writer.writerow(row)


def build_summary_lines(aggregated):
    lines = []
    for row in aggregated:
        parts = [
            f"label={row['label']}",
            f"mode={row['mode']}",
            f"runCount={row['runCount']}",
        ]
        for metric in SUMMARY_METRICS:
            parts.append(f"{metric}={format_number(row[metric])}")
            parts.append(f"stdev_{metric}={format_number(row[f'stdev_{metric}'])}")
        lines.append(" ".join(parts))
    return lines


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("inputs", nargs="+", help="results.csv paths or directories containing results.csv")
    parser.add_argument("--csv-out", type=Path)
    parser.add_argument("--summary-out", type=Path)
    parser.add_argument("--json-out", type=Path)
    args = parser.parse_args()

    input_paths = [resolve_csv_path(path_text) for path_text in args.inputs]
    base_fieldnames, aggregated = aggregate_rows(input_paths)
    summary_lines = build_summary_lines(aggregated)

    if args.csv_out:
        args.csv_out.parent.mkdir(parents=True, exist_ok=True)
        write_csv(args.csv_out, aggregated, base_fieldnames)

    if args.summary_out:
        args.summary_out.parent.mkdir(parents=True, exist_ok=True)
        args.summary_out.write_text("\n".join(summary_lines) + "\n")

    if args.json_out:
        args.json_out.parent.mkdir(parents=True, exist_ok=True)
        args.json_out.write_text(json.dumps(aggregated, indent=2))

    print("\n".join(summary_lines))


if __name__ == "__main__":
    main()
