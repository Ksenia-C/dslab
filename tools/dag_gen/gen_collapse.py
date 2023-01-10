from common import *
from copy import deepcopy
from typing import List, Dict
import pathlib

graph_components_dir = pathlib.Path("gen_multithread_program_tmp/")

topGraph = GraphParams(n=7, fat=0.5, density=0.2).gen_sub_graph(graph_components_dir, 'top')
bottomGraph = GraphParams(n=18, fat=0.7, density=0.2).gen_sub_graph(graph_components_dir, 'bottom')
lastGraph = GraphParams(n=10, fat=0.1, ccr=304).gen_sub_graph(graph_components_dir, 'last')

resultGraph = Graph()
top = SubGraph.graph_from_dot(graph_components_dir / "top.dot")
top.eat_end()
bottom = SubGraph.graph_from_dot(graph_components_dir / "bottom.dot")
bottom.eat_start()
last = SubGraph.graph_from_dot(graph_components_dir / "last.dot")
last.eat_start()

resultGraph.add_sub_direct(0, deepcopy(top), f"top")
resultGraph.mesh_sub_direct(1, deepcopy(bottom), f"bootom", density=0.1, jump=2)
resultGraph.mesh_sub_direct(2, deepcopy(last), f"last", density=0.1, jump=2)

resultGraph.write_to_dot('final.dot')
