from __future__ import annotations
from dataclasses import dataclass, field
from typing import List


@dataclass
class Intent:
    """Describes what the user wants."""

    description: str
    requirements: List[str] = field(default_factory=list)
    constraints: List[str] = field(default_factory=list)

    def __str__(self) -> str:
        return self.description

    def __repr__(self) -> str:
        return f"Intent({self.description!r})"