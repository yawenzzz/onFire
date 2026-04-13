from __future__ import annotations


def build_ws_connect_kwargs(headers: dict | None, connect_signature: str) -> dict:
    kwargs = {}
    if 'proxy' in connect_signature:
        kwargs['proxy'] = None
    if not headers:
        return kwargs
    if 'additional_headers' in connect_signature:
        kwargs['additional_headers'] = headers
        return kwargs
    if 'extra_headers' in connect_signature:
        kwargs['extra_headers'] = headers
        return kwargs
    return kwargs
