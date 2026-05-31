import sys
import os
import numpy as np
import cv2
import tomllib
sys.path.append(os.path.join(os.getcwd(), 'euclid', 'machine'))
from perceive import detect_edges, extract_contours
from recognize import classify_contours

with open('euclid/machine/config.toml', 'rb') as f:
    config = tomllib.load(f)

def create_image(shape_type):
    img = np.zeros((480, 640, 3), dtype=np.uint8)
    if shape_type == 'triangle':
        pts = np.array([[320, 100], [200, 300], [440, 300]], np.int32)
        cv2.fillPoly(img, [pts], (255, 255, 255))
    return img

frame = create_image('triangle')
edges = detect_edges(frame, config['perception'])
# Note: extract_contours uses cv2.RETR_TREE, so we need to process it differently
# The hierarchy returned is [[next, prev, child, parent]]
contours, hierarchy = extract_contours(edges, config['perception'])
# Find the parent contour (the triangle)
parent_contour = None
for i, h in enumerate(hierarchy):
    if h[3] == -1:
        parent_contour = contours[i]
        break

# The contour classification needs to work on contours and hierarchy
# Let's call it and see
primitives = classify_contours(contours, hierarchy, config['recognition'], frame)

print(f"Primitives found: {len(primitives)}")
assert len(primitives) > 0
print(f"Primitive color: {primitives[0].color}")
assert all(c > 200 for c in primitives[0].color)
print("Color recognition tests passed.")
