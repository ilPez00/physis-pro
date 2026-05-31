import svgwrite
import numpy as np
import os

def subdivide_polygon(vertices, depth, displacement):
    if depth == 0:
        return vertices
    
    new_vertices = []
    for i in range(len(vertices)):
        p1 = vertices[i]
        p2 = vertices[(i + 1) % len(vertices)]
        new_vertices.append(p1)
        
        # Midpoint
        mid = ((p1[0] + p2[0]) / 2, (p1[1] + p2[1]) / 2)
        # Koch-like outward displacement
        normal = (-(p2[1] - p1[1]), (p2[0] - p1[0]))
        length = np.sqrt(normal[0]**2 + normal[1]**2)
        if length > 0:
            norm_normal = (normal[0] / length, normal[1] / length)
            new_v = (mid[0] + norm_normal[0] * displacement, 
                     mid[1] + norm_normal[1] * displacement)
            new_vertices.append(new_v)
            
    return subdivide_polygon(new_vertices, depth - 1, displacement / 2)

def elaborate(graph, entry, config, output_path):
    os.makedirs(os.path.dirname(output_path), exist_ok=True)
    dwg = svgwrite.Drawing(output_path, profile='full', size=('640px', '480px'))
    
    for primitive in graph.nodes:
        # Convert color (BGR) to hex
        r, g, b = primitive.color[2], primitive.color[1], primitive.color[0]
        fill_color = f"#{r:02x}{g:02x}{b:02x}"
        
        # Draw main polygon
        coords = primitive.coords
        vertices = [(float(v[0][0]), float(v[0][1])) for v in coords]
        subdivided = subdivide_polygon(vertices, entry.generation, 20.0)
        
        # Build path string for polygon + holes
        path_data = f"M {subdivided[0][0]} {subdivided[0][1]} "
        for v in subdivided[1:]:
            path_data += f"L {v[0]} {v[1]} "
        path_data += "Z"
        
        # Add holes as negative paths
        for hole in primitive.holes:
            poly = hole.reshape(-1, 2)
            path_data += f" M {poly[0][0]} {poly[0][1]} "
            for v in poly[1:]:
                path_data += f"L {v[0]} {v[1]} "
            path_data += "Z"
        
        dwg.add(dwg.path(d=path_data, stroke='black', fill=fill_color, fill_rule='evenodd'))
        
    dwg.save()
