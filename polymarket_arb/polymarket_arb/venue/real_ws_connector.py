from __future__ import annotations


async def iter_messages_from_source(source_factory, limit: int):
    async with source_factory() as source:
        count = 0
        while count < limit:
            try:
                msg = await source.recv()
            except StopAsyncIteration:
                break
            yield msg
            count += 1
