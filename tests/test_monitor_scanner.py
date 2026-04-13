import unittest

from polymarket_arb.monitoring.scanner import GammaScanner, RealtimeGammaScanner, scan_complete_bucket_events


class StubClient:
    def __init__(self, events: list[dict]) -> None:
        self.events = events
        self.calls = []
        self.book_calls = []

    def fetch_events(self, limit: int = 50, closed: str = "false", offset: int = 0):
        self.calls.append((limit, closed, offset))
        return self.events[offset: offset + limit]

    def fetch_book(self, token_id: str):
        self.book_calls.append(token_id)
        return {
            "bids": [{"price": "0.11", "size": "1000"}, {"price": "0.10", "size": "500"}],
            "asks": [{"price": "0.12", "size": "700"}, {"price": "0.13", "size": "600"}],
        }


class StubWSClient:
    def __init__(self, messages):
        self.messages = list(messages)

    async def iter_messages(self, limit: int):
        count = 0
        while self.messages and count < limit:
            yield self.messages.pop(0)
            count += 1

    def drain_messages(self):
        msgs = list(self.messages)
        self.messages.clear()
        return msgs


class MonitorScannerTests(unittest.TestCase):
    def test_scan_returns_candidates_rejections_and_stats(self) -> None:
        client = StubClient(
            [
                {
                    "slug": "fannie-mae-ipo-closing-market-cap",
                    "title": "Fannie Mae IPO Closing Market Cap",
                    "resolutionSource": "https://example.com/fannie",
                    "tags": [{"label": "Finance"}],
                    "showAllOutcomes": True,
                    "volume24hr": 1234.5,
                    "liquidity": 9876.5,
                    "markets": [
                        {"question": "Will Fannie Mae's market cap be less than $200B at market close on IPO day?", "bestBid": 0.002, "bestAsk": 0.003, "lastTradePrice": 0.003, "spread": 0.001, "volume24hr": 100.0, "liquidity": 1000.0, "orderMinSize": 5, "orderPriceMinTickSize": 0.001, "clobTokenIds": '[\"123\"]', "active": True, "closed": False},
                        {"question": "Will Fannie Mae's market cap be between $200B and $250B at market close on IPO day?", "bestBid": 0.002, "bestAsk": 0.003, "lastTradePrice": 0.003, "spread": 0.001, "volume24hr": 100.0, "liquidity": 1000.0, "orderMinSize": 5, "orderPriceMinTickSize": 0.001, "clobTokenIds": '[\"123\"]', "active": True, "closed": False},
                        {"question": "Will Fannie Mae's market cap be between $250B and $300B at market close on IPO day?", "bestBid": 0.002, "bestAsk": 0.003, "lastTradePrice": 0.003, "spread": 0.001, "volume24hr": 100.0, "liquidity": 1000.0, "orderMinSize": 5, "orderPriceMinTickSize": 0.001, "clobTokenIds": '[\"123\"]', "active": True, "closed": False},
                        {"question": "Will Fannie Mae's market cap be between $300B and $350B at market close on IPO day?", "bestBid": 0.003, "bestAsk": 0.004, "lastTradePrice": 0.004, "spread": 0.001, "volume24hr": 100.0, "liquidity": 1000.0, "orderMinSize": 5, "orderPriceMinTickSize": 0.001, "clobTokenIds": '[\"123\"]', "active": True, "closed": False},
                        {"question": "Will Fannie Mae's market cap be between $350B and $400B at market close on IPO day?", "bestBid": 0.004, "bestAsk": 0.005, "lastTradePrice": 0.005, "spread": 0.001, "volume24hr": 100.0, "liquidity": 1000.0, "orderMinSize": 5, "orderPriceMinTickSize": 0.001, "clobTokenIds": '[\"123\"]', "active": True, "closed": False},
                        {"question": "Will Fannie Mae's market cap be $400B or greater at market close on IPO day?", "bestBid": 0.002, "bestAsk": 0.003, "lastTradePrice": 0.003, "spread": 0.001, "volume24hr": 100.0, "liquidity": 1000.0, "orderMinSize": 5, "orderPriceMinTickSize": 0.001, "clobTokenIds": '[\"123\"]', "active": True, "closed": False},
                        {"question": "Will Fannie Mae not IPO by June 30, 2026?", "bestBid": 0.977, "bestAsk": 0.978, "lastTradePrice": 0.978, "spread": 0.001, "volume24hr": 100.0, "liquidity": 1000.0, "orderMinSize": 5, "orderPriceMinTickSize": 0.001, "clobTokenIds": '[\"123\"]', "active": True, "closed": False},
                    ],
                },
                {
                    "slug": "consensys-ipo-closing-market-cap-above",
                    "title": "Consensys IPO closing market cap above ___ ?",
                    "showAllOutcomes": True,
                    "markets": [
                        {"question": "Consensys IPO closing market cap above $1B?", "bestAsk": 0.28, "active": True, "closed": False},
                        {"question": "Consensys IPO closing market cap above $2B?", "bestAsk": 0.27, "active": True, "closed": False},
                        {"question": "Consensys IPO closing market cap above $3B?", "bestAsk": 0.15, "active": True, "closed": False},
                    ],
                },
            ]
        )

        snapshot = scan_complete_bucket_events(client, limit=50, min_edge=0.03)

        self.assertEqual(client.calls, [(50, "false", 0)])
        self.assertEqual(snapshot.event_count, 2)
        self.assertEqual(snapshot.complete_event_count, 1)
        self.assertEqual(snapshot.rejection_counts["overlapping_thresholds"], 1)
        self.assertEqual(len(snapshot.candidates), 0)
        self.assertEqual(len(snapshot.rejections), 2)
        self.assertAlmostEqual(snapshot.best_gross_edge, 0.001)
        self.assertEqual(snapshot.rejections[0].best_bid, 0.002)
        self.assertEqual(snapshot.rejections[0].best_ask, 0.003)
        self.assertEqual(snapshot.rejections[0].volume_24hr, 100.0)
        self.assertEqual(snapshot.rejections[0].depth_bid_top, 1000.0)
        self.assertEqual(snapshot.rejections[0].depth_ask_top, 700.0)
        self.assertEqual(snapshot.rejections[0].event_url, "https://polymarket.com/event/fannie-mae-ipo-closing-market-cap")

    def test_scan_limits_book_depth_fetches_to_depth_window(self) -> None:
        events = []
        for index in range(6):
            events.append(
                {
                    "slug": f"event-{index}",
                    "title": f"Event {index}",
                    "showAllOutcomes": True,
                    "tags": [{"label": "Politics"}],
                    "markets": [
                        {
                            "question": f"Will item {index} happen by June 30, 2026?",
                            "bestBid": 0.1,
                            "bestAsk": 0.2,
                            "lastTradePrice": 0.15,
                            "spread": 0.1,
                            "volume24hr": 10.0,
                            "liquidity": 100.0,
                            "orderMinSize": 5,
                            "orderPriceMinTickSize": 0.01,
                            "clobTokenIds": f'[\"token-{index}\"]',
                            "active": True,
                            "closed": False,
                        }
                    ],
                }
            )
        client = StubClient(events)

        scan_complete_bucket_events(client, limit=6, min_edge=0.03, depth_window=3)

        self.assertEqual(client.book_calls, ["token-0", "token-1", "token-2"])

    def test_scan_paginates_until_limit_or_exhaustion(self) -> None:
        events = []
        for index in range(120):
            events.append(
                {
                    "slug": f"event-{index}",
                    "title": f"How many things happen {index}",
                    "showAllOutcomes": True,
                    "tags": [{"label": "Politics"}],
                    "markets": [
                        {
                            "question": f"How many things happen {index}? no one",
                            "bestBid": 0.1,
                            "bestAsk": 0.2,
                            "lastTradePrice": 0.15,
                            "spread": 0.1,
                            "volume24hr": 10.0,
                            "liquidity": 100.0,
                            "orderMinSize": 5,
                            "orderPriceMinTickSize": 0.01,
                            "clobTokenIds": f'[\"token-{index}\"]',
                            "active": True,
                            "closed": False,
                        },
                        {
                            "question": f"How many things happen {index}? or more",
                            "bestBid": 0.1,
                            "bestAsk": 0.2,
                            "lastTradePrice": 0.15,
                            "spread": 0.1,
                            "volume24hr": 10.0,
                            "liquidity": 100.0,
                            "orderMinSize": 5,
                            "orderPriceMinTickSize": 0.01,
                            "clobTokenIds": f'[\"token-{index}-b\"]',
                            "active": True,
                            "closed": False,
                        },
                    ],
                }
            )
        client = StubClient(events)

        snapshot = scan_complete_bucket_events(client, limit=120, min_edge=0.03, depth_window=1)

        self.assertEqual(snapshot.event_count, 120)
        self.assertEqual(snapshot.page_count, 2)
        self.assertEqual(snapshot.scan_limit, 120)
        self.assertEqual(set(client.calls), {(100, "false", 0), (20, "false", 100)})

    def test_gamma_scanner_scan_once_honors_configured_limit(self) -> None:
        events = []
        for index in range(300):
            events.append(
                {
                    "slug": f"event-{index}",
                    "title": f"How many things happen {index}",
                    "showAllOutcomes": True,
                    "tags": [{"label": "Politics"}],
                    "markets": [
                        {
                            "question": f"How many things happen {index}? no one",
                            "bestAsk": 0.2,
                            "active": True,
                            "closed": False,
                        },
                        {
                            "question": f"How many things happen {index}? or more",
                            "bestAsk": 0.2,
                            "active": True,
                            "closed": False,
                        },
                    ],
                }
            )
        client = StubClient(events)
        scanner = GammaScanner(client=client, min_edge_threshold=0.03, limit=250)

        snapshot = scanner.scan_once()

        self.assertEqual(snapshot.event_count, 250)
        self.assertEqual(snapshot.scan_limit, 250)
        self.assertEqual(snapshot.window_limit, 250)
        self.assertEqual(set(client.calls), {(100, "false", 0), (100, "false", 100), (50, "false", 200)})

    def test_gamma_scanner_scan_once_honors_configured_depth_window(self) -> None:
        events = []
        for index in range(6):
            events.append(
                {
                    "slug": f"event-{index}",
                    "title": f"How many things happen {index}",
                    "showAllOutcomes": True,
                    "tags": [{"label": "Politics"}],
                    "markets": [
                        {
                            "question": f"How many things happen {index}? no one",
                            "bestAsk": 0.2,
                            "clobTokenIds": f'[\"token-{index}\"]',
                            "active": True,
                            "closed": False,
                        },
                        {
                            "question": f"How many things happen {index}? or more",
                            "bestAsk": 0.2,
                            "clobTokenIds": f'[\"token-{index}-b\"]',
                            "active": True,
                            "closed": False,
                        },
                    ],
                }
            )
        client = StubClient(events)
        scanner = GammaScanner(client=client, min_edge_threshold=0.01, limit=6, depth_window=1)

        scanner.scan_once()

        self.assertEqual(len(client.book_calls), 1)

    def test_gamma_scanner_scan_once_alternates_full_and_hot_windows(self) -> None:
        events = []
        for index in range(300):
            events.append(
                {
                    "slug": f"event-{index}",
                    "title": f"How many things happen {index}",
                    "showAllOutcomes": True,
                    "tags": [{"label": "Politics"}],
                    "markets": [
                        {"question": f"How many things happen {index}? no one", "bestAsk": 0.2, "active": True, "closed": False},
                        {"question": f"How many things happen {index}? or more", "bestAsk": 0.2, "active": True, "closed": False},
                    ],
                }
            )
        client = StubClient(events)
        scanner = GammaScanner(client=client, min_edge_threshold=0.01, limit=250, hot_limit=100, full_scan_every=2)

        first = scanner.scan_once()
        second = scanner.scan_once()
        third = scanner.scan_once()

        self.assertEqual((first.scan_mode, first.scan_limit, first.window_limit, first.event_count, first.scan_offset), ("full", 250, 250, 250, 0))
        self.assertEqual((second.scan_mode, second.scan_limit, second.window_limit, second.event_count, second.scan_offset), ("hot", 250, 100, 100, 0))
        self.assertEqual((third.scan_mode, third.scan_limit, third.window_limit, third.event_count, third.scan_offset), ("full", 250, 250, 250, 0))

    def test_gamma_scanner_hot_windows_rotate_across_full_range(self) -> None:
        events = []
        for index in range(500):
            events.append(
                {
                    "slug": f"event-{index}",
                    "title": f"How many things happen {index}",
                    "showAllOutcomes": True,
                    "tags": [{"label": "Politics"}],
                    "markets": [
                        {"question": f"How many things happen {index}? no one", "bestAsk": 0.2, "active": True, "closed": False},
                        {"question": f"How many things happen {index}? or more", "bestAsk": 0.2, "active": True, "closed": False},
                    ],
                }
            )
        client = StubClient(events)
        scanner = GammaScanner(client=client, min_edge_threshold=0.01, limit=500, hot_limit=200, full_scan_every=5)

        offsets = [scanner.scan_once().scan_offset for _ in range(5)]

        self.assertEqual(offsets, [0, 0, 200, 400, 0])

    def test_gamma_scanner_hot_snapshot_carries_last_full_reference_summary(self) -> None:
        events = []
        for index in range(500):
            ask = 0.6
            if index == 0:
                ask = 0.48
            events.append(
                {
                    "slug": f"event-{index}",
                    "title": f"How many things happen {index}",
                    "showAllOutcomes": True,
                    "tags": [{"label": "Politics"}],
                    "markets": [
                        {"question": f"How many things happen {index}? no one", "bestAsk": ask, "active": True, "closed": False},
                        {"question": f"How many things happen {index}? or more", "bestAsk": ask, "active": True, "closed": False},
                    ],
                }
            )
        client = StubClient(events)
        scanner = GammaScanner(client=client, min_edge_threshold=0.01, limit=500, hot_limit=200, full_scan_every=5)

        full_snapshot = scanner.scan_once()
        hot_snapshot = scanner.scan_once()

        self.assertEqual(full_snapshot.candidate_count, 1)
        self.assertEqual(hot_snapshot.scan_mode, "hot")
        self.assertEqual(hot_snapshot.candidate_count, 1)
        self.assertEqual(hot_snapshot.reference_candidate_count, 1)
        self.assertAlmostEqual(hot_snapshot.reference_best_gross_edge, full_snapshot.best_gross_edge)
        self.assertEqual(hot_snapshot.scan_offset, 0)

    def test_realtime_gamma_scanner_uses_bootstrap_then_ws_updates_without_refetch(self) -> None:
        client = StubClient(
            [
                {
                    "slug": "event-1",
                    "title": "How many things happen 1",
                    "showAllOutcomes": True,
                    "tags": [{"label": "Politics"}],
                    "markets": [
                        {"slug": "m1a", "question": "How many things happen 1? no one", "bestAsk": 0.60, "bestBid": 0.50, "active": True, "closed": False},
                        {"slug": "m1b", "question": "How many things happen 1? or more", "bestAsk": 0.60, "bestBid": 0.50, "active": True, "closed": False},
                    ],
                }
            ]
        )
        ws = StubWSClient([
            {"market_id": "m1a", "market_state": "OPEN", "best_bid": 0.2, "best_ask": 0.20},
            {"market_id": "m1b", "market_state": "OPEN", "best_bid": 0.2, "best_ask": 0.20},
        ])
        scanner = RealtimeGammaScanner(client=client, min_edge_threshold=0.01, limit=1, hot_limit=1, ws_client=ws)

        first = scanner.scan_once()
        second = scanner.scan_once()

        self.assertEqual(len(client.calls), 1)
        self.assertEqual(first.data_source, "gamma-ws")
        self.assertEqual(first.candidate_count, 0)
        self.assertEqual(second.candidate_count, 1)

    def test_realtime_gamma_scanner_tracks_event_repeat_interval_ms(self) -> None:
        client = StubClient(
            [
                {
                    "slug": "event-1",
                    "title": "How many things happen 1",
                    "showAllOutcomes": True,
                    "tags": [{"label": "Politics"}],
                    "markets": [
                        {"slug": "m1a", "question": "How many things happen 1? no one", "bestAsk": 0.20, "bestBid": 0.10, "active": True, "closed": False},
                        {"slug": "m1b", "question": "How many things happen 1? or more", "bestAsk": 0.20, "bestBid": 0.10, "active": True, "closed": False},
                    ],
                }
            ]
        )
        ws = StubWSClient([
            {"market_id": "m1a", "market_state": "OPEN", "best_bid": 0.2, "best_ask": 0.20},
            {"market_id": "m1a", "market_state": "OPEN", "best_bid": 0.21, "best_ask": 0.21},
        ])
        scanner = RealtimeGammaScanner(client=client, min_edge_threshold=0.01, limit=1, hot_limit=1, ws_client=ws)

        scanner.scan_once()
        second = scanner.scan_once()

        self.assertIsNotNone(second.candidates[0].repeat_interval_ms)

    def test_realtime_gamma_scanner_reports_missing_ws_auth_cleanly(self) -> None:
        client = StubClient(
            [
                {
                    "slug": "event-1",
                    "title": "How many things happen 1",
                    "showAllOutcomes": True,
                    "tags": [{"label": "Politics"}],
                    "markets": [
                        {"slug": "m1a", "question": "How many things happen 1? no one", "bestAsk": 0.20, "bestBid": 0.10, "active": True, "closed": False},
                        {"slug": "m1b", "question": "How many things happen 1? or more", "bestAsk": 0.20, "bestBid": 0.10, "active": True, "closed": False},
                    ],
                }
            ]
        )
        scanner = RealtimeGammaScanner(client=client, min_edge_threshold=0.01, limit=1, hot_limit=1, ws_client=None)
        scanner._ws_setup_error = "missing websocket auth (set POLYMARKET_KEY_ID/POLYMARKET_SECRET_KEY or fresh PM_*)"
        scanner._stream.ws_client = None

        snapshot = scanner.scan_once()

        self.assertEqual(snapshot.realtime_status, "error")
        self.assertIn("missing websocket auth", snapshot.realtime_reason)

    def test_realtime_gamma_scanner_rejects_stale_ws_timestamp(self) -> None:
        client = StubClient(
            [
                {
                    "slug": "event-1",
                    "title": "How many things happen 1",
                    "showAllOutcomes": True,
                    "tags": [{"label": "Politics"}],
                    "markets": [
                        {"slug": "m1a", "question": "How many things happen 1? no one", "bestAsk": 0.20, "bestBid": 0.10, "active": True, "closed": False},
                        {"slug": "m1b", "question": "How many things happen 1? or more", "bestAsk": 0.20, "bestBid": 0.10, "active": True, "closed": False},
                    ],
                }
            ]
        )
        scanner = RealtimeGammaScanner(client=client, min_edge_threshold=0.01, limit=1, hot_limit=1, ws_client=None)
        scanner._ws_setup_error = "stale websocket auth timestamp (60000ms old)"
        scanner._stream.ws_client = None

        snapshot = scanner.scan_once()

        self.assertEqual(snapshot.realtime_status, "error")
        self.assertIn("stale websocket auth timestamp", snapshot.realtime_reason)

    def test_realtime_gamma_scanner_reports_tls_probe_reason_cleanly(self) -> None:
        client = StubClient(
            [
                {
                    "slug": "event-1",
                    "title": "How many things happen 1",
                    "showAllOutcomes": True,
                    "tags": [{"label": "Politics"}],
                    "markets": [
                        {"slug": "m1a", "question": "How many things happen 1? no one", "bestAsk": 0.20, "bestBid": 0.10, "active": True, "closed": False},
                        {"slug": "m1b", "question": "How many things happen 1? or more", "bestAsk": 0.20, "bestBid": 0.10, "active": True, "closed": False},
                    ],
                }
            ]
        )
        scanner = RealtimeGammaScanner(
            client=client,
            min_edge_threshold=0.01,
            limit=1,
            hot_limit=1,
            ws_client=None,
            transport_probe=lambda _url: (False, "tls_err: ConnectionResetError: [Errno 54] Connection reset by peer"),
        )

        snapshot = scanner.scan_once()

        self.assertEqual(snapshot.realtime_status, "error")
        self.assertIn("tls_err", snapshot.realtime_reason)
