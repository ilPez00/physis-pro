from __future__ import annotations
from typing import Any, Dict, List, Optional, Tuple
import time

from .artifact import Artifact
from .intent import Intent
from .environment import Environment
from .observation import Observation, Constraints
from .diagnosis import Diagnosis
from .mutation import minimal_mutation, Mutation
from .generator import Generator, PlaceholderGenerator
from .history import FeedbackHistory


class CoherenceEngine:
    """Coordinates diagnosis and mutation to move an artifact toward coherence.

    The core loop:
        generation → environment → observation → diagnosis → minimal mutation → generation
    """

    def __init__(
        self,
        generator: Generator,
        environment: Environment,
        constraints: Optional[Constraints] = None,
        max_attempts: int = 10,
    ) -> None:
        self.generator = generator
        self.environment = environment
        self.constraints = constraints or Constraints()
        self.max_attempts = max_attempts
        self.history = FeedbackHistory()
        self._final_artifact: Optional[Artifact] = None

    def run(self, intent: Intent) -> Tuple[Optional[Artifact], FeedbackHistory]:
        """Run the full corrective feedback loop.

        Args:
            intent: What the user wants.

        Returns:
            A tuple of (final_artifact, history).
        """
        self.history = FeedbackHistory()
        artifact = self.generator.generate(intent)
        self.history.record(
            attempt=0,
            artifact=artifact,
            observation=Observation(success=False),  # placeholder
            diagnosis=Diagnosis(
                violated_constraint="initial",
                probable_cause="initial generation",
            ),
            mutation=Mutation(artifact=artifact, description="initial generation"),
        )

        for attempt in range(1, self.max_attempts + 1):
            # Observe the artifact in the environment
            observation = self.environment.observe(artifact)

            # Check if constraints are satisfied
            if observation.success and self._check_constraints(observation):
                self._final_artifact = artifact
                return artifact, self.history

            # Diagnose the failure
            diagnosis = self._diagnose(
                artifact,
                observation,
            )

            # Check for stagnation
            if self.history.detect_stagnation(same_constraint=diagnosis.violated_constraint):
                print(
                    f"[STAGNATION] Detected after {attempt} attempts "
                    f"with same constraint: {diagnosis.violated_constraint}"
                )
                break

            # Record the attempt
            self.history.record(
                attempt=attempt,
                artifact=artifact,
                observation=observation,
                diagnosis=diagnosis,
                mutation=Mutation(
                    artifact=artifact,
                    description=f"mutation before correction {attempt}",
                ),
            )

            # Mutate the artifact based on diagnosis
            mutation = minimal_mutation(artifact, diagnosis)
            artifact = mutation.apply()

            # Record the mutation
            self.history.record(
                attempt=attempt,
                artifact=artifact,
                observation=Observation(success=False),
                diagnosis=diagnosis,
                mutation=mutation,
            )

        # Max attempts reached or stagnation
        self._final_artifact = artifact
        return artifact, self.history

    def _diagnose(
        self,
        artifact: Artifact,
        observation: Observation,
    ) -> Diagnosis:
        """Produce a diagnosis from an observation.

        In a full system this would use more sophisticated logic,
        but the minimal implementation extracts basic information.
        """
        # Determine violated constraint based on observation
        if observation.error:
            violated = "execution_error"
        elif not observation.success:
            violated = "unsatisfied_output"
        else:
            violated = "constraint_not_met"

        # Build probable cause
        cause = observation.error or "unknown failure"

        # Try to identify affected region from the artifact source
        affected = None
        source = artifact.get("source_code", "")
        if source:
            # Simple heuristic: look for the first function/def
            for line in source.split("\n"):
                if line.strip().startswith("def "):
                    affected = line.strip()[:60]
                    break

        # Build correction hints
        hints = []
        if observation.error:
            hints.append(f"fix: {observation.error[:80]}")

        return Diagnosis(
            violated_constraint=violated,
            probable_cause=cause,
            affected_region=affected,
            confidence=0.9,
            correction_hints=hints,
        )

    def _check_constraints(self, observation: Observation) -> bool:
        """Check if the observation satisfies the stored constraints."""
        for req in self.constraints.requirements:
            if req not in str(observation):
                return False
        return observation.success


def run_corrective_loop(
    intent_description: str,
    generator: Generator,
    environment: Environment,
    max_attempts: int = 10,
) -> Tuple[Optional[Artifact], FeedbackHistory]:
    """Run a corrective feedback loop with a simple intent description.

    Convenience function for quick demos and tests.
    """
    intent = Intent(description=intent_description)
    engine = CoherenceEngine(
        generator=generator,
        environment=environment,
        max_attempts=max_attempts,
    )
    return engine.run(intent)