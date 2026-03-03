#! /bin/python3

import os, json, re
import pandas as pd

from get_log_lines import get_log_lines
from msg_delay import parse_msg_delay

meta_log = "META.log"

exp_dir = "results/Avax-BlkLen/"

metrics = ["avg_tput", "avg_cpu", "deliver_late_chance", "deliver_late_dur_ms", "sched_dur_ms"]

result_dirs = os.listdir(exp_dir)
records = []

for result_dir in result_dirs:

    result_path = os.path.join(exp_dir, result_dir)
    if not os.path.isdir(result_path):
        continue
    files = os.listdir(result_path)
    start_rt = None
    end_rt = None
    for file in files:
        if file == meta_log:
            continue
        csv_path = os.path.join(result_path, file)
        df = pd.read_csv(csv_path)
        first_commit = df["Avg Latency (s)"].ne(0).idxmax()
        last_record = (df["Available Memory"] > 3e8).idxmin()  # 300 MB
        last_record = last_record if last_record > 0 else df.shape[0]
        df = df.iloc[first_commit:last_record]
        avg_latency = df["Avg Latency (s)"].mean()
        df = df.loc[df["Runtime (s)"] > avg_latency + 10]
        df = df.iloc[1::] if df.shape[0] > 0 else df   # skip the first record
        start = int(df.iloc[0]["Runtime (s)"]) if df.shape[0] > 0 else None
        end = int(df.iloc[-1]["Runtime (s)"]) if df.shape[0] > 0 else None
        start_rt = start if start_rt is None else max(start_rt, start)
        end_rt = end if end_rt is None else min(end_rt, end)

    log_dir = os.path.join("logs/", result_dir)
    log_lines = get_log_lines(log_dir, start_rt, end_rt)
    msg_delay = parse_msg_delay(log_dir, line_ranges=log_lines)

    stats_file = os.path.join(exp_dir, result_dir, "stats.json")
    if not os.path.isfile(stats_file):
        continue
    with open(stats_file, "r") as f:
        stats = json.load(f) 
    stats["arrive_late_chance"] = msg_delay["arrive_late_chance"]
    stats["arrive_late_dur_ms"] = msg_delay["arrive_late_ms"]
    stats["deliver_late_chance"] = msg_delay["deliver_late_chance"]
    stats["deliver_late_dur_ms"] = msg_delay["deliver_late_ms"]
    
    with open(stats_file, "w") as f:
        json.dump(stats, f)

    # print(result_path, stats)