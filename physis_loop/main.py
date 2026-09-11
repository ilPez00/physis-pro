#!/usr/bin/env python3
"""Physis Corrective Feedback Engine - Runnable demonstration."""

import sys
import os

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from physis_loop.artifact import Artifact, Intent
from physis_loop.observation import Observation, Constraints
from physis_loop.diagnosis import Diagnosis
from physis_loop.mutation import minimal_mutation, Mutation
from physis_loop.generator import PlaceholderGenerator
from physis_loop.python_environment import PythonEnvironment
from physis_loop.core import run_corrective_loop


def demo_basic_loop():
    print("=" * 60)
    print("Physis Corrective Feedback Engine - Basic Demonstration")
    print("=" * 60)

    gen = PlaceholderGenerator()
    env = PythonEnvironment(timeout=5.0)

    print("\n--- Setup ---")
    print(f"Generator: {type(gen).__name__}")
    print(f"Environment: {type(env).__name__} (timeout={env.timeout}s)")
    print(f"Max attempts: 10")

    print("\n--- Running loop: 'print hello world' ---")
    final_artifact, history = run_corrective_loop(
        "print hello world",
        gen,
        env,
        max_attempts=3,
    )

    print("\n--- Result ---")
    print(f"Final artifact attempts: {history.current_attempt()}")
    source = final_artifact.data.get("source_code", "") if final_artifact else "None"
    print(f"Generated source (first 80 chars): {source[:80]}...")

    if history.current_attempt() >= 1:
        print("\n--- History Summary ---")
        print(f"  Total attempts: {history.current_attempt()}")
        last_diag = history.get_last_diagnosis()
        if last_diag:
            print(f"  Last diagnosis: {last_diag.violated_constraint} (cause: {last_diag.probable_cause})")

    print("\n" + "=" * 60)


def demo_minimal_mutation_preservation():
    print("\n" + "=" * 60)
    print("Demonstration: Working Portions Preserved During Correction")
    print("=" * 60)

    gen = PlaceholderGenerator()

    artifact = Artifact(data={"source_code": """def working_function():
    return "this works"

def broken_function():
    # This function has an issue - uses undefined variable
    result = undefined_variable + 1
    return result

def another_working_function():
    return 42"""})

    env = PythonEnvironment(timeout=2.0)

    print("\n--- Original artifact ---")
    print(artifact.data["source_code"])

    observation = env.observe(artifact)
    print(f"\n--- Observation ---")
    print(f"  Success: {observation.success}")
    if observation.error:
        print(f"  Error: {observation.error[:60]}")

    diagnosis = Diagnosis(
        violated_constraint="execution_error",
        probable_cause="undefined variable",
        affected_region="broken_function",
        confidence=0.95,
        correction_hints=["fix undefined variable in broken_function"],
    )

    print(f"\n--- Diagnosis ---")
    print(f"  Violated constraint: {diagnosis.violated_constraint}")
    print(f"  Affected region: {diagnosis.affected_region}")
    print(f"  Correction hints: {diagnosis.correction_hints}")

    mutation = minimal_mutation(artifact, diagnosis)
    new_artifact = mutation.apply()

    new_source = new_artifact.data.get("source_code", "")

    print(f"\n--- Corrected artifact ---")
    print(new_source)

    print(f"\n--- Verification ---")
    checks = {
        "working_function preserved": "def working_function()" in new_source,
        "another_working_function preserved": "def another_working_function()" in new_source,
        "affected region referenced": "broken_function" in new_source,
    }

    all_pass = True
    for check_name, passed in checks.items():
        status = "PASS" if passed else "FAIL"
        print(f"  {status}: {check_name}")
        if not passed:
            all_pass = False

    if all_pass:
        print("  All checks passed - working code was preserved!")
    else:
        print("  Some checks failed.")

    print("\n" + "=" * 60)


def demo_stagnation_detection():
    print("\n" + "=" * 60)
    print("Demonstration: Stagnation Detection")
    print("=" * 60)

    from physis_loop.history import FeedbackHistory

    h = FeedbackHistory()

    for attempt in range(3):
        artifact = Artifact(data={"source_code": "print('hello')"})
        observation = Observation(success=False, error="undefined name")
        diagnosis = Diagnosis(
            violated_constraint="execution_error",
            probable_cause="undefined name",
        )
        mutation = Mutation(artifact=artifact, description=f"mutation {attempt}")

        h.record(
            attempt=attempt + 1,
            artifact=artifact,
            observation=observation,
            diagnosis=diagnosis,
            mutation=mutation,
        )

    print("\n--- Attempts recorded ---")
    print(f"Total attempts: {h.current_attempt()}")

    stagnant = h.detect_stagnation(same_constraint="execution_error")
    print(f"Stagnation detected (same_constraint='execution_error'): {stagnant}")

    if stagnant:
        print("System would stop - same failure repeated 3 times")
    else:
        print("System would continue trying")

    print("\n" + "=" * 60)


def main():
    demo_basic_loop()
    demo_minimal_mutation_preservation()
    demo_stagnation_detection()


if __name__ == "__main__":
    main()
