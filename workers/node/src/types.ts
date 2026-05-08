// Wire shapes for the external-task protocol.
// See workers/PROTOCOL.md for the contract.

export interface Variable {
  name: string;
  /** "String" | "Long" | "Double" | "Boolean" | "Json" | "Null" */
  value_type: string;
  value: unknown;
}

export const Variable = {
  string(name: string, value: string): Variable {
    return { name, value_type: "String", value };
  },
  long(name: string, value: number): Variable {
    return { name, value_type: "Long", value: Math.trunc(value) };
  },
  double(name: string, value: number): Variable {
    return { name, value_type: "Double", value };
  },
  boolean(name: string, value: boolean): Variable {
    return { name, value_type: "Boolean", value };
  },
  json(name: string, value: unknown): Variable {
    return { name, value_type: "Json", value };
  },
  null(name: string): Variable {
    return { name, value_type: "Null", value: null };
  },
};

export interface ExternalTask {
  id: string;
  topic: string | null;
  instance_id: string;
  execution_id: string;
  locked_until: string | null;
  retries: number;
  retry_count: number;
  variables: Variable[];
}

export function variable(task: ExternalTask, name: string): unknown {
  return task.variables.find((v) => v.name === name)?.value;
}

export function variableMap(task: ExternalTask): Record<string, unknown> {
  return Object.fromEntries(task.variables.map((v) => [v.name, v.value]));
}
