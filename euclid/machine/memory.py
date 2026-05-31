import json
import os
import time
from dataclasses import dataclass, asdict

@dataclass
class MemoryEntry:
    hash: str
    label: str
    generation: int
    count: int
    last_seen: str
    svgs: list[str]
    observations: list[str]
    status: str

@dataclass
class Abstraction:
    graph_hash: str
    abstract_hash: str
    label: str

def lookup(hash, memory_dir):
    for root, dirs, files in os.walk(memory_dir):
        if 'meta.json' in files:
            path = os.path.join(root, 'meta.json')
            with open(path, 'r') as f:
                data = json.load(f)
                if data['hash'] == hash:
                    return MemoryEntry(**data)
    return None

def store(entry, memory_dir):
    save_dir = os.path.join(memory_dir, entry.hash)
    os.makedirs(save_dir, exist_ok=True)
    with open(os.path.join(save_dir, 'meta.json'), 'w') as f:
        json.dump(asdict(entry), f, indent=2)

def store_abstraction(abs_entry, memory_dir):
    abs_dir = os.path.join(memory_dir, 'abstractions', abs_entry.abstract_hash)
    os.makedirs(abs_dir, exist_ok=True)
    with open(os.path.join(abs_dir, 'meta.json'), 'w') as f:
        json.dump(asdict(abs_entry), f, indent=2)

def lookup_abstraction(graph_hash, memory_dir):
    abs_dir = os.path.join(memory_dir, 'abstractions')
    for root, dirs, files in os.walk(abs_dir):
        if 'meta.json' in files:
            with open(os.path.join(root, 'meta.json'), 'r') as f:
                data = json.load(f)
                if data['graph_hash'] == graph_hash:
                    return Abstraction(**data)
    return None

def append_log(event, log_file):
    with open(log_file, 'a') as f:
        f.write(json.dumps(event) + '\n')
