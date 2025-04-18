#! /bin/python3

import os, sys, re
from datetime import datetime
from subprocess import check_output

import pandas as pd

meta_log = "META.log"
strong_pref_regex = "Cluster[\d]+ (.*) INFO .*\(([\d]+)\) In the last minute: is_strongly_preferred_calls: ([\d]+), is_preferred_checks: ([\d]+)"
working_set_regex = "Cluster[\d]+ (.*) INFO .*\(([\d]+)\) working set size: txn_dag: ([\d]+)"

def parse_avax_pref_checks(log_dir):
    log_files = os.listdir(log_dir)
    metrics = []

    for file_name in log_files:
        if file_name == meta_log:
            continue
        
        log_file = os.path.join(log_dir, file_name)

        try:
            raw_metrics = check_output(f"cat {log_file} | grep 'is_strongly_preferred_calls'", shell=True, encoding="utf-8")
        except:
            print(f"skipping file {log_file}...")
            continue

        raw_lines = raw_metrics.split('\n')
        patterns = [re.search(strong_pref_regex, line) for line in raw_lines]
        patterns = filter(lambda x: x is not None, patterns)
        parsed_metrics = [pattern.groups() for pattern in patterns]

        init = None
        for (ts, node, pref_check_calls, pref_checks) in parsed_metrics:
            datetime_ts = datetime.strptime(ts, "%Y-%m-%dT%H:%M:%S")
            if init is None:
                init = datetime_ts
            time = (datetime_ts - init).total_seconds()
            metrics.append((time, int(node), int(pref_check_calls), int(pref_checks)))

    return pd.DataFrame(metrics, columns=["time", "node", "pref-check-calls", "pref-checks-count"])


def parse_working_set(log_dir):
    log_files = os.listdir(log_dir)
    metrics = []

    for file_name in log_files:
        if file_name == meta_log:
            continue
        
        log_file = os.path.join(log_dir, file_name)

        try:
            raw_metrics = check_output(f"cat {log_file} | grep 'working set size'", shell=True, encoding="utf-8")
        except:
            print(f"skipping file {log_file}...")
            continue

        raw_lines = raw_metrics.split('\n')
        patterns = [re.search(working_set_regex, line) for line in raw_lines]
        patterns = filter(lambda x: x is not None, patterns)
        parsed_metrics = [pattern.groups() for pattern in patterns]

        init = None
        for (ts, node, working_set_size) in parsed_metrics:
            datetime_ts = datetime.strptime(ts, "%Y-%m-%dT%H:%M:%S")
            if init is None:
                init = datetime_ts
            time = (datetime_ts - init).total_seconds()
            metrics.append((time, int(node), int(working_set_size)))

    return pd.DataFrame(metrics, columns=["time", "node", "working-set-size"])


if __name__ == "__main__":
    log_dir = sys.argv[1]
    
    pref_stats = parse_avax_pref_checks(log_dir)
    print(pref_stats)

    working_set_stats = parse_working_set(log_dir)
    print(working_set_stats)
