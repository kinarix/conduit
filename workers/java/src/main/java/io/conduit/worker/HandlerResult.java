package io.conduit.worker;

import java.util.Arrays;
import java.util.List;
import java.util.Objects;

/** Sealed return type — handlers either complete or throw a BPMN error. */
public sealed interface HandlerResult permits HandlerResult.Complete, HandlerResult.BpmnError {

  /** Report {@code POST /complete} with optional output variables. */
  record Complete(List<Variable> variables) implements HandlerResult {
    public Complete {
      Objects.requireNonNull(variables, "variables");
      variables = List.copyOf(variables);
    }
  }

  /**
   * Report {@code POST /bpmn-error}: branches the BPMN through a matching
   * boundaryErrorEvent. Code is required; message is informational.
   */
  record BpmnError(String code, String message, List<Variable> variables) implements HandlerResult {
    public BpmnError {
      Objects.requireNonNull(code, "code");
      message = message == null ? "" : message;
      variables = variables == null ? List.of() : List.copyOf(variables);
    }
  }

  static HandlerResult complete(Variable... variables) {
    return new Complete(Arrays.asList(variables));
  }

  static HandlerResult bpmnError(String code, String message, Variable... variables) {
    return new BpmnError(code, message, Arrays.asList(variables));
  }
}
