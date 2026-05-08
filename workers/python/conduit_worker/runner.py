"""Polling loop and the ``@handler`` decorator."""

from __future__ import annotations

import asyncio
import logging
from dataclasses import dataclass
from typing import Awaitable, Callable, Iterable

from .client import Client
from .result import BpmnError, Complete, HandlerResult
from .types import ExternalTask

log = logging.getLogger("conduit_worker")

HandlerFn = Callable[[ExternalTask], Awaitable[HandlerResult]]


@dataclass
class _Registration:
    topic: str
    fn: HandlerFn


@dataclass
class RunnerConfig:
    worker_id: str
    max_jobs: int = 10
    lock_duration_secs: int = 30
    poll_interval_secs: float = 1.0


def handler(*, topic: str) -> Callable[[HandlerFn], HandlerFn]:
    """Mark an async function as a Conduit task handler.

    Example::

        @handler(topic="http.call")
        async def http_call(task: ExternalTask) -> HandlerResult:
            return Complete(variables=[Variable.string("status", "ok")])

    The decorated function is unchanged at runtime; the decorator only
    attaches a ``__conduit_topic__`` attribute that ``Runner.discover`` reads.
    """

    def wrap(fn: HandlerFn) -> HandlerFn:
        setattr(fn, "__conduit_topic__", topic)
        return fn

    return wrap


class Runner:
    """Fetch-handle-report loop. Register handlers, then ``await runner.run()``."""

    def __init__(self, client: Client, config: RunnerConfig) -> None:
        self._client = client
        self._config = config
        self._handlers: dict[str, HandlerFn] = {}

    def register(self, topic: str, fn: HandlerFn) -> None:
        """Bind ``fn`` to ``topic``. Overwrites any previous binding."""
        self._handlers[topic] = fn

    def discover(self, *fns: HandlerFn) -> None:
        """Register every function carrying a ``__conduit_topic__`` attribute.

        Pairs with the ``@handler(topic=...)`` decorator: collect your handler
        functions into a list and pass them to ``runner.discover(*handlers)``.
        """
        for fn in fns:
            topic = getattr(fn, "__conduit_topic__", None)
            if not topic:
                raise ValueError(f"{fn!r} is not decorated with @handler(topic=...)")
            self.register(topic, fn)

    async def run(self) -> None:
        if not self._handlers:
            raise RuntimeError("no handlers registered")
        while True:
            did_work = await self._tick()
            if not did_work:
                await asyncio.sleep(self._config.poll_interval_secs)

    async def _tick(self) -> bool:
        did_work = False
        for topic in list(self._handlers.keys()):
            try:
                tasks = await self._client.fetch_and_lock(
                    worker_id=self._config.worker_id,
                    topic=topic,
                    max_jobs=self._config.max_jobs,
                    lock_duration_secs=self._config.lock_duration_secs,
                )
            except Exception:
                log.exception("fetch-and-lock failed for topic %s", topic)
                continue
            for task in tasks:
                await self._dispatch(task)
            if tasks:
                did_work = True
        return did_work

    async def _dispatch(self, task: ExternalTask) -> None:
        topic = task.topic or ""
        fn = self._handlers.get(topic)
        if fn is None:
            log.warning("no handler for topic %s (task %s)", topic, task.id)
            return

        try:
            result = await fn(task)
        except Exception as exc:
            log.exception("handler raised — reporting failure")
            try:
                await self._client.failure(task.id, self._config.worker_id, str(exc))
            except Exception:
                log.exception("failure call itself failed for task %s", task.id)
            return

        try:
            if isinstance(result, BpmnError):
                await self._client.bpmn_error(
                    task.id,
                    self._config.worker_id,
                    result.code,
                    result.message,
                    result.variables,
                )
            elif isinstance(result, Complete):
                await self._client.complete(task.id, self._config.worker_id, result.variables)
            else:
                raise TypeError(f"handler returned non-HandlerResult: {result!r}")
        except Exception:
            log.exception("report-back call failed for task %s", task.id)
