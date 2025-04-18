#! /bin/python3

import os, re
import json

import pandas as pd
from parse_blocks_gen import parse_total_blks_gen
from parse_vcore import parse_vcore_util
from get_log_lines import get_log_lines

commit_depth = 6
warmup_time = 0
txns_per_block = 1796

exp_dir = "results/Btc-Scale/"
csv_file_regex = "copycat_cluster_[\d]+\.csv"

result_dirs = os.listdir(exp_dir)

for result_dir in result_dirs:
    dir_path = os.path.join(exp_dir, result_dir)
    if not os.path.isdir(dir_path):
        continue

    files = os.listdir(dir_path)
    actual_chain_length = 0
    actual_ts = 0
    for file_name in files:
        if not re.match(csv_file_regex, file_name):
            continue

        file_path = os.path.join(dir_path, file_name)
        df = pd.read_csv(file_path)
        ts = df["Runtime (s)"].iloc[-1]
        chain_length = df["Chain Length"].iloc[-1]
        # txns = chain_length * txns_per_block
        # tputs.append(txns / (ts + warmup_time))
        # chain_lengths.append(chain_length)
        if chain_length > actual_chain_length or actual_ts == 0:
            actual_chain_length = chain_length
            actual_ts = ts

    log_dir = os.path.join("logs/", result_dir)
    log_lines = get_log_lines(log_dir, 0, actual_ts)
    blocks_generated = parse_total_blks_gen(log_dir)
    ecore_util = parse_vcore_util(log_dir, log_lines)

    config_file = os.path.join(dir_path, "config.json")
    with open(config_file, "r") as f:
        config = json.load(f)

    stats_file = os.path.join(dir_path, "stats.json")
    with open(stats_file, "r") as f:
        orig_json = json.load(f)
        
    # chain length is off by 1
    orig_json["avg_tput"] = ((actual_chain_length + commit_depth) * txns_per_block) / (actual_ts + warmup_time) if actual_chain_length > 0 else float('nan')
    orig_json["stale_rate"] = 1 - ((actual_chain_length + commit_depth) / blocks_generated) if actual_chain_length > 0 else float('nan')
    orig_json["ecore_util"] = ecore_util["avg"]
    with open(stats_file, "w") as f:
        json.dump(orig_json, f, indent=2)