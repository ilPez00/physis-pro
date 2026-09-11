from __future__ import annotations
import subprocess
import tempfile
import os
import signal
from typing import Any, Optional
from .environment import Environment
from .observation import Observation


class PythonEnvironment(Environment):
    """Concrete environment that executes generated Python source code.

    It:
        1. receives generated Python source
        2. executes it in a controlled subprocess
        3. captures stdout
        4. captures stderr
        5. captures exit status
        6. enforces a timeout
        7. returns an Observation

    Clearly document that this is an experimental sandbox and is not
    a security boundary.
    """

    def __init__(self, timeout: float = 5.0, work_dir: Optional[str] = None) -> None:
        self.timeout = timeout
        self.work_dir = work_dir or tempfile.mkdtemp(prefix="physis_env_")

    def observe(self, artifact: Any) -> Observation:
        """Execute the artifact's source code and return an Observation.

        Args:
            artifact: Must contain a "source_code" key with Python code.

        Returns:
            An Observation capturing the execution result.
        """
        source_code = artifact.get("source_code", "") if hasattr(artifact, 'get') else str(artifact)

        # Write the source to a temporary file
        source_file = os.path.join(self.work_dir, "artifact.py")
        with open(source_file, "w") as f:
            f.write(source_code)

        # Execute in a subprocess with timeout
        try:
            result = subprocess.run(
                [os.sys.executable, source_file],
                capture_output=True,
                text=True,
                timeout=self.timeout,
                cwd=self.work_dir,
            )

            return Observation(
                success=(result.returncode == 0),
                output=result.stdout[-500:] if result.stdout else None,
                error=result.stderr[-500:] if result.stderr else None,
                execution_time=result.time if hasattr(result, 'time') else 0.0,
                exit_code=result.returncode,
            )
        except subprocess.TimeoutExpired:
            return Observation(
                success=False,
                error=f"Execution timed out after {self.timeout}s",
                execution_time=self.timeout,
                exit_code=-1,
            )
        except Exception as e:
            return Observation(
                success=False,
                error=f"Execution error: {str(e)}",
                execution_time=0.0,
                exit_code=-1,
            )

    def __repr__(self) -> str:
        return f"PythonEnvironment(timeout={self.timeout}, work_dir={self.work_dir})"