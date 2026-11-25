import json
import os
import unittest
from io import BytesIO
from unittest.mock import patch
from luna import BUDGET, Luna, MAX_CALLS


class LunaBudgetTests(unittest.TestCase):
    @patch.dict(os.environ, GIL_HARBOR_API_KEY="unit-test")
    def test_usage_and_limits(self):
        model = Luna()
        messages = [{"role":"user", "content":"Select a statistic."}]
        response = {"status":"completed", "usage":{"input_tokens":100, "output_tokens":20},
                    "output":[{"type":"message","content":[{"type":"output_text","text":'{"answer":"mean"}'}]}]}
        with patch("urllib.request.urlopen", return_value=BytesIO(json.dumps(response).encode())) as api:
            self.assertEqual(model.ask(messages, ["mean","median"]), "mean")
            self.assertAlmostEqual(model.usage()["estimated_usd"], 0.000044)
            model.reserved = BUDGET
            with self.assertRaisesRegex(RuntimeError, "limit"):
                model.ask(messages, [])
            model.reserved = 0
            model.calls = MAX_CALLS
            with self.assertRaisesRegex(RuntimeError, "limit"):
                model.ask(messages, [])
            self.assertEqual(api.call_count, 1)
