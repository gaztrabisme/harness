"""Acceptance oracle for the LRUCache shakedown kata.

This file is the FIXED success criterion — written before the agent runs, never
edited to fit the result. The agent's job is to write `lru_cache.py` (next to
this file) exporting `LRUCache(capacity)` so every case below passes:

    python3 -m unittest discover -s dogfood/lru-cache -t dogfood/lru-cache -v

Cases are sequence-based on purpose: passing them requires real LRU semantics
(eviction order + recency refresh), not a lookup table keyed on inputs.
"""

import unittest

from lru_cache import LRUCache


class TestLRUCache(unittest.TestCase):
    def test_put_then_get(self):
        c = LRUCache(2)
        c.put("a", 1)
        self.assertEqual(c.get("a"), 1)

    def test_missing_returns_none(self):
        c = LRUCache(2)
        self.assertIsNone(c.get("nope"))

    def test_eviction_at_capacity(self):
        # Fill past capacity; the least-recently-used key is evicted.
        c = LRUCache(2)
        c.put("a", 1)
        c.put("b", 2)
        c.put("c", 3)  # evicts "a" (LRU)
        self.assertIsNone(c.get("a"))
        self.assertEqual(c.get("b"), 2)
        self.assertEqual(c.get("c"), 3)

    def test_get_refreshes_recency(self):
        # get() must mark a key most-recently-used so it survives the next evict.
        c = LRUCache(2)
        c.put("a", 1)
        c.put("b", 2)
        self.assertEqual(c.get("a"), 1)  # "a" now MRU, "b" is LRU
        c.put("c", 3)  # evicts "b", not "a"
        self.assertEqual(c.get("a"), 1)
        self.assertIsNone(c.get("b"))
        self.assertEqual(c.get("c"), 3)

    def test_update_existing_overwrites_and_refreshes(self):
        # Re-putting an existing key updates its value AND marks it MRU.
        c = LRUCache(2)
        c.put("a", 1)
        c.put("b", 2)
        c.put("a", 10)  # "a" now MRU with value 10, "b" is LRU
        c.put("c", 3)   # evicts "b"
        self.assertEqual(c.get("a"), 10)
        self.assertIsNone(c.get("b"))
        self.assertEqual(c.get("c"), 3)

    def test_capacity_one(self):
        c = LRUCache(1)
        c.put("a", 1)
        c.put("b", 2)  # evicts "a"
        self.assertIsNone(c.get("a"))
        self.assertEqual(c.get("b"), 2)

    def test_longer_sequence(self):
        # A longer interleaving of put/get to stress the recency bookkeeping.
        c = LRUCache(3)
        c.put(1, "one")
        c.put(2, "two")
        c.put(3, "three")
        self.assertEqual(c.get(1), "one")   # order (LRU→MRU): 2,3,1
        c.put(4, "four")                     # evicts 2
        self.assertIsNone(c.get(2))
        self.assertEqual(c.get(3), "three")  # order: 1,4,3
        c.put(5, "five")                     # evicts 1
        self.assertIsNone(c.get(1))
        self.assertEqual(c.get(4), "four")
        self.assertEqual(c.get(5), "five")
        self.assertEqual(c.get(3), "three")


if __name__ == "__main__":
    unittest.main(verbosity=2)
