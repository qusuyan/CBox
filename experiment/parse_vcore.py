#! /bin/python3

import os, sys, re
from subprocess import check_output

import pandas as pd
import json

file_regex = "[\d]+\.[\d]+\.[\d]+\.[\d]+\.log"
vcore_regex = "copycat::node: \(([0-9]+)\) vCore utilization: ([0-9.]+)"

def parse_vcore_util(log_dir, line_ranges = {}):
    log_files = os.listdir(log_dir)

    vcore_utils = []

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

        try:
            vcore_util = check_output(f"{print_command} | grep 'vCore utilization'", shell=True, encoding="utf-8")
            vcore_util = vcore_util.split('\n')
        except:
            vcore_util = []

        vcore_util_pattern = [re.search(vcore_regex, line) for line in vcore_util]
        vcore_util_pattern = filter(lambda x: x is not None, vcore_util_pattern)
        vcore_util_pattern = [pattern.groups() for pattern in vcore_util_pattern]

        for (node, util) in vcore_util_pattern:
            util_float = float(util)
            vcore_utils.append((node, util_float))
    
    per_node_avg_util = {}
    for (node, util) in vcore_utils:
        (total, count) = per_node_avg_util.get(node, (0, 0))
        total += util
        count += 1
        per_node_avg_util[node] = (total, count)

    per_node_avg_util = {node: total / count for node, (total, count) in per_node_avg_util.items()}
    avg = sum(per_node_avg_util.values()) / len(per_node_avg_util)
    return {"avg": avg, "per_node": per_node_avg_util}

if __name__ == "__main__":
    log_dir = sys.argv[1]
    vcore_utils = parse_vcore_util(log_dir)
    print(json.dumps(vcore_utils, indent = 2))
