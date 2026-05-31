import numpy as np

class PrimitiveTracker:
    def __init__(self):
        self.previous_primitives = []
        self.next_id = 0

    def update(self, current_primitives):
        # Very simple tracking: match by nearest centroid
        tracked = []
        for curr in current_primitives:
            best_match = None
            min_dist = 50 # threshold
            curr_center = np.array([curr.bbox[0] + curr.bbox[2]/2, curr.bbox[1] + curr.bbox[3]/2])
            
            for prev_id, prev_center, prev_p in self.previous_primitives:
                dist = np.linalg.norm(curr_center - prev_center)
                if dist < min_dist:
                    min_dist = dist
                    best_match = prev_id
            
            if best_match is not None:
                tracked_id = best_match
            else:
                tracked_id = self.next_id
                self.next_id += 1
            
            tracked.append((tracked_id, curr_center, curr))
            
        self.previous_primitives = tracked
        return [t[0] for t in tracked] # return IDs for primitives
