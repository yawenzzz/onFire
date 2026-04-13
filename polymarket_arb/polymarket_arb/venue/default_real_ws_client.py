from __future__ import annotations

import inspect

from polymarket_arb.venue.default_ws_connect import resolve_connect_factory
from polymarket_arb.venue.real_capture_factory import build_async_ws_client
from polymarket_arb.venue.ws_connect_kwargs import build_ws_connect_kwargs


def build_default_ws_client(
    url: str,
    optional_factory_resolver,
    connect_factory=None,
    headers: dict | None = None,
    connect_signature: str | None = None,
):
    factory = connect_factory if connect_factory is not None else resolve_connect_factory(optional_factory_resolver)
    if factory is None:
        return None
    signature_text = connect_signature or str(inspect.signature(factory))
    kwargs = build_ws_connect_kwargs(headers, signature_text)
    if kwargs:
        if connect_signature is not None:
            try:
                factory(url, **kwargs)
            except Exception:
                pass
        wrapped = lambda ws_url: factory(ws_url, **kwargs)
        return build_async_ws_client(url, wrapped)
    return build_async_ws_client(url, factory)
