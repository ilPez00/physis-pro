# Mission: Physis Corrective Feedback Engine

## M1: Core Type Definitions
### T1.1: Artifact Data Structure
- [ ] S1.1.1: Define Artifact dataclass with data and metadata fields | size:S
- [ ] S1.1.2: Implement Artifact.get() and Artifact.set() methods | size:S
- [ ] S1.1.3: Implement Artifact.to_dict() and Artifact.from_dict() | size:S
- [ ] S1.1.4: Implement Artifact.__str__() and Artifact.__repr__() | size:S

### T1.2: Intent Data Structure
- [ ] S1.2.1: Define Intent dataclass with description, requirements, constraints | size:S
- [ ] S1.2.2: Implement Intent.__str__() and Intent.__repr__() | size:S

### T1.3: Observation Data Structure
- [ ] S1.3.1: Define Observation dataclass with success, output, error, execution_time, exit_code, metadata, timestamp | size:S
- [ ] S1.3.2: Implement Observation.__str__() | size:S

### T1.4: Constraints Data Structure
- [ ] S1.4.1: Define Constraints dataclass with requirements and checks | size:S
- [ ] S1.4.2: Implement Constraints.add() and Constraints.__iter__() | size:S

## M2: Environment Abstraction
### T2.1: Abstract Environment Base Class
- [ ] S2.1.1: Define Environment abstract base class with observe() method | size:S
- [ ] S2.1.2: Implement Environment.__repr__() | size:S

### T2.2: PythonEnvironment
- [ ] S2.2.1: Implement PythonEnvironment.__init__() with timeout and work_dir | size:S
- [ ] S2.2.2: Implement PythonEnvironment.observe() executing Python code in subprocess | size:M
- [ ] S2.2.3: Implement timeout handling for subprocess | size:M
- [ ] S2.2.4: Implement TimeoutExpired exception handling | size:M
- [ ] S2.2.5: Implement general exception handling in observe() | size:M
- [ ] S2.2.6: Implement PythonEnvironment.__repr__() | size:S

## M3: Diagnosis
### T3.1: Diagnosis Data Structure
- [ ] S3.1.1: Define Diagnosis dataclass with violated_constraint, probable_cause, affected_region, confidence, correction_hints, metadata | size:S
- [ ] S3.1.2: Implement Diagnosis.is_high_confidence() method | size:S
- [ ] S3.1.3: Implement Diagnosis.__str__() method | size:S

## M4: Mutation
### T4.1: Mutation Data Structure
- [ ] S4.1.1: Define Mutation dataclass with artifact, changes, description | size:S
- [ ] S4.1.2: Implement Mutation.apply() method | size:S
- [ ] S4.1.3: Implement Mutation._apply_recursive() helper method | size:S
- [ ] S4.1.4: Implement Mutation.__str__() method | size:S

### T4.2: Minimal Mutation Logic
- [ ] S4.2.1: Implement minimal_mutation() function | size:M
- [ ] S4.2.2: Ensure preservation of working structure | size:M
- [ ] S4.2.3: Ensure local correction preference | size:M
- [ ] S4.2.4: Ensure minimal change preference | size:M

## M5: Generator Interface
### T5.1: Generator Abstract Base Class
- [ ] S5.1.1: Define Generator abstract class with generate() method | size:S
- [ ] S5.1.2: Define Generator abstract class with correct() method | size:S
- [ ] S5.1.3: Implement Generator interface documentation | size:S

### T5.2: PlaceholderGenerator
- [ ] S5.2.1: Implement PlaceholderGenerator.generate() | size:S
- [ ] S5.2.2: Implement PlaceholderGenerator.correct() | size:S

## M6: Feedback History
### T6.1: FeedbackHistory Data Structure
- [ ] S6.1.1: Define FeedbackHistory class with _history list | size:S
- [ ] S6.1.2: Implement FeedbackHistory.record() method | size:S
- [ ] S6.1.3: Implement FeedbackHistory.current_attempt() method | size:S
- [ ] S6.1.4: Implement FeedbackHistory.get_last_diagnosis() method | size:S
- [ ] S6.1.5: Implement FeedbackHistory.get_last_observation() method | size:S
- [ ] S6.1.6: Implement FeedbackHistory.detect_stagnation() method | size:M
- [ ] S6.1.7: Implement FeedbackHistory.__str__() method | size:S
- [ ] S6.1.8: Implement FeedbackHistory.to_dict() classmethod | size:S
- [ ] S6.1.9: Implement FeedbackHistory.from_dict() classmethod | size:S

## M7: CoherenceEngine
### T7.1: CoherenceEngine Main Loop
- [ ] S7.1.1: Implement CoherenceEngine.__init__() | size:S
- [ ] S7.1.2: Implement CoherenceEngine.run() main loop | size:M
- [ ] S7.1.3: Implement _check_constraints() helper method | size:S
- [ ] S7.1.4: Implement _diagnose() helper method | size:M
- [ ] S7.1.5: Implement stagnation detection and break | size:M

### T7.2: CoherenceEngine Diagnostics
- [ ] S7.2.1: Implement _diagnose() method with error detection | size:M
- [ ] S7.2.2: Implement affected region identification from source | size:M
- [ ] S7.2.3: Implement correction hints generation | size:M

### T7.3: CoherenceEngine Convenience
- [ ] S7.3.1: Implement run_corrective_loop() convenience function | size:S

## M8: Integration Tests
### T8.1: Valid Artifact Succeeds Immediately
- [ ] S8.1.1: Test that a valid artifact succeeds on first attempt | size:S
- [ ] S8.1.2: Verify history records single attempt | size:S

### T8.2: Broken Artifact Correction
- [ ] S8.2.1: Test broken artifact is corrected after environmental failure | size:S
- [ ] S8.2.2: Verify correction preserves working portions | size:M

### T8.3: History Recording
- [ ] S8.3.1: Test history records each attempt | size:S
- [ ] S8.3.2: Verify history detect_stagnation works | size:S

### T8.4: Repeated Failure Detection
- [ ] S8.4.1: Test repeated identical failure detection | size:S
- [ ] S8.4.2: Verify STAGNATION state is produced | size:S

### T8.5: Replaceable Environment
- [ ] S8.5.1: Test environment is replaceable independently of generator | size:M

### T8.6: Preservation During Correction
- [ ] S8.6.1: Test working portions preserved conceptually during correction | size:M

## M9: README and Demo
### T9.1: README Creation
- [ ] S9.1.1: Write minimal README explaining the architecture | size:S
- [ ] S9.1.2: Document core types and their relationships | size:S
- [ ] S9.1.3: Document the corrective feedback loop | size:S

### T9.2: Runnable Demonstration
- [ ] S9.2.1: Create main.py with runnable demonstration | size:M
- [ ] S9.2.2: Demonstrate valid artifact succeeding immediately | size:M
- [ ] S9.2.3: Demonstrate broken artifact correction | size:M
- [ ] S9.2.4: Show history recording in action | size:M

## M10: Final Verification
### T10.1: Full Test Suite
- [ ] S10.1.1: Run all tests and ensure pass | size:M
- [ ] S10.1.2: Verify no syntax errors in any module | size:M
- [ ] S10.1.3: Verify import chain works end-to-end | size:S

### T10.2: Mission Completion
- [ ] S10.2.1: Ensure ALL TODO items marked [x] | size:M
- [ ] S10.2.2: Conclude mission with verified completion | size:M