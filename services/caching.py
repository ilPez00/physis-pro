import hashlib

class SemanticCache:
    def __init__(self, threshold=0.98):
        self.cache = {} # Key: Hash of prompt, Value: Response
        self.threshold = threshold

    def _get_key(self, prompt):
        return hashlib.sha256(prompt.encode()).hexdigest()

    def get(self, prompt):
        key = self._get_key(prompt)
        return self.cache.get(key)

    def set(self, prompt, response):
        key = self._get_key(prompt)
        self.cache[key] = response

    def invalidate(self, prompt_pattern):
        # Simple invalidation
        keys_to_del = [k for k in self.cache.keys() if prompt_pattern in k]
        for k in keys_to_del:
            del self.cache[k]
