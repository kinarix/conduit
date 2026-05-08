"""Handler return values."""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Iterable

from .types import Variable


@dataclass
class Complete:
    """Report ``POST /complete`` with optional output variables."""

    variables: list[Variable] = field(default_factory=list)


@dataclass
class BpmnError:
    """Report ``POST /bpmn-error``: branches the BPMN through a matching boundary event."""

    code: str
    message: str = ""
    variables: list[Variable] = field(default_factory=list)


HandlerResult = Complete | BpmnError


def complete(*variables: Variable) -> Complete:
    """Convenience constructor: ``Complete(list(variables))``."""
    return Complete(variables=list(variables))


def bpmn_error(code: str, message: str = "", variables: Iterable[Variable] = ()) -> BpmnError:
    """Convenience constructor for a BPMN error."""
    return BpmnError(code=code, message=message, variables=list(variables))
