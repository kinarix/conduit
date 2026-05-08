package io.conduit.worker;

import com.fasterxml.jackson.annotation.JsonCreator;
import com.fasterxml.jackson.annotation.JsonProperty;

/**
 * One process variable on the wire. {@code valueType} is one of
 * {@code "String"}, {@code "Long"}, {@code "Double"}, {@code "Boolean"},
 * {@code "Json"}, {@code "Null"}.
 */
public record Variable(
    String name,
    @JsonProperty("value_type") String valueType,
    Object value) {

  @JsonCreator
  public Variable(
      @JsonProperty("name") String name,
      @JsonProperty("value_type") String valueType,
      @JsonProperty("value") Object value) {
    this.name = name;
    this.valueType = valueType;
    this.value = value;
  }

  public static Variable string(String name, String value) {
    return new Variable(name, "String", value);
  }

  public static Variable longVar(String name, long value) {
    return new Variable(name, "Long", value);
  }

  public static Variable doubleVar(String name, double value) {
    return new Variable(name, "Double", value);
  }

  public static Variable bool(String name, boolean value) {
    return new Variable(name, "Boolean", value);
  }

  public static Variable json(String name, Object value) {
    return new Variable(name, "Json", value);
  }

  public static Variable nullVar(String name) {
    return new Variable(name, "Null", null);
  }
}
