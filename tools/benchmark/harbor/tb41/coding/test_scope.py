"""What a submission may contain follows what each task collects and builds with."""
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

with patch.dict(os.environ, TB_TASK="react-lead-form"):
    import solver


class ScopeTests(unittest.TestCase):
    def test_the_react_project_files_the_verifier_builds_with_are_writable_and_the_specification_is_not(self):
        with tempfile.TemporaryDirectory() as temp, patch.multiple(solver, APP=Path(temp).resolve(), TASK="react-lead-form"):
            for name in ("package.json", "tsconfig.json", "vite.config.ts", "index.html", "src/lib/submitLead.ts",
                         "src/components/LeadForm.tsx", "src/app.css"):
                self.assertEqual(solver.source(name, True), Path(temp).resolve() / name)
            for name in ("package-lock.json", "src/spec/lead_schema.json", "src/data/lead_input.json", "README.md",
                         "node_modules/x/index.ts", "src/lib/data.json", "../outside.ts"):
                with self.subTest(name=name), self.assertRaises(ValueError) as refused:
                    solver.source(name, True)
            # Reading stays open to everything in the task directory.
            self.assertEqual(solver.source("src/spec/lead_schema.json"), Path(temp).resolve() / "src/spec/lead_schema.json")

    def test_the_other_tasks_keep_their_scope(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp).resolve()
            with patch.multiple(solver, APP=root, TASK="ks-solver-cpp"):
                solver.source("solution.cpp", True); solver.source("helper.hpp", True)
                for name in ("oracle.hpp", "package.json", "main.cpp"):
                    with self.assertRaisesRegex(ValueError, "solution.cpp and .hpp files other than oracle.hpp"):
                        solver.source(name, True)
            with patch.multiple(solver, APP=root, TASK="vf2-speedup-networkx"):
                solver.source("fast_networkx.py", True); solver.source("fast_networkx/_core.c", True)
                with self.assertRaisesRegex(ValueError, "fast_networkx.py and anything under fast_networkx/"):
                    solver.source("setup.py", True)


if __name__ == "__main__":
    unittest.main()
