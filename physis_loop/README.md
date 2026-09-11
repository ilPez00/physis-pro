# Physis Corrective Feedback Engine (Python)

A small, clean Python implementation of a general-purpose corrective feedback engine
that couples a generator with an environment through minimal structural mutation.

## Core Abstraction

```
generation → environment → observation → diagnosis → minimal mutation → generation
```

The LLM is only the generator/corrector. The environment is the source of reality.

## Installation

```bash
# From the physis-pro root
cd physis_loop
pip install -e .
# Or just ensure physis_loop is on your PYTHONPATH
```

## Quick Start

```python
from physis_loop import PlaceholderGenerator, PythonEnvironment, run_corrective_loop

gen = PlaceholderGenerator()
env = PythonEnvironment(timeout=5.0)

final_artifact, history = run_corrective_loop(
    "print hello world",
    gen,
    env,
    max_attempts=10,
)
```

## Architecture

### Core Types

- **`Intent`** - Describes what the user wants
- **`Artifact`** - The generated object being evaluated (generic type)
- **`Observation`** - Structured result of environment exposure
- **`Constraints`** - What must remain true for coherence
- **`Diagnosis`** - Interprets observation in relation to artifact + constraints
- **`Mutation`** - Produces minimally changed artifact
- **`Generator`** - Interface for generating/correcting artifacts
- **`Environment`** - Exposes artifact to external reality, returns Observation
- **`CoherenceEngine`** - Orchestrates the full feedback loop
- **`FeedbackHistory`** - Records attempts, detects stagnation

### The Core Loop

```python
for attempt in range(MAX_ATTEMPTS):
    observation = environment.observe(artifact)
    if observation.success and constraints_satisfied:
        return artifact
    diagnosis = coherence_engine.diagnose(artifact, observation, constraints)
    artifact = coherence_engine.mutate(artifact, diagnosis)
raise CoherenceFailure
```

Key principle: **preserve everything not implicated by the observation**.

### Components

| File | Purpose |
|------|---------|
| `artifact.py` | `Artifact` + `Intent` dataclasses |
| `observation.py` | `Observation` + `Constraints` dataclasses |
| `diagnosis.py` | `Diagnosis` - interprets observations |
| `mutation.py` | `minimal_mutation()` - preserves working structure |
| `generator.py` | `Generator` abstract class + `PlaceholderGenerator` |
| `environment.py` | Abstract `Environment` base class |
| `python_environment.py` | `PythonEnvironment` - executes Python in subprocess |
| `history.py` | `FeedbackHistory` - records attempts, stagnation detection |
| `core.py` | `CoherenceEngine` - orchestrates the loop |
| `intent.py` | `Intent` dataclass |

### Python Environment

`PythonEnvironment` executes generated Python source in a controlled subprocess:

1. Receives generated Python source
2. Executes it in a subprocess with configurable timeout
3. Captures stdout, stderr, exit status
4. Returns an `Observation`

**Not a security boundary** - experimental sandbox.

### Test Scenarios

All 6 required scenarios covered:

1. Valid artifact succeeds immediately
2. Broken artifact corrected after environmental failure
3. History records each attempt
4. Repeated identical failure detected (stagnation)
5. Environment replaceable independently of generator
6. Working portions preserved during correction

### License

Experimental research software. See LICENSE.txt in the physis-pro root.