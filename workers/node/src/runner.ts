import type { Client } from "./client.js";
import type { HandlerResult } from "./result.js";
import type { ExternalTask } from "./types.js";

export type HandlerFn = (task: ExternalTask) => Promise<HandlerResult>;

export interface HandlerDefinition {
  topic: string;
  handle: HandlerFn;
}

/**
 * Builder form: `defineHandler({ topic, handle })`. The returned object is
 * also a HandlerDefinition you can pass to `runner.register(def)`.
 */
export function defineHandler(def: HandlerDefinition): HandlerDefinition {
  return def;
}

export interface RunnerConfig {
  workerId: string;
  maxJobs?: number;
  lockDurationSecs?: number;
  pollIntervalMs?: number;
}

export class Runner {
  private readonly handlers = new Map<string, HandlerFn>();
  private readonly cfg: Required<RunnerConfig>;
  private stopped = false;

  constructor(private readonly client: Client, config: RunnerConfig) {
    this.cfg = {
      workerId: config.workerId,
      maxJobs: config.maxJobs ?? 10,
      lockDurationSecs: config.lockDurationSecs ?? 30,
      pollIntervalMs: config.pollIntervalMs ?? 1_000,
    };
  }

  /** Register a handler. Accepts either `(topic, fn)` or a HandlerDefinition. */
  register(topicOrDef: string | HandlerDefinition, fn?: HandlerFn): void {
    if (typeof topicOrDef === "string") {
      if (!fn) throw new Error("register(topic, fn) requires fn");
      this.handlers.set(topicOrDef, fn);
    } else {
      this.handlers.set(topicOrDef.topic, topicOrDef.handle);
    }
  }

  stop(): void {
    this.stopped = true;
  }

  async run(): Promise<void> {
    if (this.handlers.size === 0) throw new Error("no handlers registered");
    while (!this.stopped) {
      const didWork = await this.tick();
      if (!didWork) {
        await sleep(this.cfg.pollIntervalMs);
      }
    }
  }

  /** Internal: one fetch-handle-report cycle across every registered topic. Exposed for tests. */
  async tick(): Promise<boolean> {
    let didWork = false;
    for (const topic of this.handlers.keys()) {
      let tasks: ExternalTask[];
      try {
        tasks = await this.client.fetchAndLock(
          this.cfg.workerId,
          topic,
          this.cfg.maxJobs,
          this.cfg.lockDurationSecs,
        );
      } catch (err) {
        console.error(`fetch-and-lock failed for ${topic}:`, err);
        continue;
      }
      for (const task of tasks) {
        await this.dispatch(task);
      }
      if (tasks.length > 0) didWork = true;
    }
    return didWork;
  }

  private async dispatch(task: ExternalTask): Promise<void> {
    const fn = this.handlers.get(task.topic ?? "");
    if (!fn) {
      console.warn(`no handler for topic ${task.topic} (task ${task.id})`);
      return;
    }
    let result: HandlerResult;
    try {
      result = await fn(task);
    } catch (err) {
      try {
        await this.client.failure(task.id, this.cfg.workerId, errMessage(err));
      } catch (ferr) {
        console.error(`failure call failed for task ${task.id}:`, ferr);
      }
      return;
    }
    try {
      if (result.kind === "bpmn-error") {
        await this.client.bpmnError(
          task.id,
          this.cfg.workerId,
          result.code,
          result.message,
          result.variables,
        );
      } else {
        await this.client.complete(task.id, this.cfg.workerId, result.variables);
      }
    } catch (err) {
      console.error(`report-back call failed for task ${task.id}:`, err);
    }
  }
}

function sleep(ms: number): Promise<void> {
  return new Promise((res) => setTimeout(res, ms));
}

function errMessage(e: unknown): string {
  if (e instanceof Error) return e.message;
  return String(e);
}
