#!/usr/bin/env python3
"""Lattice ipykernel stdio JSON-lines bridge.

Speaks newline-delimited JSON on stdin/stdout. ZMQ stays inside this process
(via jupyter_client + ipykernel); the trusted Rust host never links ZMQ or
embeds CPython.

Protocol (see crate README):
  requests:  execute | interrupt | shutdown
  responses: ready | stream | execute_result | error | done | bridge_error

`interrupt` is handled on the stdin loop without waiting for an in-flight
`execute` to finish, so the kernel can be signalled while Rust is collecting
iopub messages.
"""

from __future__ import annotations

import json
import sys
import threading
import traceback
from typing import Any


def _emit(payload: dict[str, Any]) -> None:
    sys.stdout.write(json.dumps(payload, ensure_ascii=False) + "\n")
    sys.stdout.flush()


def _bridge_error(req_id: str | None, message: str) -> None:
    payload: dict[str, Any] = {"type": "bridge_error", "message": message}
    if req_id is not None:
        payload["id"] = req_id
    _emit(payload)


def _plain_text(data: dict[str, Any] | None) -> dict[str, str]:
    if not isinstance(data, dict):
        return {"text/plain": ""}
    plain = data.get("text/plain")
    if plain is None:
        return {"text/plain": ""}
    if isinstance(plain, list):
        plain = "".join(str(part) for part in plain)
    return {"text/plain": str(plain)}


def main() -> int:
    try:
        from jupyter_client import KernelManager
    except ImportError as err:
        _bridge_error(None, f"jupyter_client is not available: {err}")
        return 1

    km = KernelManager()
    try:
        km.start_kernel(stdout=None, stderr=None)
        kc = km.client()
        kc.start_channels()
        try:
            kc.wait_for_ready(timeout=60)
        except Exception as err:  # noqa: BLE001 — surface as protocol error
            _bridge_error(None, f"kernel failed to become ready: {err}")
            km.shutdown_kernel(now=True)
            return 1
    except Exception as err:  # noqa: BLE001
        _bridge_error(None, f"failed to start ipykernel: {err}")
        return 1

    _emit({"type": "ready"})

    # One execute at a time; interrupt must not wait on this lock.
    execute_lock = threading.Lock()
    shutdown_requested = threading.Event()
    emit_lock = threading.Lock()

    def emit(payload: dict[str, Any]) -> None:
        with emit_lock:
            _emit(payload)

    def handle_execute(req_id: str, code: str) -> None:
        try:
            msg_id = kc.execute(code, store_history=True, allow_stdin=False)
            status = "ok"
            while not shutdown_requested.is_set():
                try:
                    msg = kc.get_iopub_msg(timeout=1.0)
                except Exception:
                    # Timeout or empty — keep polling so interrupt can idle the kernel.
                    continue

                parent = msg.get("parent_header") or {}
                if parent.get("msg_id") != msg_id:
                    continue

                msg_type = msg.get("msg_type")
                content = msg.get("content") or {}

                if msg_type == "stream":
                    emit(
                        {
                            "type": "stream",
                            "id": req_id,
                            "name": content.get("name") or "stdout",
                            "text": content.get("text") or "",
                        }
                    )
                elif msg_type == "execute_result":
                    emit(
                        {
                            "type": "execute_result",
                            "id": req_id,
                            "data": _plain_text(content.get("data")),
                        }
                    )
                elif msg_type == "error":
                    status = "error"
                    tb = content.get("traceback") or []
                    if not isinstance(tb, list):
                        tb = [str(tb)]
                    emit(
                        {
                            "type": "error",
                            "id": req_id,
                            "ename": content.get("ename") or "Error",
                            "evalue": content.get("evalue") or "",
                            "traceback": [str(line) for line in tb],
                        }
                    )
                elif msg_type == "status" and content.get("execution_state") == "idle":
                    break
            else:
                status = "abort"

            emit({"type": "done", "id": req_id, "status": status})
        except Exception as err:  # noqa: BLE001
            emit({"type": "bridge_error", "id": req_id, "message": str(err)})
            emit({"type": "done", "id": req_id, "status": "error"})
        finally:
            execute_lock.release()

    try:
        for raw in sys.stdin:
            if shutdown_requested.is_set():
                break
            line = raw.strip()
            if not line:
                continue
            try:
                req = json.loads(line)
            except json.JSONDecodeError as err:
                _bridge_error(None, f"invalid JSON request: {err}")
                continue

            req_type = req.get("type")
            req_id = req.get("id")
            if not isinstance(req_id, str) or not req_id:
                _bridge_error(None, "request missing string id")
                continue

            if req_type == "execute":
                code = req.get("code")
                if not isinstance(code, str):
                    emit({"type": "bridge_error", "id": req_id, "message": "execute requires string code"})
                    emit({"type": "done", "id": req_id, "status": "error"})
                    continue
                if not execute_lock.acquire(blocking=False):
                    emit(
                        {
                            "type": "bridge_error",
                            "id": req_id,
                            "message": "execute already in progress",
                        }
                    )
                    emit({"type": "done", "id": req_id, "status": "error"})
                    continue
                threading.Thread(
                    target=handle_execute,
                    args=(req_id, code),
                    name=f"lattice-execute-{req_id}",
                    daemon=True,
                ).start()
            elif req_type == "interrupt":
                try:
                    km.interrupt_kernel()
                    emit({"type": "done", "id": req_id, "status": "ok"})
                except Exception as err:  # noqa: BLE001
                    emit({"type": "bridge_error", "id": req_id, "message": f"interrupt failed: {err}"})
                    emit({"type": "done", "id": req_id, "status": "error"})
            elif req_type == "shutdown":
                shutdown_requested.set()
                try:
                    km.interrupt_kernel()
                except Exception:  # noqa: BLE001
                    pass
                emit({"type": "done", "id": req_id, "status": "ok"})
                break
            else:
                emit({"type": "bridge_error", "id": req_id, "message": f"unknown request type: {req_type!r}"})
                emit({"type": "done", "id": req_id, "status": "error"})
    except Exception:  # noqa: BLE001
        _bridge_error(None, traceback.format_exc())
        return 1
    finally:
        shutdown_requested.set()
        # Give an in-flight execute a moment to observe shutdown.
        acquired = execute_lock.acquire(timeout=2.0)
        if acquired:
            execute_lock.release()
        try:
            kc.stop_channels()
        except Exception:  # noqa: BLE001
            pass
        try:
            km.shutdown_kernel(now=True)
        except Exception:  # noqa: BLE001
            pass

    return 0


if __name__ == "__main__":
    sys.exit(main())
