from __future__ import annotations
from typing import Any, Dict, List, Optional, Tuple
from .observation import Observation
from .diagnosis import Diagnosis


class FeedbackHistory:
    """Maintains a history of attempts, observations, diagnoses, and mutations.

    Allows the system to detect repeated failures and produce a
    STAGNATION state when essentially the same failure occurs repeatedly.
    """

    def __init__(self) -> None:
        self._history: List[Dict[str, Any]] = []

    def record(
        self,
        attempt: int,
        artifact: Any,
        observation: Observation,
        diagnosis: Diagnosis,
        mutation: Any,
    ) -> None:
        """Record a complete attempt cycle."""
        entry = {
            "attempt": attempt,
            "artifact_summary": (
                artifact.data.get("source_code", str(artifact))[:100]
                if hasattr(artifact, 'data')
                else str(artifact)[:100]
            ),
            "observation_success": observation.success,
            "observation_error": observation.error,
            "diagnosis": {
                "violated_constraint": diagnosis.violated_constraint,
                "probable_cause": diagnosis.probable_cause,
                "confidence": diagnosis.confidence,
            },
            "mutation_description": (
                mutation.description if hasattr(mutation, 'description') else str(mutation)[:80]
            ),
        }
        self._history.append(entry)

    def current_attempt(self) -> int:
        """Return the number of recorded attempts (next attempt index)."""
        return len(self._history)

    def get_last_diagnosis(self) -> Optional[Diagnosis]:
        """Get the diagnosis from the most recent attempt."""
        if not self._history:
            return None
        last = self._history[-1]
        d = last.get("diagnosis", {})
        return Diagnosis(
            violated_constraint=d.get("violated_constraint", ""),
            probable_cause=d.get("probable_cause", ""),
            confidence=d.get("confidence", 1.0),
        )

    def get_last_observation(self) -> Optional[Observation]:
        """Get the observation from the most recent attempt."""
        if not self._history:
            return None
        last = self._history[-1]
        obs_data = last.get("observation", {})
        return Observation(
            success=obs_data.get("success", False),
            error=obs_data.get("error"),
            output=obs_data.get("output"),
        )

    def detect_stagnation(
        self,
        window: int = 3,
        same_constraint: Optional[str] = None,
    ) -> bool:
        """Detect if the same failure has occurred repeatedly.

        If essentially the same failure occurs repeatedly, return True
        to indicate stagnation.
        """
        if len(self._history) < window:
            return False

        # Check the last `window` attempts for repeated constraints
        constraints = []
        for entry in self._history[-window:]:
            c = entry.get("diagnosis", {}).get("violated_constraint", "")
            constraints.append(c)

        # If all recent failures have the same violated constraint, we're stagnated
        if same_constraint:
            return all(c == same_constraint for c in constraints)
        else:
            return len(set(constraints)) == 1

    def __str__(self) -> str:
        return f"FeedbackHistory({len(self._history)} attempts recorded)"

    def to_dict(self) -> Dict[str, Any]:
        return {"history": self._history}

    @classmethod
    def from_dict(cls, d: Dict[str, Any]) -> FeedbackHistory:
        h = cls()
        h._history = d.get("history", [])
        return h