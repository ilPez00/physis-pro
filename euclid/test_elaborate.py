import sys
import os
import numpy as np
sys.path.append(os.path.join(os.getcwd(), 'euclid', 'machine'))

from recognize import Primitive
from compose import Graph
from elaborate import elaborate
from memory import MemoryEntry

# Setup
output_path = 'euclid/machine/output/test.svg'
if not os.path.exists('euclid/machine/output'):
    os.makedirs('euclid/machine/output')

# Mock Primitive (using simple triangle vertices)
vertices = np.array([[320, 100], [200, 300], [440, 300]])
p = Primitive('triangle', 3, vertices, (200, 100, 240, 200), 'h1', 0.5, 1.0)
graph = Graph(nodes=[p], edges=[], hash='h1')
entry = MemoryEntry('h1', 'triangle', 1, 1, 'now', [], [], 'new')
config = {} # placeholder

elaborate(graph, entry, config, output_path)

assert os.path.exists(output_path)
print("Elaborate module tests passed (SVG created).")
