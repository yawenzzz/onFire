import json
import subprocess
import tempfile
import unittest
from pathlib import Path


class CopytradeLatencyReportScriptTests(unittest.TestCase):
    def test_latency_report_script_renders_text_and_json(self) -> None:
        with tempfile.TemporaryDirectory(prefix="copytrade-latency-report-") as tmpdir:
            summary = Path(tmpdir) / "summary.txt"
            summary.write_text(
                "\n".join(
                    [
                        "user=0xwallet",
                        "status=submit_completed",
                        "latest_tx=0xtx",
                        "latest_activity_timestamp=1776736832",
                        "latest_activity_price=0.88",
                        "watch_started_at_unix_ms=1776756500106",
                        "watch_finished_at_unix_ms=1776756509965",
                        "watch_elapsed_ms=9859",
                        "leader_to_watch_finished_ms=19677965",
                        "gate_started_at_unix_ms=1776756519085",
                        "payload_build_started_at_unix_ms=1776756519093",
                        "order_built_at_unix_ms=1776756520817",
                        "order_build_elapsed_ms=1724",
                        "payload_ready_at_unix_ms=1776756520962",
                        "payload_prep_elapsed_ms=1869",
                        "leader_to_payload_ready_ms=19688962",
                        "follower_effective_price=0.91000091",
                        "price_gap=0.03000090",
                        "price_gap_bps=340.9193",
                        "adverse_price_gap_bps=340.9193",
                        "submit_started_at_unix_ms=1776756520963",
                        "submit_finished_at_unix_ms=1776756521766",
                        "submit_roundtrip_elapsed_ms=803",
                        "leader_to_submit_started_ms=19688963",
                        "leader_to_submit_finished_ms=19689766",
                    ]
                )
                + "\n"
            )

            text_result = subprocess.run(
                ["bash", "scripts/run_rust_copytrade_latency_report.sh", "--report", str(summary)],
                cwd=Path.cwd(),
                text=True,
                capture_output=True,
                check=True,
            )
            self.assertIn("== copytrade latency report ==", text_result.stdout)
            self.assertIn("capture_to_payload_ready_ms=10997", text_result.stdout)
            self.assertIn("capture_to_submit_finished_ms=11801", text_result.stdout)
            self.assertIn("adverse_price_gap_bps=340.9193", text_result.stdout)

            json_result = subprocess.run(
                [
                    "bash",
                    "scripts/run_rust_copytrade_latency_report.sh",
                    "--report",
                    str(summary),
                    "--json",
                ],
                cwd=Path.cwd(),
                text=True,
                capture_output=True,
                check=True,
            )
            payload = json.loads(json_result.stdout)
            self.assertEqual(payload["status"], "submit_completed")
            self.assertEqual(payload["pricing"]["price_gap_bps"], "340.9193")
            self.assertEqual(payload["payload"]["capture_to_payload_ready_ms"], "10997")

    def test_latency_report_script_keeps_watch_and_leader_fields_for_duplicate_skip(self) -> None:
        with tempfile.TemporaryDirectory(prefix="copytrade-latency-duplicate-") as tmpdir:
            summary = Path(tmpdir) / "summary.txt"
            summary.write_text(
                "\n".join(
                    [
                        "user=0xwallet",
                        "status=duplicate_tx_skipped",
                        "latest_tx=0xdup",
                        "latest_activity_timestamp=1776303488",
                        "latest_activity_price=0.5",
                        "watch_started_at_unix_ms=1776759584237",
                        "watch_finished_at_unix_ms=1776759584358",
                        "watch_elapsed_ms=121",
                        "leader_to_watch_finished_ms=456096358",
                        "submit_exit=not_run",
                    ]
                )
                + "\n"
            )

            text_result = subprocess.run(
                ["bash", "scripts/run_rust_copytrade_latency_report.sh", "--report", str(summary)],
                cwd=Path.cwd(),
                text=True,
                capture_output=True,
                check=True,
            )
            self.assertIn("status=duplicate_tx_skipped", text_result.stdout)
            self.assertIn("leader_timestamp=1776303488", text_result.stdout)
            self.assertIn("leader_price=0.5", text_result.stdout)
            self.assertIn("watch_elapsed_ms=121", text_result.stdout)
            self.assertIn("submit_roundtrip_elapsed_ms=", text_result.stdout)

            json_result = subprocess.run(
                [
                    "bash",
                    "scripts/run_rust_copytrade_latency_report.sh",
                    "--report",
                    str(summary),
                    "--json",
                ],
                cwd=Path.cwd(),
                text=True,
                capture_output=True,
                check=True,
            )
            payload = json.loads(json_result.stdout)
            self.assertEqual(payload["status"], "duplicate_tx_skipped")
            self.assertEqual(payload["leader"]["timestamp"], "1776303488")
            self.assertEqual(payload["leader"]["price"], "0.5")
            self.assertEqual(payload["watch"]["elapsed_ms"], "121")

    def test_latency_report_script_keeps_watch_and_leader_fields_for_no_new_activity_skip(self) -> None:
        with tempfile.TemporaryDirectory(prefix="copytrade-latency-no-new-activity-") as tmpdir:
            summary = Path(tmpdir) / "summary.txt"
            summary.write_text(
                "\n".join(
                    [
                        "user=0xwallet",
                        "status=no_new_activity_skipped",
                        "latest_tx=0xstale",
                        "latest_activity_timestamp=1776763674",
                        "latest_activity_price=0.06",
                        "watch_started_at_unix_ms=1776764669952",
                        "watch_finished_at_unix_ms=1776764692570",
                        "watch_elapsed_ms=22618",
                        "leader_to_watch_finished_ms=1018570",
                        "submit_exit=not_run",
                    ]
                )
                + "\n"
            )

            text_result = subprocess.run(
                ["bash", "scripts/run_rust_copytrade_latency_report.sh", "--report", str(summary)],
                cwd=Path.cwd(),
                text=True,
                capture_output=True,
                check=True,
            )
            self.assertIn("status=no_new_activity_skipped", text_result.stdout)
            self.assertIn("leader_timestamp=1776763674", text_result.stdout)
            self.assertIn("leader_price=0.06", text_result.stdout)
            self.assertIn("watch_elapsed_ms=22618", text_result.stdout)
            self.assertIn("submit_roundtrip_elapsed_ms=", text_result.stdout)

            json_result = subprocess.run(
                [
                    "bash",
                    "scripts/run_rust_copytrade_latency_report.sh",
                    "--report",
                    str(summary),
                    "--json",
                ],
                cwd=Path.cwd(),
                text=True,
                capture_output=True,
                check=True,
            )
            payload = json.loads(json_result.stdout)
            self.assertEqual(payload["status"], "no_new_activity_skipped")
            self.assertEqual(payload["leader"]["timestamp"], "1776763674")
            self.assertEqual(payload["leader"]["price"], "0.06")
            self.assertEqual(payload["watch"]["elapsed_ms"], "22618")
