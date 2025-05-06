#! /bin/python3

import os, json, re
import pandas as pd
import statistics
from subprocess import check_output

meta_log = "META.log"
blk_build_val_regex = "INFO: Cluster\d+.*INFO *copycat::stage::consensus::block_management::diem::basic: \(\d+\) avg_blk_building_time: (\d+.?\d+), avg_blk_validation_time: (\d+.?\d+)"
blk_delivery_regex  = "INFO: Cluster\d+.*INFO *copycat::stage::consensus::block_management::diem::basic: \(\d+\) blk_delivery_times: (\[.*\])"
vote_delivery_regex = "INFO: Cluster\d+.*INFO *copycat::stage::consensus::decide::diem::basic: \(\d+\) vote_delivery_times: (\[.*\])"

def parse_diem_latency_breakdown(log_dir):
    log_files = os.listdir(log_dir)

    blk_build_times = []
    blk_deliver_times = []
    blk_validate_times = []
    vote_deliver_times = []

    for file_name in log_files:
        if file_name == meta_log:
            continue
        
        log_file = os.path.join(log_dir, file_name)
        
        try:
            raw_metrics = check_output(f"cat {log_file} | grep 'diem'", shell=True, encoding="utf-8")
        except:
            print(f"skipping file {log_file}...")
            continue

        raw_lines = raw_metrics.split('\n')
        for line in raw_lines:
            # check for block building and validation
            pattern = re.search(blk_build_val_regex, line)
            if pattern is not None:
                (build_time, validate_time) = pattern.groups()
                blk_build_times.append(float(build_time))
                blk_validate_times.append(float(validate_time))
                continue

            # check for block sending
            pattern = re.search(blk_delivery_regex, line)
            if pattern is not None:
                (blk_send_times, ) = pattern.groups()
                blk_deliver_times += eval(blk_send_times)
                continue

            # check for voting times
            pattern = re.search(vote_delivery_regex, line)
            if pattern is not None:
                (vote_send_times, ) = pattern.groups()
                vote_deliver_times += eval(vote_send_times)

    avg_build_time = statistics.mean(blk_build_times)
    avg_validate_time = statistics.mean(blk_validate_times)
    quorum_blk_send_time = statistics.quantiles(blk_deliver_times, n=3)[1]
    quorum_vote_send_time = statistics.quantiles(vote_deliver_times, n=3)[1]

    return {
        "build_time": avg_build_time,
        "propose_time": quorum_blk_send_time,
        "validate_time": avg_validate_time,
        "vote_time": quorum_vote_send_time,
    }

exp_dir = "results/Diem-Scale"
exp_names = os.listdir(exp_dir)

records = []
for exp_name in exp_names:
    print(exp_name)
    result_dir = os.path.join(exp_dir, exp_name)
    if not os.path.isdir(result_dir):
        continue

    log_dir = os.path.join("logs", exp_name)
    config_path = os.path.join(result_dir, "config.json")
    with open(config_path, "r") as f:
        config = json.load(f)
    latency_breakdown = parse_diem_latency_breakdown(log_dir)
    records.append((config["num-nodes"], config["script-runtime"], latency_breakdown["build_time"], 
                    latency_breakdown["propose_time"], latency_breakdown["validate_time"], 
                    latency_breakdown["vote_time"]))

latencies = pd.DataFrame(records, columns=["num-nodes", "script-runtime", "build_time", "propose_time", "validate_time", "vote_time"])
latencies = latencies.groupby(["script-runtime", "num-nodes"]).mean()
latencies.to_csv(os.path.join(exp_dir, "diem_latency_breakdown.csv"))
print(latencies)