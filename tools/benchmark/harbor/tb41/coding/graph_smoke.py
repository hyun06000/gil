"""Exercise the public drop-in API from a separate caller using small examples."""
import unittest

import fast_networkx as fnx


class GraphClient(unittest.TestCase):
    def test_graph_kind_and_conversion(self):
        for cls, directed in [(fnx.Graph, False), (fnx.DiGraph, True)]:
            with self.subTest(cls=cls):
                graph = cls()
                graph.add_edge("a", "b")
                self.assertEqual(graph.is_directed(), directed)
                self.assertEqual(graph.has_edge("b", "a"), not directed)
                self.assertFalse(graph.to_undirected().is_directed())
                self.assertTrue(graph.to_directed().is_directed())

    def test_copy_and_attributes(self):
        graph = fnx.Graph()
        graph.add_node("oak", group=2)
        graph.add_edge("oak", "elm")
        copy = graph.copy()
        copy.remove_node("elm")
        self.assertTrue(graph.has_node("elm"))
        self.assertEqual(copy.nodes["oak"]["group"], 2)

    def test_public_mapping_functions(self):
        left, right = fnx.DiGraph(), fnx.DiGraph()
        left.add_edge("head", "tail")
        right.add_edge(3, 7)
        expected = {"head": 3, "tail": 7}
        self.assertTrue(fnx.vf2pp_is_isomorphic(left, right))
        self.assertEqual(fnx.vf2pp_isomorphism(left, right), expected)
        self.assertEqual(list(fnx.vf2pp_all_graph_isomorphisms(left, right)), [expected])
        right.add_node(9)
        self.assertFalse(fnx.vf2pp_is_isomorphic(left, right))


if __name__ == "__main__":
    unittest.main()
