#! /bin/python

import os, json
import pandas as pd

from avax_utils import parse_query_success

exp_dir = "results/Avax-Faulty"
exp_names = os.listdir(exp_dir)

dfs = []
for exp_name in exp_names:
    exp_path = os.path.join(exp_dir, exp_name)
    if not os.path.isdir(exp_path):
        continue

    exp_config_path = os.path.join(exp_path, "config.json")
    with open(exp_config_path, "r") as f:
        config = json.load(f)

    exp_log = os.path.join("logs", exp_name)
    query_count = parse_query_success(exp_log)
    query_count = query_count[["queries-succeeded", "queries-failed"]]
    query_count["correct-type"] = config["correct-type"]
    query_count["num-faulty"] = config["num-faulty"]
    query_count["success-rate"] = query_count["queries-succeeded"] / (query_count["queries-succeeded"] + query_count["queries-failed"])
    dfs.append(query_count)

df = pd.concat(dfs)
agg_df = df.groupby(["correct-type", "num-faulty"]).mean().sort_values(["correct-type", "num-faulty"])
agg_df.to_csv("avax_query_success.csv")
print(agg_df)