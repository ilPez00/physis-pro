from textual.widgets import Static

class PerceptionPanel(Static):
    def on_mount(self):
        self.update("Perception Module Initialized")

class MemoryPanel(Static):
    def on_mount(self):
        self.update("Memory Module Initialized")

class ElaborationPanel(Static):
    def on_mount(self):
        self.update("Elaboration Module Initialized")

class LogPanel(Static):
    def on_mount(self):
        self.update("Log Module Initialized")
