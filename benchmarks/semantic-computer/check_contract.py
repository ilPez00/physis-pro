#!/usr/bin/env python3
"""Check scaffold integrity and critical plan/result invariants, without execution.

This is not a complete JSON Schema validator or an implementation benchmark.
The worker's implementation must enforce equivalent checks at its boundaries.
"""
import copy
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]


def check_plan(plan, capabilities, object_revisions):
    seen = set()
    required_validators = set()
    for step in plan["steps"]:
        if step["step_id"] in seen:
            raise ValueError("duplicate step ID")
        if not set(step["requires_steps"]).issubset(seen):
            raise ValueError("dependency is missing, cyclic or out of order")
        key = (step["capability_id"], step["capability_version"])
        if key not in capabilities:
            raise ValueError("unregistered capability version")
        if not step["authorization_ref"]:
            raise ValueError("missing authorization reference")
        for object_id in step["bound_object_ids"]:
            if object_revisions.get(object_id) != step["state_revision"]:
                raise ValueError("unknown object or stale state")
        required_validators.update(capabilities[key])
        seen.add(step["step_id"])
    if not seen:
        raise ValueError("empty executable plan")
    return required_validators


def check_result(result, required_validators, event_ids):
    if result["status"] != "success":
        return
    reports = result["validations"]
    if not required_validators or {r["validator_id"] for r in reports} != required_validators:
        raise ValueError("success omits required validators")
    if len(reports) != len(required_validators):
        raise ValueError("duplicate validator reports")
    for report in reports:
        if report["status"] != "verified":
            raise ValueError("unverified outcome cannot be successful")
        if not report["evidence_event_ids"] or not set(report["evidence_event_ids"]).issubset(event_ids):
            raise ValueError("validation evidence does not resolve")


def main():
    schema = json.loads((ROOT / "schemas/semantic-computer/v1.schema.json").read_text())
    manifest = json.loads((ROOT / "benchmarks/semantic-computer/manifest.json").read_text())
    refs = []

    def visit(value):
        if isinstance(value, dict):
            if "$ref" in value:
                refs.append(value["$ref"])
                assert value["$ref"].startswith("#/$defs/")
                assert value["$ref"].split("/")[-1] in schema["$defs"]
            for child in value.values():
                visit(child)
        elif isinstance(value, list):
            for child in value:
                visit(child)

    visit(schema)
    assert refs and len(manifest["tasks"]) == len({t["task_id"] for t in manifest["tasks"]})
    assert manifest["status"] == "scaffold_not_executed"
    assert all(t["status"] == "not_implemented" for t in manifest["tasks"])
    step = {"step_id": "build", "capability_id": "repo.build", "capability_version": "1",
            "requires_steps": [], "authorization_ref": "fixture-consent", "bound_object_ids": ["repo:fixture"],
            "state_revision": "r1"}
    plan = {"steps": [step]}
    registry = {("repo.build", "1"): {"build_exit", "source_revision"}}
    required = check_plan(plan, registry, {"repo:fixture": "r1"})
    result = {"status": "success", "validations": [
        {"validator_id": v, "status": "verified", "evidence_event_ids": ["event:validation"]}
        for v in sorted(required)]}
    check_result(result, required, {"event:validation"})
    rejected = 0

    def must_reject(callback):
        nonlocal rejected
        try:
            callback()
        except ValueError:
            rejected += 1
        else:
            raise AssertionError("known-bad contract was accepted")

    must_reject(lambda: check_plan(plan, registry, {"repo:fixture": "r2"}))
    must_reject(lambda: check_plan(plan, {}, {"repo:fixture": "r1"}))
    cycle = copy.deepcopy(plan); cycle["steps"][0]["requires_steps"] = ["build"]
    must_reject(lambda: check_plan(cycle, registry, {"repo:fixture": "r1"}))
    duplicate = copy.deepcopy(plan); duplicate["steps"].append(copy.deepcopy(step))
    must_reject(lambda: check_plan(duplicate, registry, {"repo:fixture": "r1"}))
    unverified = copy.deepcopy(result); unverified["validations"][0]["status"] = "unknown"
    must_reject(lambda: check_result(unverified, required, {"event:validation"}))
    incomplete = copy.deepcopy(result); incomplete["validations"].pop()
    must_reject(lambda: check_result(incomplete, required, {"event:validation"}))
    must_reject(lambda: check_result(result, required, set()))
    print(f"PASS: {len(refs)} schema references, {len(manifest['tasks'])} task definitions, "
          f"2 positive contract examples, {rejected}/7 known-bad controls rejected.")
    print("Runtime adapters, schema-wide validation and empirical performance: NOT MEASURED.")


if __name__ == "__main__":
    main()
