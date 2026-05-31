import sys
import os
import json
sys.path.append(os.path.join(os.getcwd(), 'euclid', 'machine'))
from memory import MemoryEntry, store, lookup, append_log

# Setup paths
memory_dir = 'euclid/machine/memory'
log_file = os.path.join(memory_dir, 'log.jsonl')
if not os.path.exists(memory_dir):
    os.makedirs(memory_dir)

# Test 1: Store and Lookup
entry = MemoryEntry(
    hash='t3_r1_a1',
    label='triangle',
    generation=0,
    count=1,
    last_seen='2026-05-31T12:00:00',
    svgs=['gen_0.svg'],
    observations=['obs1.jpg'],
    status='new'
)
store(entry, memory_dir)

retrieved = lookup('t3_r1_a1', memory_dir)
print(f"Retrieved hash: {retrieved.hash if retrieved else 'None'}")
assert retrieved is not None
assert retrieved.label == 'triangle'

# Test 2: Log append
event = {'timestamp': '2026-05-31T12:05:00', 'phase': 'intellection', 'hash': 't3_r1_a1'}
append_log(event, log_file)
with open(log_file, 'r') as f:
    last_line = f.readlines()[-1]
    logged_event = json.loads(last_line)
    print(f"Logged event phase: {logged_event['phase']}")
    assert logged_event['phase'] == 'intellection'

print("Memory module tests passed.")
