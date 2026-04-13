import tempfile
import unittest
from pathlib import Path

from polymarket_arb.app.status_server_cli import main


class StatusServerCliTests(unittest.TestCase):
    def test_cli_starts_and_stops_server_once(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            code = main(['--root', str(root), '--host', '127.0.0.1', '--port', '0', '--once'])
            self.assertEqual(code, 0)
