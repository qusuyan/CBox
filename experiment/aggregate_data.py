#! /bin/python3

import os
import json
import pandas as pd

# exp_dir = "results/Avax-Scale/"
# configs = ["crypto", "num-nodes"]
exp_dir = "results/Avax-BlkLen/"
configs = ["correct-config", "num-nodes"]

metrics = ["avg_tput", "avg_cpu", "deliver_late_chance", "deliver_late_dur_ms", "sched_dur_ms"]

result_dirs = os.listdir(exp_dir)
records = []

for result_dir in result_dirs:
    config_file = os.path.join(exp_dir, result_dir, "config.json")
    with open(config_file, "r") as f:
        config = json.load(f)
    
    stats_file = os.path.join(exp_dir, result_dir, "stats.json")
    with open(stats_file, "r") as f:
        stats = json.load(f)

    data = [config[c] for c in configs] + [stats[m] for m in metrics]
    records.append(data)

df = pd.DataFrame(records, columns = configs + metrics)
agg_df = df.groupby(configs).mean()
print(agg_df)

output = os.path.join(exp_dir, "agg_results.csv")
agg_df.to_csv(output)
