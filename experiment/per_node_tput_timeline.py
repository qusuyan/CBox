#! /bin/python3

import os, sys, re
from datetime import datetime
from subprocess import check_output

import pandas as pd

meta_log = "META.log"
commit_regex = "Cluster[0-9+] (.*) INFO *copycat::stage::consensus::decide: \(([0-9]+)\) .* txns_sent: ([0-9.]+)"

log_dir = sys.argv[1]
log_files = os.listdir(log_dir)

records = []

for file_name in log_files:
    if file_name == meta_log:
        continue

    log_file = os.path.join(log_dir, file_name)

    try:
        commit_lines = check_output(f"cat {log_file} | grep 'copycat::stage::consensus::decide:'", shell=True, encoding="utf-8")
        commit_lines = commit_lines.split('\n')
    except:
        commit_lines = []

    commit_pattern = [re.search(commit_regex, line) for line in commit_lines]
    commit_pattern = filter(lambda x: x is not None, commit_pattern)
    commit_pattern = [pattern.groups() for pattern in commit_pattern]

    start_ts = None
    local_records = []
    for (ts, node, txns) in commit_pattern:
        datetime_ts = datetime.strptime(ts, "%Y-%m-%dT%H:%M:%S")
        local_records.append((datetime_ts, int(node), int(txns)))
        start_ts = min(start_ts, datetime_ts) if start_ts is not None else datetime_ts
    local_records = [((ts - start_ts).total_seconds(), node, txns) for (ts, node, txns) in local_records]
    records.extend(local_records)

df = pd.DataFrame(records, columns=["time", "node", "commits"])
df.to_csv("timeline.csv")
