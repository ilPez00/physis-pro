import cv2
import numpy as np
import os
import time

class PerceptionError(Exception):
    pass

def capture_frame(source):
    if isinstance(source, int):
        cap = cv2.VideoCapture(source)
        if not cap.isOpened():
            raise PerceptionError(f"Could not open camera: {source}")
        ret, frame = cap.read()
        cap.release()
        if not ret:
            raise PerceptionError("Failed to capture frame")
        return frame
    elif isinstance(source, str):
        if not os.path.exists(source):
            raise PerceptionError(f"File not found: {source}")
        frame = cv2.imread(source)
        if frame is None:
            raise PerceptionError(f"Failed to read image file: {source}")
        return frame
    else:
        raise PerceptionError("Invalid source type")

def detect_edges(frame, config):
    gray = cv2.cvtColor(frame, cv2.COLOR_BGR2GRAY)
    blurred = cv2.GaussianBlur(gray, (config['gaussian_blur'], config['gaussian_blur']), 0)
    edges = cv2.Canny(blurred, config['canny_low'], config['canny_high'])
    return edges

def extract_contours(edge_map, config):
    # CHANGED: RETR_TREE returns full hierarchy
    contours, hierarchy = cv2.findContours(edge_map, cv2.RETR_TREE, cv2.CHAIN_APPROX_SIMPLE)
    
    # Filter by area, keeping hierarchy in mind
    filtered_indices = [i for i, cnt in enumerate(contours) if cv2.contourArea(cnt) >= config['min_contour_area']]
    
    # Return contours and their original hierarchy
    return [contours[i] for i in filtered_indices], hierarchy[0][filtered_indices]

def score_saliency(contours, frame_shape):
    if not contours:
        return 0.0
    return min(1.0, len(contours) / 10.0)

def save_observation(frame, observations_dir):
    filename = f"{int(time.time())}.jpg"
    path = os.path.join(observations_dir, filename)
    cv2.imwrite(path, frame)
    return filename
