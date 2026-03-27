#!/usr/bin/env python3
import argparse
import csv
import json
from pathlib import Path


PRESETS = {
    "decision": [
        {"label": "blend", "metric": "avgCpuTotalMs", "weight": 1.0},
        {"label": "reproject-blend-default", "metric": "avgCpuTotalMs", "weight": 1.5},
        {"label": "multi-blend-count3", "metric": "avgCpuPerGeneratedFrameMs", "weight": 3.0},
        {"label": "adaptive-multi-target180", "metric": "avgCpuPerGeneratedFrameMs", "weight": 3.0},
    ],
    "full": [
        {"label": "blend", "metric": "avgCpuTotalMs", "weight": 1.0},
        {"label": "adaptive-blend", "metric": "avgCpuTotalMs", "weight": 1.0},
        {"label": "search-blend-r1", "metric": "avgCpuTotalMs", "weight": 1.0},
        {"label": "reproject-blend-default", "metric": "avgCpuTotalMs", "weight": 1.5},
        {"label": "multi-blend-count2", "metric": "avgCpuPerGeneratedFrameMs", "weight": 2.0},
        {"label": "multi-blend-count3", "metric": "avgCpuPerGeneratedFrameMs", "weight": 3.0},
        {"label": "adaptive-multi-default", "metric": "avgCpuPerGeneratedFrameMs", "weight": 2.0},
        {"label": "adaptive-multi-target180", "metric": "avgCpuPerGeneratedFrameMs", "weight": 3.0},
    ],
}


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
        return {row["label"]: row for row in reader}


def parse_cases(text: str):
    cases = []
    for chunk in text.split(","):
        chunk = chunk.strip()
        if not chunk:
            continue
        parts = chunk.split(":")
        if len(parts) not in (2, 3):
            raise SystemExit(
                f"Invalid case spec '{chunk}'. Expected label:metric[:weight]"
            )
        label, metric = parts[0], parts[1]
        weight = float(parts[2]) if len(parts) == 3 else 1.0
        cases.append({"label": label, "metric": metric, "weight": weight})
    return cases


def to_float(row, key, default=0.0):
    raw = row.get(key, "")
    if raw in (None, ""):
        return default
    return float(raw)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("baseline")
    parser.add_argument("candidate")
    parser.add_argument("--preset", choices=sorted(PRESETS), default="decision")
    parser.add_argument("--cases", help="Override cases as label:metric[:weight],...")
    parser.add_argument("--max-regression-pct", type=float, default=0.5)
    parser.add_argument("--min-weighted-improvement-pct", type=float, default=0.25)
    parser.add_argument("--json", action="store_true", dest="as_json")
    args = parser.parse_args()

    baseline_path = resolve_csv_path(args.baseline)
    candidate_path = resolve_csv_path(args.candidate)
    baseline_rows = load_rows(baseline_path)
    candidate_rows = load_rows(candidate_path)
    cases = parse_cases(args.cases) if args.cases else PRESETS[args.preset]

    comparisons = []
    total_weight = 0.0
    weighted_improvement_pct = 0.0
    worst_regression_pct = 0.0

    for case in cases:
        label = case["label"]
        metric = case["metric"]
        weight = float(case["weight"])
        if label not in baseline_rows:
            raise SystemExit(f"Missing baseline row for {label} in {baseline_path}")
        if label not in candidate_rows:
            raise SystemExit(f"Missing candidate row for {label} in {candidate_path}")

        baseline_value = to_float(baseline_rows[label], metric)
        candidate_value = to_float(candidate_rows[label], metric)
        if baseline_value == 0.0:
            raise SystemExit(f"Baseline metric is zero for {label}:{metric}")
        delta = candidate_value - baseline_value
        pct = (delta / baseline_value) * 100.0
        improvement_pct = -pct
        worst_regression_pct = max(worst_regression_pct, max(0.0, pct))
        weighted_improvement_pct += improvement_pct * weight
        total_weight += weight
        comparisons.append(
            {
                "label": label,
                "metric": metric,
                "weight": weight,
                "baseline": baseline_value,
                "candidate": candidate_value,
                "delta": delta,
                "pct": pct,
                "improvementPct": improvement_pct,
                "candidateStdev": to_float(candidate_rows[label], f"stdev_{metric}"),
            }
        )

    weighted_improvement_pct = (
        weighted_improvement_pct / total_weight if total_weight > 0.0 else 0.0
    )
    accepted = (
        worst_regression_pct <= args.max_regression_pct
        and weighted_improvement_pct >= args.min_weighted_improvement_pct
    )

    result = {
        "baseline": str(baseline_path),
        "candidate": str(candidate_path),
        "preset": args.preset,
        "comparisons": comparisons,
        "weightedImprovementPct": weighted_improvement_pct,
        "worstRegressionPct": worst_regression_pct,
        "maxRegressionPct": args.max_regression_pct,
        "minWeightedImprovementPct": args.min_weighted_improvement_pct,
        "accepted": accepted,
    }

    if args.as_json:
        print(json.dumps(result, indent=2))
    else:
        print(
            f"baseline={baseline_path} candidate={candidate_path} preset={args.preset} "
            f"weightedImprovementPct={weighted_improvement_pct:.3f} "
            f"worstRegressionPct={worst_regression_pct:.3f} accepted={int(accepted)}"
        )
        for item in comparisons:
            print(
                f"label={item['label']} metric={item['metric']} baseline={item['baseline']:.3f} "
                f"candidate={item['candidate']:.3f} delta={item['delta']:+.3f} "
                f"pct={item['pct']:+.3f}% stdev={item['candidateStdev']:.3f} weight={item['weight']:.3f}"
            )

    raise SystemExit(0 if accepted else 2)


if __name__ == "__main__":
    main()
