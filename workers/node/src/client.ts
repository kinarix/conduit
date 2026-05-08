import type { ExternalTask, Variable } from "./types.js";

export interface ClientConfig {
  baseUrl: string;
  apiKey?: string;
  /** Per-request timeout in milliseconds (default 30000). */
  requestTimeoutMs?: number;
  /** Override fetch (e.g. for tests). Defaults to globalThis.fetch. */
  fetch?: typeof fetch;
}

export class HttpError extends Error {
  constructor(public status: number, public body: string) {
    super(`engine returned ${status}: ${body}`);
    this.name = "HttpError";
  }
}

export class Client {
  private readonly baseUrl: string;
  private readonly apiKey: string | undefined;
  private readonly timeoutMs: number;
  private readonly fetchFn: typeof fetch;

  constructor(config: ClientConfig) {
    this.baseUrl = config.baseUrl.replace(/\/+$/, "");
    this.apiKey = config.apiKey;
    this.timeoutMs = config.requestTimeoutMs ?? 30_000;
    const f = config.fetch ?? globalThis.fetch;
    if (!f) throw new Error("no fetch available — pass config.fetch on Node < 18");
    this.fetchFn = f.bind(globalThis);
  }

  async fetchAndLock(
    workerId: string,
    topic: string,
    maxJobs = 10,
    lockDurationSecs = 30,
  ): Promise<ExternalTask[]> {
    const resp = await this.post("/api/v1/external-tasks/fetch-and-lock", {
      worker_id: workerId,
      topic,
      max_jobs: maxJobs,
      lock_duration_secs: lockDurationSecs,
    });
    return (await resp.json()) as ExternalTask[];
  }

  async complete(taskId: string, workerId: string, variables: Variable[] = []): Promise<void> {
    await this.post(`/api/v1/external-tasks/${taskId}/complete`, {
      worker_id: workerId,
      variables,
    });
  }

  async failure(taskId: string, workerId: string, errorMessage: string): Promise<void> {
    await this.post(`/api/v1/external-tasks/${taskId}/failure`, {
      worker_id: workerId,
      error_message: errorMessage,
    });
  }

  async bpmnError(
    taskId: string,
    workerId: string,
    errorCode: string,
    errorMessage: string,
    variables: Variable[] = [],
  ): Promise<void> {
    await this.post(`/api/v1/external-tasks/${taskId}/bpmn-error`, {
      worker_id: workerId,
      error_code: errorCode,
      error_message: errorMessage,
      variables,
    });
  }

  async extendLock(taskId: string, workerId: string, lockDurationSecs: number): Promise<void> {
    await this.post(`/api/v1/external-tasks/${taskId}/extend-lock`, {
      worker_id: workerId,
      lock_duration_secs: lockDurationSecs,
    });
  }

  private async post(path: string, body: unknown): Promise<Response> {
    const headers: Record<string, string> = { "Content-Type": "application/json" };
    if (this.apiKey) headers.Authorization = `Bearer ${this.apiKey}`;
    const ac = new AbortController();
    const t = setTimeout(() => ac.abort(), this.timeoutMs);
    try {
      const resp = await this.fetchFn(this.baseUrl + path, {
        method: "POST",
        headers,
        body: JSON.stringify(body),
        signal: ac.signal,
      });
      if (!resp.ok) {
        const text = await resp.text().catch(() => "");
        throw new HttpError(resp.status, text);
      }
      return resp;
    } finally {
      clearTimeout(t);
    }
  }
}
