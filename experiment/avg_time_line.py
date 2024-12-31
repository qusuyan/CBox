#! /bin/python3

import os, re, json

import pandas as pd

# exp_times = list(range(0, 310, 30))
exp_time = 310

csv_file_regex = "copycat_cluster_[\d]+\.csv"

exp_dir = "results/Avax-Pmaker"
result_dirs = os.listdir(exp_dir)

df = pd.DataFrame()

for result_dir in result_dirs:
    result_path = os.path.join(exp_dir, result_dir)
    if not os.path.isdir(result_path):
        continue
    config_path = os.path.join(result_path, "config.json")
    with open(config_path, "r") as f:
        config = json.load(f)
    if config["num-nodes"] != 20:
        continue

    result_files = os.listdir(result_path)
    for file in result_files:
        if not re.match(csv_file_regex, file):
            continue

        file_path = os.path.join(result_path, file)
        raw_result = pd.read_csv(file_path)
        raw_result = raw_result[["Runtime (s)", "Throughput (txn/s)"]]
        raw_result["config"] = config["correct-config"]
        df = pd.concat([df, raw_result])

df = df[df["Runtime (s)"] <= exp_time]
df = df[["Runtime (s)", "Throughput (txn/s)", "config"]]
df = df.groupby(["config", "Runtime (s)"]).mean().reset_index(names=["config", "Runtime (s)"])
df = df.pivot(columns="config", index="Runtime (s)")
df.columns = [column[1] for column in df.columns]
df = df.reset_index("Runtime (s)")
print(df)

out_file = os.path.join(exp_dir, "tput_timeline.csv")
df.to_csv(out_file)
