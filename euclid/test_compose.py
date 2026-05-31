import sys
import os
import numpy as np
sys.path.append(os.path.join(os.getcwd(), 'euclid', 'machine'))

from recognize import Primitive
from compose import build_graph

# Mock primitives
p1 = Primitive('triangle', 3, np.array([]), (10, 10, 50, 50), 'h1', 0.5, 1.0)
p2 = Primitive('quad', 4, np.array([]), (20, 20, 50, 50), 'h2', 0.5, 1.0)

config = {'proximity_threshold': 0.15}

graph = build_graph([p1, p2], config)
print(f"Graph hash: {graph.hash}")
print(f"Edges: {len(graph.edges)}")

assert len(graph.edges) == 1
assert graph.edges[0].relation == 'near'
print("Compose module tests passed.")
