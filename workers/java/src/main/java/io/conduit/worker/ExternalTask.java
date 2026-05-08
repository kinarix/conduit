package io.conduit.worker;

import com.fasterxml.jackson.annotation.JsonCreator;
import com.fasterxml.jackson.annotation.JsonProperty;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

/** One external task delivered by {@code /external-tasks/fetch-and-lock}. */
public record ExternalTask(
    String id,
    String topic,
    @JsonProperty("instance_id") String instanceId,
    @JsonProperty("execution_id") String executionId,
    @JsonProperty("locked_until") String lockedUntil,
    int retries,
    @JsonProperty("retry_count") int retryCount,
    List<Variable> variables) {

  @JsonCreator
  public ExternalTask(
      @JsonProperty("id") String id,
      @JsonProperty("topic") String topic,
      @JsonProperty("instance_id") String instanceId,
      @JsonProperty("execution_id") String executionId,
      @JsonProperty("locked_until") String lockedUntil,
      @JsonProperty("retries") int retries,
      @JsonProperty("retry_count") int retryCount,
      @JsonProperty("variables") List<Variable> variables) {
    this.id = id;
    this.topic = topic;
    this.instanceId = instanceId;
    this.executionId = executionId;
    this.lockedUntil = lockedUntil;
    this.retries = retries;
    this.retryCount = retryCount;
    this.variables = variables == null ? List.of() : List.copyOf(variables);
  }

  /** Look up a single variable's value by name, or {@code null} if absent. */
  public Object variable(String name) {
    for (Variable v : variables) {
      if (v.name().equals(name)) return v.value();
    }
    return null;
  }

  /** Variables collapsed into a name → value map. */
  public Map<String, Object> variableMap() {
    Map<String, Object> m = new LinkedHashMap<>();
    for (Variable v : variables) m.put(v.name(), v.value());
    return Collections.unmodifiableMap(m);
  }
}
