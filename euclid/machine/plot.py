import os
import shutil

def plot(svg_path, config):
    # Stub for physical plotter integration.
    # In a real system, this would call vpype and pyserial
    output_path = os.path.join(config['memory']['output_dir'], "current.svg")
    shutil.copy(svg_path, output_path)
    print(f"Plotter stub: SVG sent to {output_path}")
