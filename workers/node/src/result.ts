import type { Variable } from "./types.js";

export type HandlerResult =
  | { kind: "complete"; variables: Variable[] }
  | { kind: "bpmn-error"; code: string; message: string; variables: Variable[] };

export const HandlerResult = {
  complete(...variables: Variable[]): HandlerResult {
    return { kind: "complete", variables };
  },
  bpmnError(code: string, message = "", ...variables: Variable[]): HandlerResult {
    return { kind: "bpmn-error", code, message, variables };
  },
};
