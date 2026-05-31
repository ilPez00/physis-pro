import asyncio
import tomllib
import os
import datetime
from textual.app import App, ComposeResult
from textual.containers import Grid
from tui.panels import PerceptionPanel, MemoryPanel, ElaborationPanel, LogPanel
from perceive import capture_frame, detect_edges, extract_contours, save_observation
from recognize import classify_contours
from compose import build_graph
from memory import lookup, store, append_log, MemoryEntry, lookup_abstraction, store_abstraction, Abstraction
from elaborate import elaborate
from gif_util import create_elaboration_gif
from intellect import contemplate
from track import PrimitiveTracker
from plot import plot
from perceive import capture_frame, detect_edges, extract_contours, save_observation

class EuclidApp(App):
    # ... (rest of code)
    async def on_mount(self):
        with open("euclid/machine/config.toml", "rb") as f:
            self.config = tomllib.load(f)
        self.source = "/home/gio/Immagini/image.jpg"
        self.tracker = PrimitiveTracker() # Initialize tracker
        asyncio.create_task(self.cycle_loop())

    async def cycle_loop(self):
        while True:
            try:
                frame = capture_frame(self.source)
                edges = detect_edges(frame, self.config['perception'])
                contours, hierarchy = extract_contours(edges, self.config["perception"])
                primitives = classify_contours(contours, hierarchy, self.config["recognition"], frame)
                
                # Update tracker
                track_ids = self.tracker.update(primitives)
                
                graph = build_graph(primitives, self.config['composition'])
                
                # Check for abstractions
                abs_entry = lookup_abstraction(graph.hash, self.config['memory']['memory_dir'])
                
                # Memory handling
                entry = lookup(graph.hash, self.config['memory']['memory_dir'])
                if not entry:
                    entry = MemoryEntry(
                        hash=graph.hash,
                        label=abs_entry.label if abs_entry else f"{graph.hash}",
                        generation=0,
                        count=1,
                        last_seen=datetime.datetime.now().isoformat(),
                        svgs=[],
                        observations=[],
                        status='new'
                    )
                else:
                    entry.generation = min(entry.generation + 1, self.config['elaboration']['max_generation'])
                
                # Elaboration
                output_filename = f"gen_{entry.generation}.svg"
                output_path = os.path.join(self.config['memory']['output_dir'], output_filename)
                elaborate(graph, entry, self.config['elaboration'], output_path)
                
                # GIF generation
                gif_path = os.path.join(self.config['memory']['output_dir'], f"{graph.hash}_process.gif")
                create_elaboration_gif(graph, entry, self.config['elaboration'], gif_path)
                
                # Physical Plot
                plot(output_path, self.config)
                
                # Intellection loop
                thought_perceived = contemplate(output_path, contours)
                
                # Update memory
                obs_filename = save_observation(frame, self.config['memory']['observations_dir'])
                entry.observations.append(obs_filename)
                store(entry, self.config['memory']['memory_dir'])
                
                # New abstraction learning (stub logic)
                if entry.count > 5 and not abs_entry:
                    new_abs = Abstraction(graph.hash, f"abs_{graph.hash[:5]}", f"concept_{graph.hash[:5]}")
                    store_abstraction(new_abs, self.config['memory']['memory_dir'])

                append_log({'timestamp': datetime.datetime.now().isoformat(), 'phase': 'intellection', 'hash': graph.hash, 'gen': entry.generation}, self.config['memory']['log_file'])
            except Exception as e:
                import traceback
                traceback.print_exc()
            
            await asyncio.sleep(self.config['elaboration']['contemplate_secs'])


if __name__ == "__main__":
    app = EuclidApp()
    app.run()
