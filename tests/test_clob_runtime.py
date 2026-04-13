import os
import tempfile
import unittest
from pathlib import Path

from polymarket_arb.auth.clob_runtime import detect_clob_runtime_status, load_clob_runtime_config


class ClobRuntimeTests(unittest.TestCase):
    def test_loads_runtime_config_from_env_file(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / ".env").write_text(
                "CLOB_PRIVATE_KEY=70ea107442a101d468635d4cae11452292d4f1d5c355caa2c458c2c1e56238fa\n"
                "CLOB_API_KEY=key\n"
                "CLOB_SECRET=sec\n"
                "CLOB_PASS_PHRASE=pass\n"
                "SIGNATURE_TYPE=2\n"
                "FUNDER_ADDRESS=0xabc\n"
            )

            cfg = load_clob_runtime_config(root)

        self.assertIsNotNone(cfg)
        assert cfg is not None
        self.assertEqual(cfg.private_key, "70ea107442a101d468635d4cae11452292d4f1d5c355caa2c458c2c1e56238fa")
        self.assertEqual(cfg.api_creds.api_key, "key")
        self.assertEqual(cfg.host, "https://clob.polymarket.com")
        self.assertEqual(cfg.chain_id, 137)
        self.assertEqual(cfg.signature_type, 2)
        self.assertEqual(cfg.funder, "0xabc")
        self.assertEqual(cfg.signer_address, "0x11084005d88a0840b5f38f8731cca9152bbd99f7")

    def test_detect_status_requires_private_key_for_level2(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / ".env").write_text(
                "CLOB_API_KEY=key\n"
                "CLOB_SECRET=sec\n"
                "CLOB_PASS_PHRASE=pass\n"
            )
            status = detect_clob_runtime_status(root)

        self.assertEqual(status["mode"], "account-ready")
        self.assertIn("missing private key", status["reason"])
        self.assertIn("PRIVATE_KEY", status["reason"])
        self.assertEqual(status["signature_type"], 0)
        self.assertFalse(status["funder_present"])
        self.assertIsNone(status["signer_address"])
