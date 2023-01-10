from common import *
from copy import deepcopy
import pathlib

graph_components_dir = pathlib.Path("gen_multithread_program_tmp/")

mainThreadGraph = GraphParams(n=2, fat=0.7).gen_sub_graph(graph_components_dir, 'main_thread')
oneThreadGraph = GraphParams(n=2, fat=0.7).gen_sub_graph(graph_components_dir, 'one_thread')
endSyncGraph = GraphParams(fat=0.7, n=3, density=0.9).gen_sub_graph(graph_components_dir, 'end_sync')

resultGraph = Graph()
main_thread = SubGraph.graph_from_dot(graph_components_dir / "main_thread.dot")
main_thread.eat_end()
one_thread = SubGraph.graph_from_dot(graph_components_dir / "one_thread.dot")
# one_thread.eat_end()
one_thread.eat_start()
end_sync = SubGraph.graph_from_dot(graph_components_dir / "end_sync.dot")

NUM_OF_PARALEL = 20
resultGraph.add_sub_direct(0, deepcopy(main_thread), f"main")
for i in range(NUM_OF_PARALEL):
    resultGraph.mesh_sub_direct(0, deepcopy(one_thread), f"thread_{i}")

resultGraph.add_sub_direct(1, end_sync, f"end")
for i in range(NUM_OF_PARALEL):
    resultGraph.mesh_sub(i + 2, NUM_OF_PARALEL + 2)

resultGraph.write_to_dot('final.dot')
