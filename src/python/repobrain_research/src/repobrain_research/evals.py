from __future__ import annotations

from dataclasses import dataclass


@dataclass(slots=True)
class EvalResult:
    scenario_id: str
    model_profile: str
    baseline_score: float
    uplifted_score: float
    latency_ms: int


def uplift_delta(baseline_score: float, uplifted_score: float) -> float:
    return uplifted_score - baseline_score


def summarize_results(results: list[EvalResult]) -> dict[str, float]:
    if not results:
        return {
            "avg_baseline": 0.0,
            "avg_uplifted": 0.0,
            "avg_delta": 0.0,
            "avg_latency_ms": 0.0,
        }

    count = float(len(results))
    avg_baseline = sum(item.baseline_score for item in results) / count
    avg_uplifted = sum(item.uplifted_score for item in results) / count
    avg_delta = (
        sum(uplift_delta(item.baseline_score, item.uplifted_score) for item in results) / count
    )
    avg_latency_ms = sum(item.latency_ms for item in results) / count

    return {
        "avg_baseline": avg_baseline,
        "avg_uplifted": avg_uplifted,
        "avg_delta": avg_delta,
        "avg_latency_ms": avg_latency_ms,
    }
