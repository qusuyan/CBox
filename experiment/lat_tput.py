#! /bin/python3

import os, json, re
import pandas as pd

lat_file_regex = "copycat_cluster_[\d+]_lat.csv"

# exp_dir = "results/Avax-Latency/"
exp_dir = "results/Narwhal-HS/"

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
    p50_lat = lat_df['Latency (s)'].quantile(0.5)
    p90_lat = lat_df['Latency (s)'].quantile(0.9)
    p99_lat = lat_df['Latency (s)'].quantile(0.99)
    records.append((config["num-nodes"], config["max-inflight-txns"], stats["avg_tput"], avg_lat, p50_lat, p90_lat, p99_lat))

df = pd.DataFrame(records, columns=["num_nodes", "txns", "tput", "avg_lat", "p50_lat", "p90_lat", "p99_lat"])
df = df.groupby(["num_nodes", "txns"]).mean()
df = df.sort_values(["num_nodes", "txns"])
print(df)
