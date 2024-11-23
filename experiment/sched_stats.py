#! /bin/python3

import os, sys, re
from subprocess import check_output

import pandas as pd
import json

file_regex = "[\d]+\.[\d]+\.[\d]+\.[\d]+\.log"
data_regex = "\(([0-9]+)\).*sched_count: ([0-9.]+), mean_sched_dur: ([0-9.]+) s, poll_count: ([0-9.]+), mean_poll_dur: ([0-9.]+) s"

stages = ["txn_validation", "txn_dissemination", "pacemaker", "block_management", "block_dissemination", "decide", "commit"]

def parse_sched_stats(log_dir, line_ranges = {}):
    log_files = os.listdir(log_dir)

    metrics = []

    for file_name in log_files:
        if not re.match(file_regex, file_name):
            continue

        log_file = os.path.join(log_dir, file_name)

        (start_line, end_line) = line_ranges.get(log_file, (None, None))
        print_command = f"cat {log_file}"
        if start_line is None:
            pass
        else:
            print_command += f" | tail -n +{start_line}"

        if end_line is None:
            pass
        else:
            line_count = end_line if start_line is None else end_line - start_line
            print_command += f" | head -n {line_count}"

        for stage in stages:
            try:
                raw_metrics = check_output(f"{print_command} | grep '{stage}' | grep 'sched_count'", shell=True, encoding="utf-8")
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
    # df = df.sort_values(["node", "stage"])

    df = df[["sched_duration", "poll_duration", "sched_count", "poll_count"]]
    df["total_sched"] = df["sched_duration"] * df["sched_count"]
    df["total_poll"] = df["poll_duration"] * df["poll_count"]
    df = df.sum()
    df["sched_dur"] = df["total_sched"] / df["sched_count"] if df["sched_count"] > 0 else 0
    df["poll_dur"] = df["total_poll"] / df["poll_count"] if df["poll_count"] > 0 else 0

    return {"sched_dur_ms": df["sched_dur"] * 1000, "poll_dur_ms": df["poll_dur"] * 1000, # to ms
            "sched_count": int(df["sched_count"]), "poll_count": int(df["poll_count"])}


if __name__ == "__main__":
    log_dir = sys.argv[1]
    sched_stats = parse_sched_stats(log_dir)
    print(json.dumps(sched_stats, indent = 2))