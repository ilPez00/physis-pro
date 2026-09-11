from __future__ import annotations
from dataclasses import dataclass, field
from typing import Any, Optional, Dict, List, get_type_hints
from .artifact import Artifact


@dataclass
class Mutation:
    """Produces a minimally changed artifact.

    The mutation explicitly prefers:
        preservation of working structure
        local correction
        minimal change
        simplicity
    """

    artifact: Artifact
    changes: Dict[str, Any] = field(default_factory=dict)
    description: str = ""

    def apply(self) -> Artifact:
        """Apply the mutation changes to produce a new artifact."""
        new_data = dict(self.artifact.data)
        self._apply_recursive(new_data, self.changes)
        new_artifact = Artifact(data=new_data, metadata=dict(self.artifact.metadata))
        return new_artifact

    def _apply_recursive(
        self, data: Dict[str, Any], changes: Dict[str, Any]
    ) -> None:
        """Recursively apply changes, preferring minimal local edits."""
        for key, value in changes.items():
            if key in data and isinstance(data[key], dict) and isinstance(value, dict):
                self._apply_recursive(data[key], value)
            else:
                data[key] = value

    def __str__(self) -> str:
        return f"Mutation(desc={self.description!r}, changes={self.changes!r})"


def minimal_mutation(
    artifact: Artifact,
    diagnosis: "Diagnosis",
    override_changes: Optional[Dict[str, Any]] = None,
) -> Mutation:
    """Produce a minimally changed artifact based on the diagnosis.

    The key design rule: preserve everything not implicated by the observation.
    If only one function causes the failure, modify that function rather
    than regenerating the entire program.
    """
    changes: Dict[str, Any] = {}

    # Determine the affected region from the diagnosis
    region = diagnosis.affected_region or "unknown"

    # Build correction hints from the diagnosis
    hints = diagnosis.correction_hints

    # Default: if there's an error, mark the region as needing attention
    if not override_changes:
        # Minimal placeholder change - in a real system, this would be
        # generated based on the diagnosis
        changes[region] = f"# FIX: {diagnosis.probable_cause}"
        changes["_diagnosis_id"] = diagnosis.violated_constraint
        changes["_diagnosis_cause"] = diagnosis.probable_cause
    else:
        changes.update(override_changes)

    return Mutation(
        artifact=artifact,
        changes=changes,
        description=f"Minimal mutation for: {diagnosis.violated_constraint}",
    )