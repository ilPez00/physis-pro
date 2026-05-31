import sys
import os
import numpy as np
sys.path.append(os.path.join(os.getcwd(), 'euclid', 'machine'))

from recognize import Primitive
from compose import build_graph

# Mock primitives
# Triangle at (10,10)
p1 = Primitive('triangle', 3, np.array([[[10,10], [20,10], [15,20]]]), (10, 10, 10, 10), 'h1', 0.5, 1.0)
# Quad inside (near)
p2 = Primitive('quad', 4, np.array([[[30,30], [40,30], [40,40], [30,40]]]), (30, 30, 10, 10), 'h2', 0.5, 1.0)

config = {'proximity_threshold': 0.15}

graph = build_graph([p1, p2], config)
print(f"Graph edges: {graph.edges}")
assert any(e.relation == 'near' for e in graph.edges)

# Test containment
p3 = Primitive('quad', 4, np.array([[[0,0], [100,0], [100,100], [0,100]]]), (0, 0, 100, 100), 'h3', 0.5, 1.0)
graph2 = build_graph([p1, p3], config)
print(f"Graph 2 edges: {graph2.edges}")
assert any(e.relation == 'contained' for e in graph2.edges)

print("Spatial relations tests passed.")
