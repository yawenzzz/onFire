from __future__ import annotations

import json
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

from polymarket_arb.ops.metrics_http_server import build_status_payload


def start_status_server(root: str | Path, host: str = '127.0.0.1', port: int = 0):
    root = Path(root)

    class Handler(BaseHTTPRequestHandler):
        def do_GET(self):  # noqa: N802
            if self.path != '/status':
                self.send_response(404)
                self.end_headers()
                return
            payload = build_status_payload(root)
            body = json.dumps(payload).encode('utf-8')
            self.send_response(200)
            self.send_header('Content-Type', 'application/json')
            self.send_header('Content-Length', str(len(body)))
            self.end_headers()
            self.wfile.write(body)

        def log_message(self, format, *args):
            return

    server = ThreadingHTTPServer((host, port), Handler)
    import threading
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    return server
