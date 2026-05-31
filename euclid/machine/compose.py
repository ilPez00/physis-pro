from dataclasses import dataclass
import numpy as np

@dataclass
class Edge:
    a: int
    b: int
    relation: str

@dataclass
class Graph:
    nodes: list
    edges: list[Edge]
    hash: str

def compute_relation(p1, p2, config):
    # 1. Check for containment
    # Bbox: (x, y, w, h)
    def is_contained(a, b):
        return (a[0] >= b[0] and a[1] >= b[1] and 
                (a[0] + a[2]) <= (b[0] + b[2]) and 
                (a[1] + a[3]) <= (b[1] + b[3]))

    if is_contained(p1.bbox, p2.bbox) or is_contained(p2.bbox, p1.bbox):
        return 'contained'

    # 2. Check for shared vertices
    # Convert contours to set of points for comparison
    pts1 = set(map(tuple, p1.coords.reshape(-1, 2)))
    pts2 = set(map(tuple, p2.coords.reshape(-1, 2)))
    shared = pts1.intersection(pts2)
    
    if len(shared) >= 2:
        return 'shared_edge'
    if len(shared) == 1:
        return 'shared_vertex'

    # 3. Check for near
    c1 = np.array([p1.bbox[0] + p1.bbox[2]/2, p1.bbox[1] + p1.bbox[3]/2])
    c2 = np.array([p2.bbox[0] + p2.bbox[2]/2, p2.bbox[1] + p2.bbox[3]/2])
    dist = np.linalg.norm(c1 - c2)
    if dist < config['proximity_threshold'] * 640:
        return 'near'
        
    return None

def graph_hash(primitives):
    primitive_hashes = sorted([p.hash for p in primitives])
    return "+".join(primitive_hashes)

def build_graph(primitives, config):
    edges = []
    for i in range(len(primitives)):
        for j in range(i + 1, len(primitives)):
            rel = compute_relation(primitives[i], primitives[j], config)
            if rel:
                edges.append(Edge(i, j, rel))
    
    return Graph(nodes=primitives, edges=edges, hash=graph_hash(primitives))
