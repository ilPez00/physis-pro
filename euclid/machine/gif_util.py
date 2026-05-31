import os
from PIL import Image, ImageDraw
import numpy as np
from elaborate import subdivide_polygon

def create_elaboration_gif(graph, entry, config, output_gif_path):
    os.makedirs(os.path.dirname(output_gif_path), exist_ok=True)
    frames = []
    for gen in range(config['max_generation'] + 1):
        img = Image.new('RGB', (640, 480), color='white')
        draw = ImageDraw.Draw(img)
        for primitive in graph.nodes:
            coords = primitive.coords
            vertices = [(float(v[0][0]), float(v[0][1])) for v in coords]
            subdivided = subdivide_polygon(vertices, gen, 20.0)
            draw.polygon(subdivided, outline='blue')
        frames.append(img)
    
    # Save using Pillow
    frames[0].save(output_gif_path, save_all=True, append_images=frames[1:], duration=500, loop=0)
