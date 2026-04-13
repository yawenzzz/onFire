from __future__ import annotations

from pathlib import Path


def upsert_env_file(path: str | Path, values: dict[str, str]) -> None:
    env_path = Path(path)
    lines = env_path.read_text().splitlines() if env_path.exists() else []
    pending = dict(values)
    updated_lines: list[str] = []

    for line in lines:
        stripped = line.strip()
        if not stripped or stripped.startswith("#") or "=" not in line:
            updated_lines.append(line)
            continue

        key, _ = line.split("=", 1)
        key = key.strip()
        if key in pending:
            updated_lines.append(f"{key}={pending.pop(key)}")
            continue

        updated_lines.append(line)

    for key, value in pending.items():
        updated_lines.append(f"{key}={value}")

    rendered = "\n".join(updated_lines)
    if updated_lines:
        rendered += "\n"
    env_path.write_text(rendered)
