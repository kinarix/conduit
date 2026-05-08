package io.conduit.worker;

import java.lang.annotation.ElementType;
import java.lang.annotation.Retention;
import java.lang.annotation.RetentionPolicy;
import java.lang.annotation.Target;

/**
 * Marks a class as a Conduit external-task handler bound to a topic.
 *
 * <p>The class must implement {@link Handler}. {@link Runner#discover}
 * finds every {@code @TaskHandler}-annotated instance passed to it and
 * registers it under the topic from the annotation.
 *
 * <pre>
 * &#64;TaskHandler(topic = "http.call")
 * public class HttpCallHandler implements Handler {
 *     public HandlerResult handle(ExternalTask task) {
 *         return HandlerResult.complete(Variable.string("status", "ok"));
 *     }
 * }
 * </pre>
 */
@Retention(RetentionPolicy.RUNTIME)
@Target(ElementType.TYPE)
public @interface TaskHandler {
  /** Topic this handler subscribes to (matches {@code <conduit:taskTopic>} in BPMN). */
  String topic();
}
