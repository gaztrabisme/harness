from collections import OrderedDict


class LRUCache:
    def __init__(self, capacity: int):
        if capacity < 1:
            raise ValueError("capacity must be >= 1")
        self.capacity = capacity
        self._cache = OrderedDict()

    def get(self, key):
        if key not in self._cache:
            return None
        self._cache.move_to_end(key)
        return self._cache[key]

    def put(self, key, value):
        if key in self._cache:
            self._cache.move_to_end(key)
            self._cache[key] = value
            return
        if len(self._cache) >= self.capacity:
            self._cache.popitem(last=False)  # pop the first (LRU) item
        self._cache[key] = value
