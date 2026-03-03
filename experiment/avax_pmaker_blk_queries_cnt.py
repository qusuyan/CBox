#! /bin/python3

import os, json
import pandas as pd

from avax_utils import parse_blks_queried_answered

exp_dir = "results/Avax-Pmaker"
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
    pref_checks = parse_blks_queried_answered(exp_log)
    pref_checks = pref_checks[["time", "blks-queried", "blks-answered"]]
    pref_checks = pref_checks.groupby("time").mean()
    pref_checks["correct-config"] = config["correct-config"]
    dfs.append(pref_checks)

df = pd.concat(dfs)
agg_df = df.groupby(["correct-config", "time"]).mean().sort_values(["correct-config", "time"])
agg_df.to_csv("avax_blk_queries.csv")
print(agg_df)
