import json
from physis.services.distiller import PhysisDistiller

class PDCAEngine:
    def __init__(self):
        self.state_store = {} # In memory state store for this session
        self.distiller = PhysisDistiller()

    def advance_phase(self, goal_id, new_phase, input_data):
        """Advances the PDCA state and distills input."""
        if goal_id not in self.state_store:
            self.state_store[goal_id] = {"phase": "plan", "history": []}

        # Distill input data based on current phase
        distilled = self.distiller.distill(input_data, self.state_store[goal_id]["phase"])
        
        # Save distilled result to history
        self.state_store[goal_id]["history"].append({
            "phase": self.state_store[goal_id]["phase"],
            "distilled_output": distilled
        })
        
        # Advance phase
        self.state_store[goal_id]["phase"] = new_phase
        return {"goal_id": goal_id, "new_phase": new_phase, "status": "advanced"}

    def get_state(self, goal_id):
        return self.state_store.get(goal_id)
