import asyncio
import unittest

from polymarket_arb.venue.reconnect_loop import run_reconnect_loop_once


class ReconnectLoopTests(unittest.TestCase):
    def test_returns_success_on_first_attempt(self) -> None:
        async def run_test():
            attempts = []

            async def runner():
                attempts.append('run')
                return 'ok'

            result = await run_reconnect_loop_once(runner, max_attempts=3, sleep_impl=lambda _: None)
            self.assertEqual(result, 'ok')
            self.assertEqual(len(attempts), 1)
        asyncio.run(run_test())

    def test_retries_until_success(self) -> None:
        async def run_test():
            state = {'n': 0}

            async def runner():
                state['n'] += 1
                if state['n'] < 3:
                    raise RuntimeError('temporary')
                return 'ok'

            sleeps = []
            async def sleep_impl(seconds):
                sleeps.append(seconds)

            result = await run_reconnect_loop_once(runner, max_attempts=3, sleep_impl=sleep_impl)
            self.assertEqual(result, 'ok')
            self.assertEqual(state['n'], 3)
            self.assertEqual(len(sleeps), 2)
        asyncio.run(run_test())
