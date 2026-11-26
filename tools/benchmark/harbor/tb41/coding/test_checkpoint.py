import hashlib
from pathlib import Path
import tempfile
import unittest

from checkpoint import Checkpoint


class CheckpointTest(unittest.TestCase):
    def test_failed_edit_restores_existing_and_removes_new_file(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source, extra = root / "main", root / "extra"
            source.write_bytes(b"working")
            receipts = {"main": hashlib.sha256(b"working").hexdigest()}
            snapshot = Checkpoint()
            snapshot.accept(receipts)
            events = []
            for path in (source, extra):
                snapshot.before_write(path)
                path.write_bytes(b"broken")
                receipts[path.name] = hashlib.sha256(b"broken").hexdigest()
            self.assertTrue(snapshot.restore(receipts, root, lambda event, _: events.append(event)))
            self.assertEqual(source.read_bytes(), b"working")
            self.assertFalse(extra.exists())
            self.assertEqual(receipts, {"main": hashlib.sha256(b"working").hexdigest()})
            self.assertEqual(events, ["restore_started", "restore_completed"])
