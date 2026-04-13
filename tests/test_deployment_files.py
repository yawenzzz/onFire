import unittest
from pathlib import Path


class DeploymentFilesTests(unittest.TestCase):
    def test_dockerfile_exists(self) -> None:
        text = Path('docker/Dockerfile').read_text()
        self.assertIn('python:3.9', text)
        self.assertIn('PYTHONPATH=polymarket_arb', text)

    def test_docker_compose_exists(self) -> None:
        text = Path('docker/docker-compose.shadow.yml').read_text()
        self.assertIn('polymarket-arb-shadow', text)
        self.assertIn('command:', text)

    def test_systemd_service_exists(self) -> None:
        text = Path('deploy/polymarket-arb-shadow.service').read_text()
        self.assertIn('ExecStart=', text)
        self.assertIn('WorkingDirectory=', text)
