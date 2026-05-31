import sys
import os
import numpy as np

# Add the machine directory to sys.path
sys.path.append(os.path.join(os.getcwd(), 'euclid', 'machine'))

from perceive import detect_edges, extract_contours, score_saliency
from tui.ascii import contours_to_ascii

# Mock a simple frame (black square)
frame = np.zeros((480, 640, 3), dtype=np.uint8)
cv2 = None # Not used here
# Just draw a white square
frame[100:300, 100:300] = 255

config = {
    'gaussian_blur': 5,
    'canny_low': 50,
    'canny_high': 150,
    'min_contour_area': 500
}

import cv2
edges = detect_edges(frame, config)
contours = extract_contours(edges, config)
saliency = score_saliency(contours, frame.shape)
ascii_out = contours_to_ascii(contours, frame.shape)

print(f"Saliency: {saliency}")
print("ASCII Preview:")
print(ascii_out)
