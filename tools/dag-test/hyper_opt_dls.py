from hyperopt import hp, fmin, tpe, Trials, space_eval
from functools import partial
import json
import subprocess
import argparse

PATH_TO_DAGS_PARENT_FOLDER = ''

argParser = argparse.ArgumentParser()
argParser.add_argument("-g", "--graphtype", help="other, tree_incr, tree_decr")

args = argParser.parse_args()

graph_type = args.graphtype

search_space = {
    'static_level': hp.uniform(label='static_level', low=0, high=4),
    'start_time': hp.uniform(label='start_time', low=-4, high=0),
    'delta': hp.uniform(label='delta',low=0, high=4),
    'ds': hp.uniform(label='ds',low=0, high=4),
}


def objective(params):
    with open("./dls_art_coefs", "w") as wf:
        json.dump(params, wf)

    subprocess.run(f"cargo run -- --graphs-folder {PATH_TO_DAGS_PARENT_FOLDER}/{graph_type}/inss/ --mode test --result-directory alibaba_meta/{graph_type}/trace", shell=True)
    result = 0
    with open("./hyper_oprt_res", "r") as rf:
        result = float(rf.readline())
    return -result


# update all schedulers results
with open("./dls_art_coefs", "w") as wf:
        json.dump({"static_level": 1.0, 'start_time':-1.0, 'delta':1.0, 'ds':1.0}, wf)

subprocess.run(f"cargo run -- --graphs-folder {PATH_TO_DAGS_PARENT_FOLDER}/{graph_type}/inss/ --mode test --result-directory alibaba_meta/{graph_type} --trace-folder alibaba_meta/{graph_type}/trace --is-first-run true", shell=True)

trials = Trials()

# search for result

best_ = fmin(
    fn=partial(objective),
    space=search_space,
    algo=tpe.suggest,
    max_evals=400,
    trials=trials,
    show_progressbar=True
)

with open("results", "a") as file:
     print(graph_type, space_eval(search_space, best_), -objective(best_), file=file)
