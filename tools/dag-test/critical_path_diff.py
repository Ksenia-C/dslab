import json
import argparse

parser = argparse.ArgumentParser(description="Synthetic DAG generator")
parser.add_argument("--file_worse", type=str, help="path to gen file")
parser.add_argument("--file_best", type=str, help="path to gen file")
args = parser.parse_args()

worse_descr = {}
with open(args.file_worse, 'r') as file:
    worse_descr = json.load(file)

best_descr = {}
with open(args.file_best, 'r') as file:
    best_descr = json.load(file)

worse_graph = worse_descr['graph']

tasks = {}
ad = {}
rad = {}

for task in worse_graph['tasks']:
    tasks[task['name']] = {}

for edge in worse_graph['data_items']:
    a, b = edge['name'].split(' -> ')
    ad[a] = ad.get(a, []) + [b]
    rad[b] = rad.get(b, []) + [a]


last_scheduled_task = ""
for event in worse_descr['events']:
    if event['type'] == 'task_started':
        tasks[event['task_name']]['worse_time_start'] = event['time']
    
    if event['type'] == 'task_completed':
        tasks[event['task_name']]['worse_time_end'] = event['time']
        last_scheduled_task = event['task_name']

for event in best_descr['events']:
    if event['type'] == 'task_started':
        tasks[event['task_name']]['best_time_start'] = event['time']
    
    if event['type'] == 'task_completed':
        tasks[event['task_name']]['best_time_end'] = event['time']


diff_tree = [last_scheduled_task]
def dfs(u: str):
    for v in rad[u]:
        if tasks[v]['best_time_end'] < tasks[v]['worse_time_end']:
            diff_tree.append(v)
            dfs(v)



dfs(last_scheduled_task)

for i in range(len(worse_descr['events']) - 1, -1, -1):
    event = worse_descr['events'][i]
    if 'task_name' in event and event['task_name'] not in diff_tree:
        worse_descr['events'].pop(i)

for i in range(len(best_descr['events']) - 1, -1, -1):
    event = best_descr['events'][i]
    if 'task_name' in event and event['task_name'] not in diff_tree:
        best_descr['events'].pop(i)


with open("worse.json", "w") as file:
    json.dump(worse_descr, file)

with open("best.json", 'w') as file:
    json.dump(best_descr, file)
