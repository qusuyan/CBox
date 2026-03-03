#! /bin/python3

import os, re, json
from subprocess import check_output
from parse_ecore import parse_ecore_util

import pandas as pd

meta_log = "META.log"
reward_regex = r"bitcoin::basic: \(([\d+])\) miner blk count: (.*)"

def parse_reward(log_dir):
    log_files = os.listdir(log_dir)

    rewards = {}

    for file_name in log_files:
        if file_name == meta_log:
            continue

        log_file = os.path.join(log_dir, file_name)

        try:
            reward_lines = check_output(f"cat {log_file} | grep 'miner blk count'", shell=True, encoding="utf-8")
            reward_lines = reward_lines.split('\n')
        except:
            reward_lines = []

        rewards_pattern = [re.search(reward_regex, line) for line in reward_lines]
        rewards_pattern = filter(lambda x: x is not None, rewards_pattern)
        rewards_parsed = [pattern.groups() for pattern in rewards_pattern]
        rewards_parsed.reverse()

        for (node, blk_cnt_list) in rewards_parsed:
            if rewards.get(node) is not None: # only keep the last line
                continue

            blk_cnt_dict = eval(blk_cnt_list)
            reward_dict = {node: blk_cnt / sum(blk_cnt_dict.values()) for node, blk_cnt in blk_cnt_dict.items()}
            rewards[node] = reward_dict

    df = pd.DataFrame(rewards.values())
    means = df.mean().to_dict()
    return means

if __name__ == "__main__":
    exp_dir = "results/Bitcoin-AsymmCompute-Tmp/"
    result_dirs = os.listdir(exp_dir)

    records = []
    for result_dir in result_dirs:
        config_file = os.path.join(exp_dir, result_dir, "config.json")
        if not os.path.isfile(config_file):
            continue
        with open(config_file, "r") as f:
            config = json.load(f)

        log_dir = os.path.join("logs/", result_dir)
        rewards = parse_reward(log_dir)
        validator_config = re.search(r"setups/bitcoin_asym_compute/validators(\d+).json", config["validator-config"])
        validator_0_ecores = int(validator_config.groups()[0])
        ecores = parse_ecore_util(log_dir)
        validator_0_ecore_util = ecores["per_node"]["0"]
        validator_other_ecore_util = (sum(ecores["per_node"].values()) - validator_0_ecore_util) / (len(ecores["per_node"]) - 1)
        records.append((validator_0_ecores, rewards.get(0, 0), validator_0_ecore_util, validator_other_ecore_util))

    df = pd.DataFrame(records, columns=["validator_0_ecores", "rewards", "validator_0_ecore_util", "validator_ecore_util"])
    df = df.groupby("validator_0_ecores").mean().reset_index(names="validator_0_ecores")
    df["comp_power"] = df["validator_0_ecores"] / (df["validator_0_ecores"] + 39)
    df["hash_power"] = (df["validator_0_ecores"] - df["validator_0_ecore_util"]) / (df["validator_0_ecores"]- df["validator_0_ecore_util"] + 39*(1-df["validator_ecore_util"]))
    df.to_csv(os.path.join(exp_dir, "rewards.csv"))
    print(df)

