package io.conduit.worker;

import static org.junit.jupiter.api.Assertions.*;

import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;
import com.sun.net.httpserver.HttpExchange;
import com.sun.net.httpserver.HttpServer;
import java.io.IOException;
import java.net.InetSocketAddress;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.atomic.AtomicInteger;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

class RunnerTest {

  private HttpServer server;
  private String baseUrl;
  private final ObjectMapper mapper = new ObjectMapper();
  private final List<JsonNode> capturedBodies = new ArrayList<>();

  @BeforeEach
  void start() throws IOException {
    server = HttpServer.create(new InetSocketAddress("127.0.0.1", 0), 0);
    baseUrl = "http://127.0.0.1:" + server.getAddress().getPort();
  }

  @AfterEach
  void stop() {
    if (server != null) server.stop(0);
  }

  private byte[] readBody(HttpExchange ex) throws IOException {
    return ex.getRequestBody().readAllBytes();
  }

  private void respond(HttpExchange ex, int status, String body) throws IOException {
    byte[] b = body.getBytes(StandardCharsets.UTF_8);
    ex.getResponseHeaders().set("Content-Type", "application/json");
    ex.sendResponseHeaders(status, b.length);
    try (var os = ex.getResponseBody()) {
      os.write(b);
    }
  }

  @Test
  void fetchAndLockRoundTrip() throws Exception {
    server.createContext("/api/v1/external-tasks/fetch-and-lock", ex -> {
      capturedBodies.add(mapper.readTree(readBody(ex)));
      respond(ex, 200,
          "[{\"id\":\"t1\",\"topic\":\"http.call\",\"instance_id\":\"i1\",\"execution_id\":\"e1\","
              + "\"retries\":3,\"retry_count\":0,"
              + "\"variables\":[{\"name\":\"order_id\",\"value_type\":\"String\",\"value\":\"ord-42\"}]}]");
    });
    server.start();

    Client c = new Client(new Client.Config(baseUrl));
    var tasks = c.fetchAndLock("java-1", "http.call", 10, 30);
    assertEquals(1, tasks.size());
    assertEquals("t1", tasks.get(0).id());
    assertEquals("ord-42", tasks.get(0).variable("order_id"));
    assertEquals("java-1", capturedBodies.get(0).get("worker_id").asText());
  }

  @Test
  void runnerDispatchesViaAnnotation() throws Exception {
    AtomicInteger completeHits = new AtomicInteger();
    server.createContext("/api/v1/external-tasks/fetch-and-lock", ex -> {
      readBody(ex);
      respond(ex, 200,
          "[{\"id\":\"t1\",\"topic\":\"http.call\",\"instance_id\":\"i1\",\"execution_id\":\"e1\","
              + "\"retries\":3,\"retry_count\":0,\"variables\":[]}]");
    });
    server.createContext("/api/v1/external-tasks/t1/complete", ex -> {
      capturedBodies.add(mapper.readTree(readBody(ex)));
      completeHits.incrementAndGet();
      respond(ex, 204, "");
    });
    server.start();

    Client c = new Client(new Client.Config(baseUrl));
    Runner r = new Runner(c, new Runner.Config("java-1"));
    r.discover(new HttpCallHandler());
    r.tick();

    assertEquals(1, completeHits.get());
    var body = capturedBodies.get(0);
    assertEquals("java-1", body.get("worker_id").asText());
    assertEquals("ok", body.get("variables").get(0).get("value").asText());
  }

  @Test
  void runnerReportsBpmnError() throws Exception {
    AtomicInteger bpmnHits = new AtomicInteger();
    server.createContext("/api/v1/external-tasks/fetch-and-lock", ex -> {
      readBody(ex);
      respond(ex, 200,
          "[{\"id\":\"t1\",\"topic\":\"policy.check\",\"instance_id\":\"i1\",\"execution_id\":\"e1\","
              + "\"retries\":3,\"retry_count\":0,\"variables\":[]}]");
    });
    server.createContext("/api/v1/external-tasks/t1/bpmn-error", ex -> {
      capturedBodies.add(mapper.readTree(readBody(ex)));
      bpmnHits.incrementAndGet();
      respond(ex, 204, "");
    });
    server.start();

    Client c = new Client(new Client.Config(baseUrl));
    Runner r = new Runner(c, new Runner.Config("java-1"));
    r.register("policy.check",
        task -> HandlerResult.bpmnError("POLICY_VIOLATION", "not allowed"));
    r.tick();

    assertEquals(1, bpmnHits.get());
    assertEquals("POLICY_VIOLATION", capturedBodies.get(0).get("error_code").asText());
  }

  @TaskHandler(topic = "http.call")
  static class HttpCallHandler implements Handler {
    @Override
    public HandlerResult handle(ExternalTask task) {
      return HandlerResult.complete(Variable.string("status", "ok"));
    }
  }
}
