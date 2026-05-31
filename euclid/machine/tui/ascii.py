import numpy as np

def contours_to_ascii(contours, frame_shape, cols=40, rows=20):
    grid = np.zeros((rows, cols), dtype=int)
    h, w = frame_shape[:2]
    for cnt in contours:
        for point in cnt:
            x, y = point[0]
            col = int(x * cols / w)
            row = int(y * rows / h)
            if 0 <= col < cols and 0 <= row < rows:
                grid[row, col] += 1
    chars = [' ', '░', '▒', '▓', '█']
    return "\n".join(["".join([chars[min(val, 4)] for val in row]) for row in grid])

def svg_to_ascii(svg_path, cols=40, rows=20):
    # Stub: simplified rasterization of lines
    return "SVG Preview Placeholder"

def graph_to_ascii(graph, cols=40, rows=20):
    # Stub: showing bounding boxes
    return "Graph Preview Placeholder"
