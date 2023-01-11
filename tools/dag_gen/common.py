import random
from typing import List, Dict
import subprocess
import pathlib


class GraphParams:
    def __init__(self, n=10, fat=0.3, density=0.5, ccr=1300):
        self.n = n
        self.fat = fat
        self.density = density
        self.ccr = ccr

    def gen_sub_graph(self, graph_components_dir: pathlib.Path, name: str):
        try:
            subprocess.check_output([
                "/usr/bin/python", "/home/ksenia/dslab/tools/dag_gen/dag_gen.py", graph_components_dir,
                "--fat", str(self.fat),
                "--density", str(self.density),
                "--ccr", str(self.ccr),
                "--count", str(self.n)
            ])
        except subprocess.CalledProcessError as e:
            print(e.output)


        for file in graph_components_dir.iterdir():
            if file.name.startswith("daggen"):
                file.rename(graph_components_dir / f"{name}.dot")

class SubGraph(object):
    '''
    Usual graph (dag) kept as adjacency_list. Also has list of start and end nodes.
    '''

    def __init__(self):
        self.start_nodes: List[str] = []
        self.end_nodes: List[str] = []
        self.nodes: Dict[str, str] = {}
        self.adjacency_list: Dict[str, List[List[str]]] = {}

    @staticmethod
    def graph_from_dot(filename):
        self_ = SubGraph()
        with open(filename, "r") as file:
            file.readline()
            for line in file.readlines():
                if line[0] == '}':
                    break
                components = line.split()
                if len(components) < 2:
                    continue
                if components[1] == '->':
                    if components[0] not in self_.adjacency_list:
                        self_.adjacency_list[components[0]] = []
                    self_.adjacency_list[components[0]].append([components[2], ' '.join(components[3:])])
                else:
                    self_.nodes[components[0]] = ' '.join(components[1:])

        has_reference_list = set()
        for key, _ in self_.nodes.items():
            if key not in self_.adjacency_list or len(self_.adjacency_list[key]) == 0:
                self_.end_nodes.append(key)
            else:
                for node, _ in self_.adjacency_list[key]:
                    has_reference_list.add(node)
        for key, _ in self_.adjacency_list.items():
            if key not in has_reference_list:
                self_.start_nodes.append(key)

        return self_
    
    def add_suffix_to_names(self, suffix: str):
        '''
        to add same subgraph to main graph several times we need different names of tasks. 
        '''
        keys = list(self.nodes.keys()).copy()
        for key in keys:
            self.nodes[f"{key}_{suffix}"] = self.nodes.pop(key)

        keys = list(self.adjacency_list.keys()).copy()
        for key in keys:
            new_key = f"{key}_{suffix}"
            self.adjacency_list[new_key] = self.adjacency_list.pop(key)
            childs = self.adjacency_list[new_key]
            for i in range(len(childs)):
                self.adjacency_list[new_key][i][0] += '_' + suffix

        for i in range(len(self.end_nodes)):
            self.end_nodes[i] += '_' + suffix
        for i in range(len(self.start_nodes)):
            self.start_nodes[i] += '_' + suffix
    
    def eat_end(self):
        '''
        delete all ends tasks. Helpful if generated graph has only one end node.
        '''
        new_ends = []
        for key in self.end_nodes:
            self.nodes.pop(key)
        for source, childs in self.adjacency_list.items():
            list_to_del = []
            ind = 0
            for destination, _ in childs:
                if destination in self.end_nodes:
                    list_to_del.append(ind)
                ind += 1
            for ind in reversed(list_to_del):
                childs.pop(ind)
            self.adjacency_list[source] = childs
            if len(childs) == 0:
                new_ends.append(source)
        self.end_nodes = new_ends
    
    def eat_start(self):
        '''
        delete all start tasks. Helpful if generated graph has only one start node.
        '''
        for key in self.start_nodes:
            self.adjacency_list.pop(key)
            self.nodes.pop(key)
        has_referance_list = set()
        for _, childs in self.adjacency_list.items():
            for child, _ in childs:
                has_referance_list.add(child)
        self.start_nodes = []
        for key in self.nodes.keys():
            if key not in has_referance_list:
                self.start_nodes.append(key)

    def data_pass_to(self, node) -> float:
        '''
        sum all incoming data to node.
        '''
        result = 0
        for _, value in self.adjacency_list.items():
            for destin, attr in value:
                if node == destin:
                    # consider attr only in format of [size="1.0"];
                    result += float(attr.split('=')[1][1:-3])
        if result == 0:
            return random.uniform(1, 100)
        return result

    @staticmethod
    def get_single_node():
        init_node = SubGraph()
        init_node.nodes["init_node"] = "[size=\"1.0\"];"
        init_node.start_nodes = ["init_node"]
        init_node.end_nodes = ["init_node"]
        return init_node


class Graph:
    '''
    Main graph consist of subgraphs and edges between them.
    '''
    def __init__(self):
        self.subgraphs: List[SubGraph] = []
        self.external_adjacency_list: Dict[str, List[List[str]]] = {}

        # start with initial node
        self.subgraphs.append(SubGraph.get_single_node())
    
    def add_sub_direct(self, ind_a: int, subgraph: SubGraph, suffix: str):
        '''
        see below for logic. Here only insert new graph
        '''
        self.subgraphs.append(subgraph)
        self.subgraphs[-1].add_suffix_to_names(suffix)
        self.add_sub(ind_a, len(self.subgraphs) - 1)
    
    def mesh_sub_direct(self, ind_a: int, subgraph: SubGraph, suffix: str, density=0.6, jump=2, random_state=42):
        '''
        see below for logic. Here only insert new graph
        '''
        self.subgraphs.append(subgraph)
        self.subgraphs[-1].add_suffix_to_names(suffix)
        self.mesh_sub(ind_a, len(self.subgraphs) - 1, density, jump, random_state)
    
    def add_sub(self, ind_a: int, ind_b: int):
        '''
        combine two graphs strait forward. End node from first graph to start node from second one.
        considered case - subgraphs end and start with only one node. But if they are more, they're devided into pairs
        '''
        start_grap = self.subgraphs[ind_a]
        end_graph = self.subgraphs[ind_b]
        for source, destination in zip(start_grap.end_nodes, end_graph.start_nodes):
            if source not in self.external_adjacency_list:
                self.external_adjacency_list[source] = []
            data_pass = start_grap.data_pass_to(source)
            data_pass += random.uniform(-data_pass*0.3, data_pass*0.3)
            self.external_adjacency_list[source].append([destination, f'[size="{data_pass}"];'])
    
    def mesh_sub(self, ind_a: int, ind_b: int, density=0.6, jump=2, random_state=42):
        '''
        combine two graphs with more links.
        all end nodes from first graph connect to one of nodes of second graph in a jump distance from start nodes.
        also add edges between nodes in a distance less than jump. For every processing node number of predecessor is depend on density arg.
        weights are all data come to predecessor devided and summed with a noise

        In case if you want to immitate multuthreading program ie workers with same jobs, random_state can be passed the same for every thread.
        but there is a bug so edges may look similar but not the same (don't check)
        '''
        random.seed(random_state)

        start_grap = self.subgraphs[ind_a]
        end_graph = self.subgraphs[ind_b]
        level_but = end_graph.start_nodes
        cnt_of_prevs = len(start_grap.end_nodes)
        levels_up = {0: set(start_grap.end_nodes)}
        for i in range(jump):
            levels_up[i+1] = set()
            for source, childs in start_grap.adjacency_list.items():
                for child, _ in childs:
                    if child in levels_up[i]:
                        levels_up[i + 1].add(source)
                        break
        
        for _ in range(jump):
            for node in level_but:
                predecessors_cnt = min(1 + int(random.uniform(0.0, density * cnt_of_prevs)), cnt_of_prevs)
                all_options = levels_up[0]
                for i in levels_up.keys():
                    all_options |= levels_up[i]
                predecessors = random.choices(list(all_options), k=predecessors_cnt)
                for predecessor in predecessors:
                    if predecessor not in self.external_adjacency_list:
                        self.external_adjacency_list[predecessor] = []

                    data_pass = start_grap.data_pass_to(node) / (random.expovariate(0.5) + 2)
                    data_edge = data_pass + random.uniform(-data_pass*0.3, data_pass*0.3)
                    self.external_adjacency_list[predecessor].append([node, f'[size="{data_edge}"];'])
            cnt_of_prevs = len(level_but)
            new_level = set()
            for node in level_but:
                if node not in end_graph.adjacency_list:
                    continue
                new_level |= set(map(lambda x: x[0], end_graph.adjacency_list[node]))
            level_but = list(new_level)
            levels_up.pop(max(levels_up.keys()))
        
        levels_down = end_graph.start_nodes
        level_but = end_graph.start_nodes
        new_level = []
        for _ in range(0):
            for source, value in end_graph.adjacency_list.items():
                if source in level_but:
                    for v, _ in value:
                        if v not in levels_down:
                            levels_down.append(v)
                            new_level.append(v)
            level_but = new_level
            new_level = []
        
        for node in start_grap.end_nodes:
            successor = random.choice(levels_down)
            if node not in self.external_adjacency_list:
                self.external_adjacency_list[node] = []
            if successor in list(map(lambda x: x[0], self.external_adjacency_list[node])):
                continue
            data_pass = start_grap.data_pass_to(node) / (random.expovariate(0.5) + 2)
            data_edge = data_pass + random.uniform(-data_pass*0.3, data_pass*0.3)
            self.external_adjacency_list[node].append([successor, f'[size="{data_edge}"];'])

    
    def write_to_dot(self, filename: str):
        with open(filename, 'w') as file:
            file.write("digraph G {\n")
            for subgraph in self.subgraphs:
                for task_name, atrib in subgraph.nodes.items():
                    file.write(f"{task_name} {atrib}\n")
                for source, valie in subgraph.adjacency_list.items():
                    for destri, atttr in valie:
                        file.write(f"{source} -> {destri} {atttr}\n")
            for source, valie in self.external_adjacency_list.items():
                    for destri, atttr in valie:
                        file.write(f"{source} -> {destri} {atttr}\n")

            file.write('}\n')
