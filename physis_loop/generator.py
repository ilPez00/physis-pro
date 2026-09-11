from __future__ import annotations
from typing import Any, Optional, List
from .artifact import Artifact
from .diagnosis import Diagnosis
from .history import FeedbackHistory


class Generator:
    """Interface for generating and correcting artifacts.

    The LLM is only the generator/corrector. The environment is the
    source of reality.
    """

    def generate(self, intent: Any, context: Optional[Dict[str, Any]] = None) -> Artifact:
        """Generate an artifact from an intent.

        Args:
            intent: What the user wants.
            context: Optional previous history/context.

        Returns:
            A new Artifact instance.
        """
        raise NotImplementedError

    def correct(
        self,
        artifact: Artifact,
        diagnosis: Diagnosis,
        history: FeedbackHistory,
    ) -> Artifact:
        """Correct an artifact based on a diagnosis.

        The essential instruction: "Change the smallest amount of structure
        necessary to remove the diagnosed violation. Preserve behavior
        that has already satisfied constraints."

        Args:
            artifact: The current artifact to correct.
            diagnosis: What went wrong and why.
            history: Feedback history of previous attempts.

        Returns:
            A corrected Artifact instance.
        """
        raise NotImplementedError


class PlaceholderGenerator(Generator):
    """A placeholder LLM API function for demonstration purposes.

    Does not depend on a particular commercial API.
    """

    def generate(self, intent: Any, context: Optional[Dict[str, Any]] = None) -> Artifact:
        """Generate a simple artifact based on the intent description."""
        description = intent.description if hasattr(intent, 'description') else str(intent)
        # Create a simple Python script based on the intent
        code_lines = [
            "# Generated code for:",
            f"# {description}",
            "",
            "def main():",
            "    pass",
        ]
        data = {
            "source_code": "\n".join(code_lines),
            "intent": description,
        }
        return Artifact(data=data)

    def correct(
        self,
        artifact: Artifact,
        diagnosis: Diagnosis,
        history: FeedbackHistory,
    ) -> Artifact:
        """Apply minimal correction based on diagnosis hints."""
        # Extract current source code
        source = artifact.get("source_code", "")

        # Apply minimal change based on diagnosis
        hints = diagnosis.correction_hints or []
        correction = f"# Corrected: {diagnosis.violated_constraint}\n"

        if hints:
            correction += f"; # hint: {hints[0]}\n"

        # Simple minimal mutation: append correction hint to source
        new_source = source.rstrip() + "\n" + correction if not source.endswith("\n") else source + correction

        # Update the artifact
        new_data = dict(artifact.data)
        new_data["source_code"] = new_source

        # Add mutation metadata
        meta = dict(artifact.metadata or {})
        attempt_num = history.current_attempt() if history else 0
        meta["last_correction"] = diagnosis.violated_constraint
        meta["last_attempt"] = attempt_num
        new_data["metadata"] = meta

        return Artifact(data=new_data, metadata=dict(artifact.metadata or {}))