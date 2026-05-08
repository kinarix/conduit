package io.conduit.worker;

import com.fasterxml.jackson.databind.ObjectMapper;
import java.io.IOException;
import java.net.URI;
import java.net.http.HttpClient;
import java.net.http.HttpRequest;
import java.net.http.HttpResponse;
import java.time.Duration;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

/** Typed wrapper over the engine's {@code /api/v1/external-tasks/*} endpoints. */
public final class Client {

  /** Configures connectivity to the Conduit engine. */
  public static final class Config {
    private final String baseUrl;
    private String apiKey;
    private Duration requestTimeout = Duration.ofSeconds(30);

    public Config(String baseUrl) {
      while (baseUrl.endsWith("/")) baseUrl = baseUrl.substring(0, baseUrl.length() - 1);
      this.baseUrl = baseUrl;
    }

    public Config apiKey(String key) {
      this.apiKey = key;
      return this;
    }

    public Config requestTimeout(Duration t) {
      this.requestTimeout = t;
      return this;
    }
  }

  /** Raised when the engine returns a non-2xx status. */
  public static final class HttpError extends RuntimeException {
    public final int status;
    public final String body;

    public HttpError(int status, String body) {
      super("engine returned " + status + ": " + body);
      this.status = status;
      this.body = body;
    }
  }

  private final HttpClient http;
  private final ObjectMapper mapper = new ObjectMapper();
  private final String baseUrl;
  private final String apiKey;

  public Client(Config config) {
    this.baseUrl = config.baseUrl;
    this.apiKey = config.apiKey;
    this.http = HttpClient.newBuilder().connectTimeout(config.requestTimeout).build();
  }

  public List<ExternalTask> fetchAndLock(String workerId, String topic, int maxJobs, int lockDurationSecs)
      throws IOException, InterruptedException {
    Map<String, Object> body = new LinkedHashMap<>();
    body.put("worker_id", workerId);
    body.put("topic", topic);
    body.put("max_jobs", maxJobs);
    body.put("lock_duration_secs", lockDurationSecs);
    HttpResponse<String> resp = post("/api/v1/external-tasks/fetch-and-lock", body);
    return mapper.readerForListOf(ExternalTask.class).readValue(resp.body());
  }

  public void complete(String taskId, String workerId, List<Variable> variables)
      throws IOException, InterruptedException {
    Map<String, Object> body = new LinkedHashMap<>();
    body.put("worker_id", workerId);
    body.put("variables", variables);
    post("/api/v1/external-tasks/" + taskId + "/complete", body);
  }

  public void failure(String taskId, String workerId, String errorMessage)
      throws IOException, InterruptedException {
    Map<String, Object> body = new LinkedHashMap<>();
    body.put("worker_id", workerId);
    body.put("error_message", errorMessage);
    post("/api/v1/external-tasks/" + taskId + "/failure", body);
  }

  public void bpmnError(
      String taskId, String workerId, String code, String message, List<Variable> variables)
      throws IOException, InterruptedException {
    Map<String, Object> body = new LinkedHashMap<>();
    body.put("worker_id", workerId);
    body.put("error_code", code);
    body.put("error_message", message);
    body.put("variables", variables);
    post("/api/v1/external-tasks/" + taskId + "/bpmn-error", body);
  }

  public void extendLock(String taskId, String workerId, int lockDurationSecs)
      throws IOException, InterruptedException {
    Map<String, Object> body = new LinkedHashMap<>();
    body.put("worker_id", workerId);
    body.put("lock_duration_secs", lockDurationSecs);
    post("/api/v1/external-tasks/" + taskId + "/extend-lock", body);
  }

  private HttpResponse<String> post(String path, Object body) throws IOException, InterruptedException {
    HttpRequest.Builder b = HttpRequest.newBuilder()
        .uri(URI.create(baseUrl + path))
        .header("Content-Type", "application/json")
        .POST(HttpRequest.BodyPublishers.ofString(mapper.writeValueAsString(body)));
    if (apiKey != null) b.header("Authorization", "Bearer " + apiKey);
    HttpResponse<String> resp = http.send(b.build(), HttpResponse.BodyHandlers.ofString());
    if (resp.statusCode() / 100 != 2) {
      throw new HttpError(resp.statusCode(), resp.body());
    }
    return resp;
  }
}
