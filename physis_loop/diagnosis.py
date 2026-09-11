from __future__ import annotations
from dataclasses import dataclass, field
from typing import Any, Optional, Dict, List, Tuple
from .observation import Observation


@dataclass
class Diagnosis:
    """Interprets an observation in relation to the artifact and constraints.

    Identifies:
        violated constraint
        probable cause
        affected region
        confidence
        information needed for correction
    """

    violated_constraint: str
    probable_cause: str
    affected_region: Optional[str] = None
    confidence: float = 1.0
    correction_hints: List[str] = field(default_factory=list)
    metadata: Dict[str, Any] = field(default_factory=dict)

    def is_high_confidence(self) -> bool:
        return self.confidence >= 0.8

    def __str__(self) -> str:
        return (
            f"Diagnosis(constraint={self.violated_constraint!r}, "
            f"cause={self.probable_cause!r}, region={self.affected_region!r}, "
            f"confidence={self.confidence:.2f})"
        )