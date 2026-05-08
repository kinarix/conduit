import json

import httpx
import pytest
import respx

from conduit_worker import (
    BpmnError,
    Client,
    ClientConfig,
    Complete,
    ExternalTask,
    Runner,
    RunnerConfig,
    Variable,
    handler,
)


@pytest.fixture
def base_url() -> str:
    return "http://engine.test"


@respx.mock
async def test_fetch_and_lock_round_trip(base_url):
    respx.post(f"{base_url}/api/v1/external-tasks/fetch-and-lock").mock(
        return_value=httpx.Response(
            200,
            json=[
                {
                    "id": "t1",
                    "topic": "http.call",
                    "instance_id": "i1",
                    "execution_id": "e1",
                    "retries": 3,
                    "retry_count": 0,
                    "variables": [
                        {"name": "order_id", "value_type": "String", "value": "ord-42"}
                    ],
                }
            ],
        )
    )
    async with Client(ClientConfig(base_url=base_url)) as c:
        tasks = await c.fetch_and_lock("py-1", "http.call")
    assert len(tasks) == 1
    assert tasks[0].id == "t1"
    assert tasks[0].variable("order_id") == "ord-42"


@respx.mock
async def test_complete_serialises_variables(base_url):
    captured = {}

    def matcher(request: httpx.Request) -> httpx.Response:
        captured["body"] = json.loads(request.content)
        return httpx.Response(204)

    respx.post(f"{base_url}/api/v1/external-tasks/task-id/complete").mock(side_effect=matcher)
    async with Client(ClientConfig(base_url=base_url)) as c:
        await c.complete("task-id", "py-1", [Variable.string("status", "ok"), Variable.long("count", 7)])
    assert captured["body"]["worker_id"] == "py-1"
    assert captured["body"]["variables"] == [
        {"name": "status", "value_type": "String", "value": "ok"},
        {"name": "count", "value_type": "Long", "value": 7},
    ]


@respx.mock
async def test_runner_dispatches_via_decorator(base_url):
    fetch_route = respx.post(f"{base_url}/api/v1/external-tasks/fetch-and-lock")
    fetch_route.side_effect = [
        httpx.Response(
            200,
            json=[
                {
                    "id": "t1",
                    "topic": "http.call",
                    "instance_id": "i1",
                    "execution_id": "e1",
                    "retries": 3,
                    "retry_count": 0,
                    "variables": [],
                }
            ],
        ),
        httpx.Response(200, json=[]),
    ]
    complete_route = respx.post(f"{base_url}/api/v1/external-tasks/t1/complete").mock(
        return_value=httpx.Response(204)
    )

    @handler(topic="http.call")
    async def http_call(task: ExternalTask):
        return Complete(variables=[Variable.string("status", "ok")])

    async with Client(ClientConfig(base_url=base_url)) as c:
        runner = Runner(c, RunnerConfig(worker_id="py-1", poll_interval_secs=0.01))
        runner.discover(http_call)
        await runner._tick()  # one tick is enough for the assertion
    assert complete_route.called


@respx.mock
async def test_runner_reports_bpmn_error(base_url):
    respx.post(f"{base_url}/api/v1/external-tasks/fetch-and-lock").mock(
        return_value=httpx.Response(
            200,
            json=[
                {
                    "id": "t1",
                    "topic": "policy.check",
                    "instance_id": "i1",
                    "execution_id": "e1",
                    "retries": 3,
                    "retry_count": 0,
                    "variables": [],
                }
            ],
        )
    )
    bpmn_route = respx.post(f"{base_url}/api/v1/external-tasks/t1/bpmn-error").mock(
        return_value=httpx.Response(204)
    )

    async def policy_check(task: ExternalTask):
        return BpmnError(code="POLICY_VIOLATION", message="not allowed")

    async with Client(ClientConfig(base_url=base_url)) as c:
        runner = Runner(c, RunnerConfig(worker_id="py-1"))
        runner.register("policy.check", policy_check)
        await runner._tick()
    assert bpmn_route.called
    body = json.loads(bpmn_route.calls.last.request.content)
    assert body["error_code"] == "POLICY_VIOLATION"
