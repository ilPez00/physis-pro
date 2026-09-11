from __future__ import annotations
from dataclasses import dataclass, field
from typing import Any, Optional, Dict, List
import time


@dataclass
class Observation:
    """Structured representation of what happened when the artifact
    was exposed to the environment."""

    success: bool
    output: Any = None
    error: Optional[str] = None
    execution_time: float = 0.0
    exit_code: Optional[int] = None
    metadata: Dict[str, Any] = field(default_factory=dict)
    timestamp: float = field(default_factory=time.time)

    def __str__(self) -> str:
        if self.success:
            return f"Observation(success=True, output={self.output!r})"
        return (
            f"Observation(success=False, error={self.error!r}, "
            f"exit_code={self.exit_code})"
        )


@dataclass
class Constraints:
    """Describes what must remain true for the artifact to be coherent."""

    requirements: List[str] = field(default_factory=list)
    checks: Dict[str, Any] = field(default_factory=dict)

    def add(self, requirement: str) -> None:
        self.requirements.append(requirement)

    def __iter__(self):
        return iter(self.requirements)