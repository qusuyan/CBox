#! /bin/python3

import os, sys, re
from subprocess import check_output

import pandas as pd

file_regex = "[\d]+\.[\d]+\.[\d]+\.[\d]+\.log"
data_regex = "\(([0-9]+)\).*scheduled duration: ([0-9\.]+) ms, scheduled count: ([0-9]+)"

stages = ["txn_validation", "txn_dissemination", "pacemaker", "block_management", "block_dissemination", "decide", "commit"]

log_dir = sys.argv[1]
log_files = os.listdir(log_dir)

metrics = []

for file_name in log_files:
    if not re.match(file_regex, file_name):
        continue

    log_file = os.path.join(log_dir, file_name)
    for stage in stages:
        try:
            raw_metrics = check_output(f"cat {log_file} | grep '{stage}' | grep 'scheduled duration'", shell=True, encoding="utf-8")
        except:
            continue

        raw_lines = raw_metrics.split('\n')
        patterns = [re.search(data_regex, line) for line in raw_lines]
        patterns = filter(lambda x: x is not None, patterns)
        parsed_metrics = [pattern.groups() for pattern in patterns]
        for (node, sched_duration, sched_count) in parsed_metrics:
            metrics.append((node, stage, float(sched_duration), int(sched_count)))

df = pd.DataFrame(metrics, columns=["node", "stage", "sched_duration", "sched_count"])
df = df.sort_values(["node", "stage"])
# df = df[["sched_duration", "sched_count"]]
df = df[["stage", "sched_duration", "sched_count"]]
df = df.groupby(["stage"])
df = df.mean()
print(df.to_string())
