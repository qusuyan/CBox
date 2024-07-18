#! /bin/python3

import os, sys, re
from subprocess import check_output

import pandas as pd
import json

file_regex = "[\d]+\.[\d]+\.[\d]+\.[\d]+\.log"
data_regex = "\(([0-9]+)\).*sched_count: ([0-9.]+), mean_sched_dur: ([0-9.]+) s, poll_count: ([0-9.]+), mean_poll_dur: ([0-9.]+) s"

stages = ["txn_validation", "txn_dissemination", "pacemaker", "block_management", "block_dissemination", "decide", "commit"]

def parse_sched_stats(log_dir):
    log_files = os.listdir(log_dir)

    metrics = []

    for file_name in log_files:
        if not re.match(file_regex, file_name):
            continue

        log_file = os.path.join(log_dir, file_name)
        for stage in stages:
            try:
                raw_metrics = check_output(f"cat {log_file} | grep '{stage}' | grep 'sched_count'", shell=True, encoding="utf-8")
            except:
                print(f"skipping stage {stage} for {log_file}...")
                continue

            raw_lines = raw_metrics.split('\n')
            patterns = [re.search(data_regex, line) for line in raw_lines]
            patterns = filter(lambda x: x is not None, patterns)
            parsed_metrics = [pattern.groups() for pattern in patterns]

            # skip first minute
            nodes_seen = set()
            for (node, sched_count, sched_duration, poll_count, poll_duration) in parsed_metrics:
                if node in nodes_seen:
                    metrics.append((node, stage, int(sched_count), float(sched_duration), int(poll_count), float(poll_duration)))
                else:
                    nodes_seen.add(node)

    df = pd.DataFrame(metrics, columns=["node", "stage", "sched_count", "sched_duration", "poll_count", "poll_duration"])
    df = df.sort_values(["node", "stage"])
    # df = df[["sched_duration", "sched_count"]]
    df_dur = df[["sched_duration", "poll_duration"]]
    # df_dur = df_dur.groupby(["stage"])
    df_dur = df_dur.mean()

    df_count = df[["sched_count", "poll_count"]]
    # df_count = df_count.groupby(["stage"])
    df_count = df_count.sum()

    return {"sched_dur_ms": df_dur["sched_duration"] * 1000, "poll_dur_ms": df_dur["poll_duration"] * 1000, # to ms
            "sched_count": int(df_count["sched_count"]), "poll_count": int(df_count["poll_count"])}


if __name__ == "__main__":
    log_dir = sys.argv[1]
    sched_stats = parse_sched_stats(log_dir)
    print(json.dumps(sched_stats, indent = 2))