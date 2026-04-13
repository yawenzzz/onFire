from __future__ import annotations


class WebsocketsLibAdapter:
    def __init__(self, connect_impl) -> None:
        self.connect_impl = connect_impl

    def connect(self, url: str):
        return self.connect_impl(url)
