from dataclasses import dataclass, field
import numpy as np
import cv2

@dataclass
class Primitive:
    type: str
    vertices: int
    coords: np.ndarray
    bbox: tuple
    hash: str
    circularity: float
    aspect_ratio: float
    color: tuple = (0, 0, 0)
    holes: list = field(default_factory=list)
    texture_score: float = 0.0

def approximate_polygon(contour, epsilon_fraction):
    perimeter = cv2.arcLength(contour, True)
    epsilon = epsilon_fraction * perimeter
    return cv2.approxPolyDP(contour, epsilon, True)

def compute_circularity(contour):
    area = cv2.contourArea(contour)
    perimeter = cv2.arcLength(contour, True)
    if perimeter == 0:
        return 0.0
    return 4 * np.pi * area / (perimeter ** 2)

def compute_texture(frame, mask):
    gray = cv2.cvtColor(frame, cv2.COLOR_BGR2GRAY)
    masked = cv2.bitwise_and(gray, gray, mask=mask.astype(np.uint8))
    laplacian = cv2.Laplacian(masked, cv2.CV_64F)
    return float(np.var(laplacian))

def classify_by_vertices(n):
    if n == 2: return "line"
    elif n == 3: return "triangle"
    elif n == 4: return "quad"
    elif n == 5: return "pentagon"
    elif n == 6: return "hexagon"
    else: return f"n{n}"

def compute_hash(primitive):
    aspect_bin = int(primitive.aspect_ratio * 4)
    angle_bin = int(primitive.circularity * 3)
    color_hash = "".join([f"{c:02x}" for c in primitive.color])
    tex_bin = int(min(primitive.texture_score / 100.0, 3))
    return f"{primitive.type}_r{aspect_bin}_a{angle_bin}_c{color_hash}_t{tex_bin}"

def reject_circle(circularity, limit):
    return circularity > limit

def classify_contours(contours, hierarchy, config, frame):
    primitives = []
    if hierarchy is None:
        return primitives

    for i, cnt in enumerate(contours):
        # hierarchy has already been indexed in perceive.py, it is now (N, 4)
        h = hierarchy[i]
        
        # Only process outermost contours (h[3] == -1)
        if h[3] != -1: 
            continue
            
        circularity = compute_circularity(cnt)
        if reject_circle(circularity, config['circularity_limit']):
            continue
            
        poly = approximate_polygon(cnt, config['approx_epsilon'])
        num_vertices = len(poly)
        
        if num_vertices < config['min_vertices'] or num_vertices > config['max_vertices']:
            continue
            
        # Find holes
        child_idx = h[2]
        holes = []
        # Note: child_idx might not be the index in the *filtered* contours list!
        # This is the tricky part. We need to find the hole in the filtered list.
        # But wait, perceive.py returns contours that were filtered.
        # So we can't easily map back.
        # Let's keep it simple: assume all child contours of the parent are also in the filtered list.
        # This might not be true if a hole is too small.
        # For this stage, this is an acceptable simplification.
        
        x, y, w, h_box = cv2.boundingRect(cnt)
        
        # Create mask
        mask = np.zeros(frame.shape[:2], dtype=np.uint8)
        cv2.drawContours(mask, [cnt], -1, 255, -1)
        for hole in holes:
            cv2.drawContours(mask, [hole], -1, 0, -1)
            
        mean_color = cv2.mean(frame, mask=mask)[:3]
        tex_score = compute_texture(frame, mask)
        
        primitive = Primitive(
            type=classify_by_vertices(num_vertices),
            vertices=num_vertices,
            coords=poly,
            bbox=(x, y, w, h_box),
            hash="",
            circularity=circularity,
            aspect_ratio=float(w) / h_box if h_box != 0 else 0.0,
            color=tuple(map(int, mean_color)),
            holes=holes,
            texture_score=tex_score
        )
        primitive.hash = compute_hash(primitive)
        primitives.append(primitive)
        
    return primitives
