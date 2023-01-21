# digraph G {
# task_1_main [size="2.129140e+01"];
# task_2_main [size="1.540917e+01"];
# task_1_main -> task_2_main [size="2.003192e+01"];

import json
import argparse


parser = argparse.ArgumentParser()
parser.add_argument('--file', type=str)
args = parser.parse_args()

worse_descr = {}
with open(args.file, 'r') as file:
    worse_descr = json.load(file)


worse_graph = worse_descr['graph']

with open("save.dot", 'w') as file:
    print("digraph G {", file=file)
    for task in worse_graph['tasks']:
        print(f'{task["name"]} [size="{task["flops"]}"];', file=file)
    for edge in worse_graph['data_items']:
        print(f'{edge["name"]} [size="{edge["size"]}"];', file=file)
    print("}", file=file)
