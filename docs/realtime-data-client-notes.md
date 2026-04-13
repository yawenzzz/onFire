# Real-time Data Client Notes

Polymarket also publishes a separate real-time streaming client repository:
- `Polymarket/real-time-data-client`

Observed subscription model:
- topic-based subscriptions
- examples include `activity/trades`, `comments/*`, and `clob_user/*`

Observed auth shape for `clob_user` subscriptions:
- `key`
- `secret`
- `passphrase`

This is a distinct surface from the `Markets WebSocket` API docs and should not be assumed equivalent without verification.
