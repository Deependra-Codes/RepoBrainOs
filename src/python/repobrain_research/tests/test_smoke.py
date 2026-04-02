import unittest

from repobrain_research.evals import EvalResult, summarize_results, uplift_delta


class EvalHelpersTest(unittest.TestCase):
    def test_uplift_delta(self) -> None:
        self.assertAlmostEqual(uplift_delta(0.4, 0.7), 0.3)

    def test_summary(self) -> None:
        summary = summarize_results(
            [
                EvalResult("s1", "weak_local_coder", 0.4, 0.6, 120),
                EvalResult("s2", "medium_cloud_coder", 0.5, 0.8, 180),
            ]
        )

        self.assertAlmostEqual(summary["avg_baseline"], 0.45)
        self.assertAlmostEqual(summary["avg_uplifted"], 0.7)
        self.assertAlmostEqual(summary["avg_delta"], 0.25)
        self.assertAlmostEqual(summary["avg_latency_ms"], 150.0)


if __name__ == "__main__":
    unittest.main()
