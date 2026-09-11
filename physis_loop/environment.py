from __future__ import annotations
from typing import Any
from .observation import Observation


class Environment:
    """Abstract base class for environments that evaluate artifacts.

    Exposes the artifact to some external reality and returns an
    Observation rather than directly deciding how the artifact should
    be fixed.

    Example:
        PythonEnvironment
            executes Python code.
    """

    def observe(self, artifact: Any) -> Observation:
        """Observe the artifact in the environment.

        Args:
            artifact: The artifact to evaluate.

        Returns:
            An Observation representing the result of exposure.
        """
        raise NotImplementedError

    def __repr__(self) -> str:
        return f"{self.__class__.__name__}()"