import argparse
import pathlib
from wfcommons.wfchef.recipes import GenomeRecipe, BlastRecipe, BwaRecipe, CyclesRecipe, EpigenomicsRecipe, MontageRecipe, SeismologyRecipe, SoykbRecipe, SrasearchRecipe
from wfcommons import WorkflowGenerator
import networkx as nx
import matplotlib.pyplot as plt

parser = argparse.ArgumentParser(description="Synthetic DAG generator")
parser.add_argument("--file_name", type=str,
                    help="path to gen file")
parser.add_argument("-n", "--count", type=int, default=[10],
                    help="task count")

args = parser.parse_args()

generator = WorkflowGenerator(SrasearchRecipe.from_num_tasks(args.count))
workflow = generator.build_workflow()
workflow.write_json(pathlib.Path(f'{args.file_name}.json'))

# nx.draw(workflow.to_nx_digraph(), cmap = plt.get_cmap('jet'))
# plt.show()

result = []
files = {}
for node in workflow.nodes:
    task = workflow.nodes[node]["task"]
    task = task.as_dict()
    task['name'] = task['name'].replace('-', '_')
    result.append(f"{task['name']} [size=\"{task['runtime']}\"];")
    for file in task["files"]:
        name = file['name']
        if name not in files:
            files[name] = {'size': file['size']}
        if file['link'] == 'output':
            files[name]['output'] = files[name].get('output', []) + [task['name']]
        elif file['link'] == 'input':
            files[name]['input'] = files[name].get('input', []) + [task['name']]
        else:
            raise NotImplementedError

has_root = False
has_end = False

for edge in files.values():
    if 'output' not in edge:
        source = "root"
        has_root = True
    elif len(edge['output']) != 1:
        raise NotImplementedError
    else:
        source = edge['output'][0]
    if 'input' not in edge:
        edge['input'] = ['end']
        has_end = True
    for destination in edge['input']:
            result.append(f"{source} -> {destination} [size=\"{edge['size']}\"]")
if has_root:
    result.append("root [label=\"root\",size=\"0.0\"];")

if has_end:
    result.append("end [label=\"end\",size=\"0.0\"];")

with open(f"{args.file_name}.dot", "w") as file:
    print("digraph G {", file=file)
    for line in result:
        print(" ", line, file=file)
    print("}", file=file)
