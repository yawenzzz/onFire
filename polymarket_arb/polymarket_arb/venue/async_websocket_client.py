from __future__ import annotations


class AsyncWebSocketClient:
    def __init__(self, url: str, connect_impl) -> None:
        self.url = url
        self.connect_impl = connect_impl

    async def iter_messages(self, limit: int):
        source = self.connect_impl(self.url)

        if hasattr(source, '__aenter__') and hasattr(source, '__aexit__'):
            async with source as conn:
                count = 0
                while count < limit:
                    try:
                        msg = await conn.recv()
                    except StopAsyncIteration:
                        break
                    yield msg
                    count += 1
            return

        for idx, msg in enumerate(source):
            if idx >= limit:
                break
            yield msg
