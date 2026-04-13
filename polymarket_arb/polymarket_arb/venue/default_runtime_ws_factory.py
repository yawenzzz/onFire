from __future__ import annotations


def resolve_runtime_ws_factory(default_resolver, override_resolver=None):
    if override_resolver is not None:
        return override_resolver()
    return default_resolver()
