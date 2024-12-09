#! /bin/python3

import os
import json
import pandas as pd

# exp_dir = "results/Avax-Scale/"
# configs = ["crypto", "num-nodes"]

# exp_dir = "results/Avax-BlkLen/"
# configs = ["correct-config", "num-nodes"]

# exp_dir = "results/ChainReplication-DummyECDSA/"
# configs = ["per-node-concurrency", "num-nodes"]

# exp_dir = "results/ChainReplication-NoEncrypt/"
# configs = ["per-node-concurrency", "num-nodes"]

exp_dir = "results/Btc-Scale-all/"
configs = ["num-nodes"]

metrics = ["avg_tput", "avg_cpu", "stale_rate", "deliver_late_chance", "deliver_late_dur_ms", "sched_dur_ms"]

result_dirs = os.listdir(exp_dir)
records = []

for result_dir in result_dirs:
    config_file = os.path.join(exp_dir, result_dir, "config.json")
    if not os.path.isfile(config_file):
        continue
    with open(config_file, "r") as f:
        config = json.load(f)
    
    stats_file = os.path.join(exp_dir, result_dir, "stats.json")
    if not os.path.isfile(stats_file):
        continue
    with open(stats_file, "r") as f:
        stats = json.load(f)

    if stats["deliver_late_chance"] > 0.0005:
        continue
    data = [config[c] for c in configs] + [stats[m] for m in metrics]
    records.append(data)

df = pd.DataFrame(records, columns = configs + metrics).fillna(0)
agg_df = df.groupby(configs).agg(['mean', 'std'])
print(agg_df)

output = os.path.join(exp_dir, "agg_results.csv")
agg_df.to_csv(output)
