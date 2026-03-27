"""
Simple file-based memory for project context.
Stores per-project facts and conversation history.
"""

import json
from pathlib import Path
from typing import Optional


class Memory:
    def __init__(self, data_dir: str = "data/memory"):
        self.data_dir = Path(data_dir)
        self.data_dir.mkdir(parents=True, exist_ok=True)

    def save(self, namespace: str, key: str, value: dict):
        """Save a value under namespace/key."""
        ns_dir = self.data_dir / namespace
        ns_dir.mkdir(parents=True, exist_ok=True)
        path = ns_dir / f"{key}.json"
        path.write_text(json.dumps(value, indent=2, default=str))

    def load(self, namespace: str, key: str) -> Optional[dict]:
        """Load a value from namespace/key."""
        path = self.data_dir / namespace / f"{key}.json"
        if path.exists():
            return json.loads(path.read_text())
        return None

    def append(self, namespace: str, key: str, entry: dict):
        """Append to a list stored at namespace/key."""
        existing = self.load(namespace, key) or {"entries": []}
        existing["entries"].append(entry)
        self.save(namespace, key, existing)

    def search(self, namespace: str, query: str) -> list[dict]:
        """Simple keyword search across all entries in a namespace."""
        results = []
        ns_dir = self.data_dir / namespace
        if not ns_dir.exists():
            return results
        query_lower = query.lower()
        for path in ns_dir.glob("*.json"):
            try:
                data = json.loads(path.read_text())
                text = json.dumps(data).lower()
                if query_lower in text:
                    results.append({"key": path.stem, "data": data})
            except (json.JSONDecodeError, OSError):
                continue
        return results
