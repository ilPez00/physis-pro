import sys
import os
import datetime
import tomllib
sys.path.append(os.path.join(os.getcwd(), 'euclid', 'machine'))
from perceive import capture_frame, detect_edges, extract_contours
from recognize import classify_contours
from compose import build_graph
from memory import lookup, store, append_log, MemoryEntry, lookup_abstraction, store_abstraction, Abstraction
from elaborate import elaborate
from gif_util import create_elaboration_gif
from intellect import contemplate

with open("euclid/machine/config.toml", "rb") as f:
    config = tomllib.load(f)

source = "/home/gio/Immagini/image.jpg"
frame = capture_frame(source)
edges = detect_edges(frame, config['perception'])
contours, hierarchy = extract_contours(edges, config['perception'])
primitives = classify_contours(contours, hierarchy, config['recognition'], frame)
graph = build_graph(primitives, config['composition'])

entry = lookup(graph.hash, config['memory']['memory_dir'])
if not entry:
    entry = MemoryEntry(
        hash=graph.hash,
        label=f"{graph.hash}",
        generation=0,
        count=1,
        last_seen=datetime.datetime.now().isoformat(),
        svgs=[],
        observations=[],
        status='new'
    )
else:
    entry.generation = min(entry.generation + 1, config['elaboration']['max_generation'])

output_filename = f"gen_{entry.generation}.svg"
output_path = os.path.join(config['memory']['output_dir'], output_filename)
elaborate(graph, entry, config['elaboration'], output_path)

gif_path = os.path.join(config['memory']['output_dir'], f"{graph.hash}_process.gif")
create_elaboration_gif(graph, entry, config['elaboration'], gif_path)

print("Test complete.")
