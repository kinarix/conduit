import { strict as assert } from "node:assert";
import { test } from "node:test";

import { Client, defineHandler, HandlerResult, Runner, Variable } from "../src/index.js";
import type { ExternalTask } from "../src/index.js";

function fakeFetch(routes: Array<(url: string, init: RequestInit) => Response | undefined>): typeof fetch {
  return (async (input: RequestInfo | URL, init?: RequestInit): Promise<Response> => {
    const url = typeof input === "string" ? input : input.toString();
    for (const r of routes) {
      const out = r(url, init ?? {});
      if (out) return out;
    }
    return new Response("no route", { status: 500 });
  }) as unknown as typeof fetch;
}

test("fetch-and-lock returns parsed tasks", async () => {
  const fetch = fakeFetch([
    (url) =>
      url.endsWith("/fetch-and-lock")
        ? Response.json([
            {
              id: "t1",
              topic: "http.call",
              instance_id: "i1",
              execution_id: "e1",
              locked_until: null,
              retries: 3,
              retry_count: 0,
              variables: [{ name: "order_id", value_type: "String", value: "ord-42" }],
            },
          ])
        : undefined,
  ]);
  const c = new Client({ baseUrl: "http://engine", fetch });
  const tasks = await c.fetchAndLock("ts-1", "http.call");
  assert.equal(tasks.length, 1);
  assert.equal(tasks[0].id, "t1");
  assert.equal(tasks[0].variables[0].value, "ord-42");
});

test("complete sends worker_id and variables", async () => {
  let captured: any = null;
  const fetch = fakeFetch([
    (url, init) => {
      if (url.endsWith("/complete")) {
        captured = JSON.parse(init.body as string);
        return new Response(null, { status: 204 });
      }
      return undefined;
    },
  ]);
  const c = new Client({ baseUrl: "http://engine", fetch });
  await c.complete("task-id", "ts-1", [Variable.string("status", "ok"), Variable.long("count", 7)]);
  assert.equal(captured.worker_id, "ts-1");
  assert.deepEqual(captured.variables, [
    { name: "status", value_type: "String", value: "ok" },
    { name: "count", value_type: "Long", value: 7 },
  ]);
});

test("runner dispatches via defineHandler", async () => {
  let completeHits = 0;
  const fetch = fakeFetch([
    (url) =>
      url.endsWith("/fetch-and-lock")
        ? Response.json([
            {
              id: "t1",
              topic: "http.call",
              instance_id: "i1",
              execution_id: "e1",
              locked_until: null,
              retries: 3,
              retry_count: 0,
              variables: [],
            },
          ])
        : undefined,
    (url) => {
      if (url.endsWith("/complete")) {
        completeHits += 1;
        return new Response(null, { status: 204 });
      }
      return undefined;
    },
  ]);

  const httpCall = defineHandler({
    topic: "http.call",
    async handle(_task: ExternalTask) {
      return HandlerResult.complete(Variable.string("status", "ok"));
    },
  });

  const c = new Client({ baseUrl: "http://engine", fetch });
  const r = new Runner(c, { workerId: "ts-1" });
  r.register(httpCall);
  await r.tick();
  assert.equal(completeHits, 1);
});

test("runner reports bpmn-error", async () => {
  let bpmnHits = 0;
  let captured: any = null;
  const fetch = fakeFetch([
    (url) =>
      url.endsWith("/fetch-and-lock")
        ? Response.json([
            {
              id: "t1",
              topic: "policy.check",
              instance_id: "i1",
              execution_id: "e1",
              locked_until: null,
              retries: 3,
              retry_count: 0,
              variables: [],
            },
          ])
        : undefined,
    (url, init) => {
      if (url.endsWith("/bpmn-error")) {
        bpmnHits += 1;
        captured = JSON.parse(init.body as string);
        return new Response(null, { status: 204 });
      }
      return undefined;
    },
  ]);
  const c = new Client({ baseUrl: "http://engine", fetch });
  const r = new Runner(c, { workerId: "ts-1" });
  r.register("policy.check", async () => HandlerResult.bpmnError("POLICY_VIOLATION", "not allowed"));
  await r.tick();
  assert.equal(bpmnHits, 1);
  assert.equal(captured.error_code, "POLICY_VIOLATION");
});
