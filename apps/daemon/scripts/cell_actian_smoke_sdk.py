#!/usr/bin/env python3
"""Actian VectorAI gRPC smoke steps for Lattice cell_actian_smoke.

Invoked by lattice-daemon; prints JSON to stdout:
  {"steps": [{"name": "...", "ok": true, "detail": "..."}, ...]}
"""

from __future__ import annotations

import json
import sys


def main() -> int:
    if len(sys.argv) < 4:
        print(
            "usage: cell_actian_smoke_sdk.py <host:port> <collection> <dimension>",
            file=sys.stderr,
        )
        return 2

    endpoint = sys.argv[1]
    collection = sys.argv[2]
    dimension = int(sys.argv[3])
    steps: list[dict[str, object]] = []

    try:
        from actian_vectorai import Distance, PointStruct, VectorAIClient, VectorParams
    except ImportError as exc:
        print(json.dumps({"steps": []}))
        print(f"actian_vectorai import failed: {exc}", file=sys.stderr)
        return 1

    try:
        with VectorAIClient(endpoint, timeout=30.0) as client:
            info = client.health_check()
            steps.append(
                {
                    "name": "health_check",
                    "ok": True,
                    "detail": f"{info.get('title', 'VectorAI')} v{info.get('version', '?')}",
                }
            )

            if client.collections.exists(collection):
                client.collections.delete(collection)
            client.collections.create(
                collection,
                vectors_config=VectorParams(size=dimension, distance=Distance.Cosine),
            )
            steps.append(
                {
                    "name": "create_collection",
                    "ok": True,
                    "detail": f"{collection} ({dimension}-d cosine)",
                }
            )

            vector_a = [0.1] * dimension
            vector_b = [0.2] * dimension
            client.points.upsert(
                collection,
                [
                    PointStruct(id=1, vector=vector_a, payload={"smoke": "a"}),
                    PointStruct(id=2, vector=vector_b, payload={"smoke": "b"}),
                ],
            )
            steps.append(
                {
                    "name": "upsert",
                    "ok": True,
                    "detail": "2 points",
                }
            )

            results = client.points.search(collection, vector=vector_a, limit=5)
            hit = len(results) > 0 and getattr(results[0], "id", None) == 1
            steps.append(
                {
                    "name": "search",
                    "ok": hit,
                    "detail": f"{len(results)} hit(s), top id={getattr(results[0], 'id', None) if results else None}",
                }
            )
    except Exception as exc:  # noqa: BLE001 — surfaced to Rust caller via stderr
        steps.append(
            {
                "name": "sdk_error",
                "ok": False,
                "detail": str(exc),
            }
        )
        print(json.dumps({"steps": steps}))
        print(str(exc), file=sys.stderr)
        return 1

    print(json.dumps({"steps": steps}))
    return 0 if all(step["ok"] for step in steps) else 1


if __name__ == "__main__":
    raise SystemExit(main())
