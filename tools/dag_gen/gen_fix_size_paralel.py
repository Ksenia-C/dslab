import argparse
from common import *
from copy import deepcopy
import pathlib

parser = argparse.ArgumentParser(description="Synthetic DAG generator")
parser.add_argument("--file_name", type=str,
                    help="path to gen file")
parser.add_argument("-n", "--count", type=int, default=10,
                    help="task count")
parser.add_argument("--thread_cnt", type=int, default=4,
                    help="thread count")

args = parser.parse_args()

graph_components_dir = pathlib.Path("/home/ksenia/dslab/tools/dag_gen/gen_multithread_program_tmp/")

# one_thread_cnt = (args.count - 5 - 2) // args.thread_cnt
# main_thread_cnt = args.count - (one_thread_cnt + 1) * args.thread_cnt - 2
main_thread_cnt = 5
one_thread_cnt = 3

mainThreadGraph = GraphParams(n=main_thread_cnt, fat=0.6).gen_sub_graph(graph_components_dir, 'main_thread')
oneThreadGraph = GraphParams(n=one_thread_cnt, fat=0.7).gen_sub_graph(graph_components_dir, 'one_thread')

resultGraph = Graph()
main_thread = SubGraph.graph_from_dot(graph_components_dir / "main_thread.dot")
main_thread.eat_end()
main_thread.eat_start()
one_thread = SubGraph.graph_from_dot(graph_components_dir / "one_thread.dot")
one_thread.eat_start()
end_sync = SubGraph.get_single_node()

# NUM_OF_PARALLEL = args.thread_cnt  # 20
NUM_OF_PARALLEL = (args.count - main_thread_cnt - 5) // one_thread_cnt
resultGraph.mesh_sub_direct(0, deepcopy(main_thread), f"main")
for i in range(NUM_OF_PARALLEL):
    resultGraph.mesh_sub_direct(0, deepcopy(one_thread), f"thread_{i}")

resultGraph.mesh_sub_direct(1, end_sync, f"end")
for i in range(NUM_OF_PARALLEL):
    resultGraph.add_sub(i + 2, NUM_OF_PARALLEL + 2)

resultGraph.write_to_dot(args.file_name)
