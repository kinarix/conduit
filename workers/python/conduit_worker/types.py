"""Wire shapes for the external-task protocol."""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Any


@dataclass
class Variable:
    """One process variable on the wire (matches PROTOCOL.md)."""

    name: str
    value_type: str
    value: Any

    @staticmethod
    def string(name: str, value: str) -> "Variable":
        return Variable(name=name, value_type="String", value=value)

    @staticmethod
    def long(name: str, value: int) -> "Variable":
        return Variable(name=name, value_type="Long", value=int(value))

    @staticmethod
    def double(name: str, value: float) -> "Variable":
        return Variable(name=name, value_type="Double", value=float(value))

    @staticmethod
    def boolean(name: str, value: bool) -> "Variable":
        return Variable(name=name, value_type="Boolean", value=bool(value))

    @staticmethod
    def json(name: str, value: Any) -> "Variable":
        return Variable(name=name, value_type="Json", value=value)

    @staticmethod
    def null(name: str) -> "Variable":
        return Variable(name=name, value_type="Null", value=None)

    def to_dict(self) -> dict[str, Any]:
        return {"name": self.name, "value_type": self.value_type, "value": self.value}


@dataclass
class ExternalTask:
    """One task delivered by ``/external-tasks/fetch-and-lock``."""

    id: str
    instance_id: str
    execution_id: str
    retries: int
    retry_count: int
    topic: str | None = None
    locked_until: str | None = None
    variables: list[Variable] = field(default_factory=list)

    @classmethod
    def from_wire(cls, raw: dict[str, Any]) -> "ExternalTask":
        return cls(
            id=raw["id"],
            instance_id=raw["instance_id"],
            execution_id=raw["execution_id"],
            retries=int(raw.get("retries", 0)),
            retry_count=int(raw.get("retry_count", 0)),
            topic=raw.get("topic"),
            locked_until=raw.get("locked_until"),
            variables=[
                Variable(name=v["name"], value_type=v["value_type"], value=v.get("value"))
                for v in raw.get("variables", []) or []
            ],
        )

    def variable(self, name: str) -> Any:
        """Return the value of the named variable, or None if absent."""
        for v in self.variables:
            if v.name == name:
                return v.value
        return None

    def variable_map(self) -> dict[str, Any]:
        return {v.name: v.value for v in self.variables}
