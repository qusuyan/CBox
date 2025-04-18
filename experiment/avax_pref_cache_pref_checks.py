#! /bin/python3

import os, json
import pandas as pd

from parse_avax_pref_checks import parse_avax_pref_checks

exp_dir = "results/Avax-Pref-Cache"
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
    pref_checks = parse_avax_pref_checks(exp_log)
    pref_checks = pref_checks[["pref-check-calls", "pref-checks-count"]]
    pref_checks["type"] = config["correct-type"]
    pref_checks["num-faulty"] = config["num-faulty"]
    pref_checks["checks-per-call"] = pref_checks["pref-checks-count"] / pref_checks["pref-check-calls"]
    dfs.append(pref_checks)

df = pd.concat(dfs)
agg_df = df.groupby(["type", "num-faulty"]).mean().sort_values(["type", "num-faulty"])
agg_df.to_csv("avax_pref_cache.csv")
print(agg_df)
