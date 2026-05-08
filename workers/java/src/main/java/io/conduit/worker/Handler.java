package io.conduit.worker;

/**
 * Executes one external task. Throwing a {@code RuntimeException}
 * triggers {@code POST /failure} (transient retry); returning a
 * {@link HandlerResult} triggers either {@code /complete} or
 * {@code /bpmn-error} based on its kind.
 */
@FunctionalInterface
public interface Handler {
  HandlerResult handle(ExternalTask task) throws Exception;
}
