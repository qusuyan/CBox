#! /bin/python3
import os, json
import pandas as pd

exp_dir = "results/Btc-Scale-32/"
configs = ["num-nodes"]
metrics = ["avg_tput", "avg_cpu", "deliver_late_chance", "deliver_late_dur_ms", "sched_dur_ms", "stale_rate"]

records = []
result_dirs = os.listdir(exp_dir)
for result_dir in result_dirs:
    config_file = os.path.join(exp_dir, result_dir, "config.json")
    if not os.path.isfile(config_file):
        continue
    with open(config_file, "r") as f:
        config = json.load(f)
        if config["num-nodes"] == 140:
            stats_file = os.path.join(exp_dir, result_dir, "stats.json")
            if not os.path.isfile(stats_file):
                continue
            with open(stats_file, "r") as f:
                stats = json.load(f)

            data = [result_dir] + [config[c] for c in configs] + [stats[m] for m in metrics]
            records.append(data)

df = pd.DataFrame(records, columns=["exp_id"] + configs + metrics)
print(df)
