from __future__ import annotations
from dataclasses import dataclass, field
from typing import Any, Dict, Optional, List
import json


@dataclass
class Artifact:
    """The generated object being evaluated.

    Initially this may contain Python source code, but the architecture
    is designed so that the artifact type is generic.
    """

    data: Dict[str, Any]
    metadata: Dict[str, Any] = field(default_factory=dict)

    def get(self, key: str, default: Any = None) -> Any:
        return self.data.get(key, default)

    def set(self, key: str, value: Any) -> None:
        self.data[key] = value

    def to_dict(self) -> Dict[str, Any]:
        return self.data

    @classmethod
    def from_dict(cls, d: Dict[str, Any]) -> Artifact:
        return cls(data=d, metadata={})

    def __str__(self) -> str:
        return json.dumps(self.data, indent=2)

    def __repr__(self) -> str:
        return f"Artifact({self.data!r})"


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