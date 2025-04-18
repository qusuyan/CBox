#! /bin/python3

import os, re, json
from datetime import datetime
from subprocess import check_output

import pandas as pd

meta_log = "META.log"
ts_regex = "Cluster\d+ (.*) INFO"
txns_sent_regex = "Cluster\d+ (.*) INFO *copycat_cluster: In the last minute: txns_sent: (\d+),"
txn_validation_out_regex = "Cluster\d+ (.*) INFO *copycat::stage::txn_validation: \((\d+)\) In the last minute: txn_batches_validated: \d+, txns_validated: (\d+)" 
block_management_out_regex = "Cluster\d+ (.*) INFO *copycat::stage::consensus::block_management: \((\d+)\) In the last minute: self_blks_sent: \d+, self_txns_sent: (\d+)"
block_dissem_out_regex = "Cluster\d+ (.*) INFO *copycat::stage::consensus::block_dissemination: \((\d+)\) In the last minute: blks_recv: \d+, txns_recv: \d+, blks_sent: \d+, txns_sent: (\d+)"
decide_out_regex = "Cluster\d+ (.*) INFO *copycat::stage::consensus::decide: \((\d+)\) In the last minute: blks_recv: \d+, txns_recv: \d+, blks_sent: \d+, txns_sent: (\d+)"
commit_out_regex = "Cluster\d+ (.*) INFO *copycat::stage::commit: \((\d+)\) In the last minute: txns_recved: \d+, txns_committed: (\d+)"

exp_dir = "results/Aptos-vs-Diem-old"
record_time = 70

exp_names = os.listdir(exp_dir)
txn_counts = []

for exp_name in exp_names:
    exp_path = os.path.join(exp_dir, exp_name)
    if not os.path.isdir(exp_path):
        continue

    config_file = os.path.join(exp_path, "config.json")
    with open(config_file, "r") as f:
        config = json.load(f)

    log_dir = os.path.join("logs", exp_name)

    total_txns_sent = 0
    total_txn_valiation = {}
    total_block_management = {}
    total_block_dissemination = {}
    total_decide = {}
    total_commit = {}
    
    for log_file_name in os.listdir(log_dir):
        if log_file_name == meta_log:
            continue

        log_file = os.path.join(log_dir, log_file_name)
    
        init = None
        with open(log_file, "r") as f:
            for line in f:
                if init is None:
                    ts_pattern = re.search(ts_regex, line)
                    if ts_pattern is None:
                        continue
                    (init_ts,) = ts_pattern.groups()
                    init = datetime.strptime(init_ts, "%Y-%m-%dT%H:%M:%S")
                
                # parse txns_sent
                pattern = re.search(txns_sent_regex, line)
                if pattern is not None:
                    (ts, txns_sent) = pattern.groups()
                    datetime_ts = datetime.strptime(ts, "%Y-%m-%dT%H:%M:%S")
                    time = (datetime_ts - init).total_seconds()
                    if time < record_time:
                        total_txns_sent += int(txns_sent)
                    else:
                        break   # reach ts limit
                    continue
                
                # parse txn_validation_sent
                pattern = re.search(txn_validation_out_regex, line)
                if pattern is not None:
                    (ts, node, txns_sent) = pattern.groups()
                    datetime_ts = datetime.strptime(ts, "%Y-%m-%dT%H:%M:%S")
                    time = (datetime_ts - init).total_seconds()
                    node = int(node)
                    txns_sent = int(txns_sent)
                    if time < record_time:
                        total_txn_valiation[node] = total_txn_valiation.get(node, 0) + txns_sent
                    else:
                        break   # reach ts limit
                    continue

                # parse block_management_sent
                pattern = re.search(block_management_out_regex, line)
                if pattern is not None:
                    (ts, node, txns_sent) = pattern.groups()
                    datetime_ts = datetime.strptime(ts, "%Y-%m-%dT%H:%M:%S")
                    time = (datetime_ts - init).total_seconds()
                    node = int(node)
                    txns_sent = int(txns_sent)
                    if time < record_time:
                        total_block_management[node] = total_block_management.get(node, 0) + txns_sent
                    else:
                        break   # reach ts limit
                    continue

                # parse block_dissemination_sent
                pattern = re.search(block_dissem_out_regex, line)
                if pattern is not None:
                    (ts, node, txns_sent) = pattern.groups()
                    datetime_ts = datetime.strptime(ts, "%Y-%m-%dT%H:%M:%S")
                    time = (datetime_ts - init).total_seconds()
                    node = int(node)
                    txns_sent = int(txns_sent)
                    if time < record_time:
                        total_block_dissemination[node] = total_block_dissemination.get(node, 0) + txns_sent
                    else:
                        break   # reach ts limit
                    continue

                # parse decide_sent
                pattern = re.search(decide_out_regex, line)
                if pattern is not None:
                    (ts, node, txns_sent) = pattern.groups()
                    datetime_ts = datetime.strptime(ts, "%Y-%m-%dT%H:%M:%S")
                    time = (datetime_ts - init).total_seconds()
                    node = int(node)
                    txns_sent = int(txns_sent)
                    if time < record_time:
                        total_decide[node] = total_decide.get(node, 0) + txns_sent
                    else:
                        break   # reach ts limit
                    continue

                # parse commit_sent
                pattern = re.search(commit_out_regex, line)
                if pattern is not None:
                    (ts, node, txns_sent) = pattern.groups()
                    datetime_ts = datetime.strptime(ts, "%Y-%m-%dT%H:%M:%S")
                    time = (datetime_ts - init).total_seconds()
                    node = int(node)
                    txns_sent = int(txns_sent)
                    if time < record_time:
                        total_commit[node] = total_commit.get(node, 0) + txns_sent
                    else:
                        break   # reach ts limit
                    continue

    # since commit stage is never the bottleneck
    txns = total_txns_sent
    txn_validation = max(total_txn_valiation.values())
    block_management = sum(total_block_management.values())
    block_dissemination = max(total_block_dissemination.values())
    decide = max(total_decide.values())
    commit = max(total_commit.values())
    if config["chain-type"] == "aptos":
        # since validators can propose repetitive txns
        scale_factor = commit / decide
        block_management *= scale_factor
        block_dissemination *= scale_factor
        decide *= scale_factor
    txn_counts.append((config["chain-type"], config["correct-config"], config["num-nodes"], txns, txn_validation, block_management, block_dissemination, decide, commit))

df = pd.DataFrame(txn_counts, columns=["chain-type", "correct-config", "num-nodes", "txns-sent", "txn-validation", "block-management", "block-dissemination", "decide", "commit"])
df = df.groupby(["chain-type", "correct-config", "num-nodes"]).mean().sort_values(["chain-type", "correct-config", "num-nodes"])
df.to_csv("aptos_diem_bottleneck.csv")
print(df)