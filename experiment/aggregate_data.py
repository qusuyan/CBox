#! /bin/python3

import os, json, math
import pandas as pd

exp_dir = "results/Aptos-Scale"
configs = ["correct-config", "num-nodes"]

# exp_dir = "results/Aptos-vs-Diem"
# configs = ["chain-type", "correct-config",  "num-nodes"]

# exp_dir = "results/Avax-Scale/"
# configs = ["correct-config", "crypto", "num-nodes"]

# exp_dir = "results/Avax-BlkLen/"
# configs = ["correct-config", "num-nodes"]

# exp_dir = "results/Avax-Faulty/"
# configs = ["correct-type", "num-faulty"]

# exp_dir = "results/Avax-Partition/"
# configs = ["correct-type", "correct-config"]

# exp_dir = "results/Avax-Pref-Cache"
# configs = ["correct-type", "num-faulty"]

# exp_dir = "results/Btc-Scale"
# configs = ["num-nodes"]

# exp_dir = "results/Btc-Few-Ecore"
# configs = ["num-nodes"]

# exp_dir = "results/Bitcoin-AsymmCompute/"
# configs = ["validator-config"]

# exp_dir = "results/ChainReplication-DummyECDSA/"
# configs = ["per-node-concurrency", "num-nodes"]

# exp_dir = "results/ChainReplication-NoEncrypt/"
# configs = ["per-node-concurrency", "num-nodes"]

# exp_dir = "results/Diem-Scale/"
# configs = ["script-runtime", "num-nodes"]

# exp_dir = "results/Crosschain-Scale"
# configs = ["chain-type", "num-nodes"]


metrics = ["avg_tput", "avg_cpu", "deliver_late_chance", "deliver_late_dur_ms", "sched_dur_ms", "ecore_util"]

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

    # if stats["deliver_late_chance"] > 0.0003:
    #     continue
    if math.isnan(stats["avg_tput"]):
        continue
    data = [config[c] for c in configs] + [stats[m] for m in metrics]
    records.append(data)

df = pd.DataFrame(records, columns = configs + metrics).fillna(0)
agg_df = df.groupby(configs).agg(['mean', 'std'])
print(agg_df)

output = os.path.join(exp_dir, "agg_results.csv")
agg_df.to_csv(output)
