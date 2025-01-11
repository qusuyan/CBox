#! /bin/python3

import os, re
import pandas as pd

lat_file_regex = "copycat_cluster_[\d+]_lat.csv"

exp_dir = "results/Avax-Latency/experiment-250103-013140/"

# parse latency
lat_dfs = []
contents = os.listdir(exp_dir)
for file in contents:
    if not re.match(lat_file_regex, file):
        continue
    lat_csv_path = os.path.join(exp_dir, file)
    lat_dfs.append(pd.read_csv(lat_csv_path, header=[0]))
lat_df = pd.concat(lat_dfs)

# print(list(zip(np.histogram(lat_df["Latency (s)"].map(math.log), bins=100))))

out_file = os.path.join(exp_dir, "latency.csv")
lat_df.to_csv(out_file)
