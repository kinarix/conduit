"""Async HTTP client for the engine's ``/api/v1/external-tasks/*`` endpoints."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Iterable

import httpx

from .types import ExternalTask, Variable


@dataclass
class ClientConfig:
    base_url: str
    api_key: str | None = None
    request_timeout_secs: float = 30.0


class HttpError(RuntimeError):
    """Raised when the engine returns a non-2xx status."""

    def __init__(self, status: int, body: str) -> None:
        super().__init__(f"engine returned {status}: {body}")
        self.status = status
        self.body = body


class Client:
    """Typed wrapper over the engine's external-task endpoints."""

    def __init__(self, config: ClientConfig) -> None:
        headers = {"Content-Type": "application/json"}
        if config.api_key:
            headers["Authorization"] = f"Bearer {config.api_key}"
        self._http = httpx.AsyncClient(
            base_url=config.base_url.rstrip("/"),
            headers=headers,
            timeout=config.request_timeout_secs,
        )

    async def aclose(self) -> None:
        await self._http.aclose()

    async def __aenter__(self) -> "Client":
        return self

    async def __aexit__(self, *exc: object) -> None:
        await self.aclose()

    async def fetch_and_lock(
        self,
        worker_id: str,
        topic: str,
        max_jobs: int = 10,
        lock_duration_secs: int = 30,
    ) -> list[ExternalTask]:
        body = {
            "worker_id": worker_id,
            "topic": topic,
            "max_jobs": max_jobs,
            "lock_duration_secs": lock_duration_secs,
        }
        resp = await self._post("/api/v1/external-tasks/fetch-and-lock", body)
        return [ExternalTask.from_wire(t) for t in resp.json()]

    async def complete(
        self,
        task_id: str,
        worker_id: str,
        variables: Iterable[Variable] = (),
    ) -> None:
        await self._post(
            f"/api/v1/external-tasks/{task_id}/complete",
            {"worker_id": worker_id, "variables": [v.to_dict() for v in variables]},
        )

    async def failure(self, task_id: str, worker_id: str, error_message: str) -> None:
        await self._post(
            f"/api/v1/external-tasks/{task_id}/failure",
            {"worker_id": worker_id, "error_message": error_message},
        )

    async def bpmn_error(
        self,
        task_id: str,
        worker_id: str,
        error_code: str,
        error_message: str,
        variables: Iterable[Variable] = (),
    ) -> None:
        await self._post(
            f"/api/v1/external-tasks/{task_id}/bpmn-error",
            {
                "worker_id": worker_id,
                "error_code": error_code,
                "error_message": error_message,
                "variables": [v.to_dict() for v in variables],
            },
        )

    async def extend_lock(self, task_id: str, worker_id: str, lock_duration_secs: int) -> None:
        await self._post(
            f"/api/v1/external-tasks/{task_id}/extend-lock",
            {"worker_id": worker_id, "lock_duration_secs": lock_duration_secs},
        )

    async def _post(self, path: str, body: dict[str, object]) -> httpx.Response:
        resp = await self._http.post(path, json=body)
        if resp.status_code // 100 != 2:
            raise HttpError(resp.status_code, resp.text)
        return resp
