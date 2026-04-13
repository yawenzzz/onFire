import tempfile
import unittest
from pathlib import Path

from polymarket_arb.auth.account_status import detect_account_status, load_pm_auth_from_root


class AccountStatusTests(unittest.TestCase):
    def test_returns_public_only_without_creds(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            status = detect_account_status(Path(tmp))
        self.assertEqual(status["mode"], "public-only")
        self.assertFalse(status["creds_present"])

    def test_detects_account_ready_from_env_file(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / ".env").write_text(
                "CLOB_API_KEY=k\n"
                "CLOB_SECRET=s\n"
                "CLOB_PASS_PHRASE=p\n"
            )
            status = detect_account_status(root, sdk_available=False)
        self.assertEqual(status["mode"], "account-ready")
        self.assertTrue(status["creds_present"])
        self.assertFalse(status["sdk_available"])
        self.assertIn("sdk", status["reason"])

    def test_detects_account_auth_ready_when_pm_headers_exist(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / ".env").write_text(
                "PM_ACCESS_KEY=kid\n"
                "PM_SIGNATURE=sig\n"
                "PM_TIMESTAMP=123\n"
            )
            status = detect_account_status(root, sdk_available=False)
        self.assertEqual(status["mode"], "account-auth-ready")

    def test_detects_account_ready_when_clob_creds_exist_with_sdk(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / ".env").write_text(
                "CLOB_API_KEY=k\n"
                "CLOB_SECRET=s\n"
                "CLOB_PASS_PHRASE=p\n"
            )
            status = detect_account_status(root, sdk_available=True)
        self.assertEqual(status["mode"], "account-ready")
        self.assertFalse(status["pm_auth_present"])

    def test_detects_account_auth_ready_when_long_lived_pm_keys_exist(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / ".env").write_text(
                "POLYMARKET_KEY_ID=key-123\n"
                "POLYMARKET_SECRET_KEY=AAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8fHx4dHBsaGRgXFhUUExIREA8ODQwLCgkIBwYFBAMCAQA=\n"
            )
            status = detect_account_status(root, sdk_available=False)
        self.assertEqual(status["mode"], "account-auth-ready")

    def test_load_pm_auth_from_root_generates_fresh_triplet_from_long_lived_keys(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / ".env.local").write_text(
                "POLYMARKET_KEY_ID=key-123\n"
                "POLYMARKET_SECRET_KEY=AAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8fHx4dHBsaGRgXFhUUExIREA8ODQwLCgkIBwYFBAMCAQA=\n"
            )

            auth = load_pm_auth_from_root(root)

            self.assertIsNotNone(auth)
            assert auth is not None
            self.assertEqual(auth["access_key"], "key-123")
            self.assertTrue(auth["signature"])
            self.assertTrue(auth["timestamp"])

    def test_load_pm_auth_from_root_falls_back_to_pm_triplet_when_long_lived_key_invalid(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / ".env.local").write_text(
                "POLYMARKET_KEY_ID=key-123\n"
                "POLYMARKET_SECRET_KEY=not-valid-base64\n"
                "PM_ACCESS_KEY=triplet-kid\n"
                "PM_SIGNATURE=triplet-sig\n"
                "PM_TIMESTAMP=123\n"
            )

            auth = load_pm_auth_from_root(root)

            self.assertIsNotNone(auth)
            assert auth is not None
            self.assertEqual(auth["access_key"], "triplet-kid")
            self.assertEqual(auth["signature"], "triplet-sig")
            self.assertEqual(auth["timestamp"], "123")
