#! /bin/python3

import os, json, re
import pandas as pd

lat_file_regex = "copycat_cluster_[\d+]_lat.csv"

# exp_dir = "results/Avax-Latency/"
exp_dir = "results/HotStuff-Perf"

records = []
result_dirs = os.listdir(exp_dir)
for result_dir in result_dirs:
    result_path = os.path.join(exp_dir, result_dir)
    config_file = os.path.join(result_path, "config.json")
    if not os.path.isfile(config_file):
        continue
    with open(config_file, "r") as f:
        config = json.load(f)
    stats_file = os.path.join(result_path, "stats.json")
    if not os.path.isfile(stats_file):
        continue
    with open(stats_file, "r") as f:
        stats = json.load(f)
    
    # parse latency
    lat_dfs = []
    contents = os.listdir(result_path)
    for file in contents:
        if not re.match(lat_file_regex, file):
            continue
        lat_csv_path = os.path.join(result_path, file)
        lat_dfs.append(pd.read_csv(lat_csv_path, header=[0]))
    lat_df = pd.concat(lat_dfs)
    avg_lat = lat_df['Latency (s)'].mean()
    p99_lat = lat_df['Latency (s)'].quantile(0.99)
    records.append((config["max-inflight-txns"], stats["avg_tput"], avg_lat, p99_lat))

df = pd.DataFrame(records, columns=["txns", "tput", "avg_lat", "p99_lat"])
df = df.groupby("txns").mean()
df = df.sort_values("txns")
print(df)
