from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path

from polymarket_arb.auth.account_status import _read_env_file
from polymarket_arb.auth.clob_runtime import load_clob_runtime_config


@dataclass
class ManualOrderConfig:
    token_id: str
    side: str
    price: float
    size: float
    order_type: str = "GTC"


@dataclass
class ManualOrderResult:
    success: bool
    reason: str
    order_id: str | None = None


def load_manual_order_config(root: str | Path) -> ManualOrderConfig | None:
    root = Path(root)
    env = _read_env_file(root / ".env")
    env.update(_read_env_file(root / ".env.local"))
    token_id = env.get("ORDER_TOKEN_ID")
    side = env.get("ORDER_SIDE")
    price = env.get("ORDER_PRICE")
    size = env.get("ORDER_SIZE")
    if not token_id or not side or not price or not size:
        return None
    try:
        return ManualOrderConfig(
            token_id=token_id,
            side=side.upper(),
            price=float(price),
            size=float(size),
            order_type=(env.get("ORDER_TYPE") or "GTC").upper(),
        )
    except (TypeError, ValueError):
        return None


def submit_manual_order(
    config: ManualOrderConfig | None,
    root: str | Path = ".",
    client_factory=None,
) -> ManualOrderResult:
    if config is None:
        return ManualOrderResult(success=False, reason="manual order draft missing")

    if isinstance(config, dict) and config.get("legs"):
        if client_factory is None:
            runtime = load_clob_runtime_config(root)
            if runtime is None:
                return ManualOrderResult(success=False, reason="clob runtime config missing")

            from py_clob_client.client import ClobClient
            from py_clob_client.clob_types import ApiCreds

            def client_factory():
                return ClobClient(
                    runtime.host,
                    chain_id=runtime.chain_id,
                    key=runtime.private_key,
                    creds=ApiCreds(
                        runtime.api_creds.api_key,
                        runtime.api_creds.api_secret,
                        runtime.api_creds.api_passphrase,
                    ),
                    signature_type=runtime.signature_type,
                    funder=runtime.funder,
                )
        try:
            client = client_factory()
            from py_clob_client.clob_types import OrderArgs, OrderType, PostOrdersArgs

            signed = []
            for leg in config.get("legs", []):
                signed_order = client.create_order(
                    OrderArgs(
                        token_id=leg["token_id"],
                        side=leg["side"],
                        price=float(leg["price"]),
                        size=float(leg["size"]),
                    )
                )
                signed.append(PostOrdersArgs(order=signed_order, orderType=getattr(OrderType, config.get("order_type", "GTC"), config.get("order_type", "GTC"))))
            response = client.post_orders(signed)
            order_id = None
            if isinstance(response, list) and response:
                first = response[0]
                if isinstance(first, dict):
                    order_id = first.get("orderID") or first.get("id")
            return ManualOrderResult(success=True, reason="submitted", order_id=order_id)
        except Exception as exc:
            return ManualOrderResult(success=False, reason=f"{type(exc).__name__}: {exc}")

    if client_factory is None:
        runtime = load_clob_runtime_config(root)
        if runtime is None:
            return ManualOrderResult(success=False, reason="clob runtime config missing")

        from py_clob_client.client import ClobClient
        from py_clob_client.clob_types import ApiCreds

        def client_factory():
            return ClobClient(
                runtime.host,
                chain_id=runtime.chain_id,
                key=runtime.private_key,
                creds=ApiCreds(
                    runtime.api_creds.api_key,
                    runtime.api_creds.api_secret,
                    runtime.api_creds.api_passphrase,
                ),
                signature_type=runtime.signature_type,
                funder=runtime.funder,
            )

    try:
        client = client_factory()
        from py_clob_client.clob_types import OrderArgs, OrderType

        order = client.create_order(
            OrderArgs(
                token_id=config.token_id,
                side=config.side,
                price=config.price,
                size=config.size,
            )
        )
        response = client.post_order(order, orderType=getattr(OrderType, config.order_type, config.order_type))
        order_id = None
        if isinstance(response, dict):
            order_id = response.get("orderID") or response.get("id")
        return ManualOrderResult(success=True, reason="submitted", order_id=order_id)
    except Exception as exc:
        return ManualOrderResult(success=False, reason=f"{type(exc).__name__}: {exc}")
