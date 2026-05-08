package io.conduit.worker;

import java.time.Duration;
import java.util.List;
import java.util.concurrent.ConcurrentHashMap;
import java.util.concurrent.atomic.AtomicBoolean;
import java.util.logging.Level;
import java.util.logging.Logger;

/** Fetch-handle-report loop. Register handlers, then call {@link #run}. */
public final class Runner {

  /** Tunes the polling loop. */
  public static final class Config {
    public final String workerId;
    public int maxJobs = 10;
    public int lockDurationSecs = 30;
    public Duration pollInterval = Duration.ofSeconds(1);

    public Config(String workerId) {
      this.workerId = workerId;
    }
  }

  private static final Logger LOG = Logger.getLogger(Runner.class.getName());

  private final Client client;
  private final Config cfg;
  private final ConcurrentHashMap<String, Handler> handlers = new ConcurrentHashMap<>();
  private final AtomicBoolean stopped = new AtomicBoolean(false);

  public Runner(Client client, Config cfg) {
    this.client = client;
    this.cfg = cfg;
  }

  /** Direct registration: bind {@code handler} to {@code topic}. */
  public Runner register(String topic, Handler handler) {
    handlers.put(topic, handler);
    return this;
  }

  /**
   * Annotation-driven registration: scan each instance for {@link TaskHandler}
   * and register it under the annotation's topic. Each instance must implement
   * {@link Handler}.
   */
  public Runner discover(Object... taskHandlers) {
    for (Object o : taskHandlers) {
      TaskHandler ann = o.getClass().getAnnotation(TaskHandler.class);
      if (ann == null) {
        throw new IllegalArgumentException(
            o.getClass().getName() + " is not annotated with @TaskHandler");
      }
      if (!(o instanceof Handler h)) {
        throw new IllegalArgumentException(
            o.getClass().getName() + " is annotated with @TaskHandler but does not implement Handler");
      }
      register(ann.topic(), h);
    }
    return this;
  }

  public void stop() {
    stopped.set(true);
  }

  public void run() throws InterruptedException {
    if (handlers.isEmpty()) throw new IllegalStateException("no handlers registered");
    while (!stopped.get()) {
      boolean didWork = tick();
      if (!didWork) {
        Thread.sleep(cfg.pollInterval.toMillis());
      }
    }
  }

  /** One fetch-handle-report cycle across every registered topic. Public for tests. */
  public boolean tick() throws InterruptedException {
    boolean didWork = false;
    for (String topic : handlers.keySet()) {
      List<ExternalTask> tasks;
      try {
        tasks = client.fetchAndLock(cfg.workerId, topic, cfg.maxJobs, cfg.lockDurationSecs);
      } catch (Exception e) {
        LOG.log(Level.WARNING, "fetch-and-lock failed for topic " + topic, e);
        continue;
      }
      for (ExternalTask t : tasks) {
        dispatch(t);
      }
      if (!tasks.isEmpty()) didWork = true;
    }
    return didWork;
  }

  private void dispatch(ExternalTask task) {
    Handler fn = handlers.get(task.topic());
    if (fn == null) {
      LOG.warning("no handler for topic " + task.topic() + " (task " + task.id() + ")");
      return;
    }
    HandlerResult result;
    try {
      result = fn.handle(task);
    } catch (Exception ex) {
      try {
        client.failure(task.id(), cfg.workerId, ex.getMessage() == null ? ex.toString() : ex.getMessage());
      } catch (Exception ferr) {
        LOG.log(Level.SEVERE, "failure call failed for task " + task.id(), ferr);
      }
      return;
    }
    try {
      switch (result) {
        case HandlerResult.BpmnError be ->
            client.bpmnError(task.id(), cfg.workerId, be.code(), be.message(), be.variables());
        case HandlerResult.Complete c ->
            client.complete(task.id(), cfg.workerId, c.variables());
      }
    } catch (Exception ex) {
      LOG.log(Level.SEVERE, "report-back call failed for task " + task.id(), ex);
    }
  }
}
