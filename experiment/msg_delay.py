#! /bin/python3

import os, sys, re
from subprocess import check_output

import pandas as pd

file_regex = "[\d]+\.[\d]+\.[\d]+\.[\d]+\.log"
deliver_regex = "mailbox::msg_queue: message get delivered later than it should: SystemTimeError\(([0-9\.]+)(.*)\)"
arrive_regex = "Messenger ([\d]+)->([\d]+): message arrived later than it should: SystemTimeError\(([0-9\.]+)(.*)\)"

log_dir = sys.argv[1]
log_files = os.listdir(log_dir)

arrive_late_metrics = []
deliver_late_metrics = []

def to_secs(quantity, unit):
    if unit == "s":
        return quantity
    elif unit == "ms":
        return quantity / 1e3
    elif unit == "µs":
        return quantity / 1e6
    elif unit == "ns":
        return quantity / 1e9
    else:
        raise Exception(f"unit {unit} not recognized")

for file_name in log_files:
    if not re.match(file_regex, file_name):
        continue

    log_file = os.path.join(log_dir, file_name)
    try:
        arrive_late = check_output(f"cat {log_file} | grep 'message arrived later than it should'", shell=True, encoding="utf-8")
        arrive_late = arrive_late.split('\n')
    except:
        arrive_late = []

    arrive_late_pattern = [re.search(arrive_regex, line) for line in arrive_late]
    arrive_late_pattern = filter(lambda x: x is not None, arrive_late_pattern)
    arrive_late_parsed = [pattern.groups() for pattern in arrive_late_pattern]
    for (src, dst, late, unit) in arrive_late_parsed:
        late_secs = to_secs(float(late), unit)
        arrive_late_metrics.append((file_name, src, dst, late_secs))
    
    try:
        deliver_late = check_output(f"cat {log_file} | grep 'message get delivered later than it should'", shell=True, encoding="utf-8")
        deliver_late = deliver_late.split('\n')
    except:
        deliver_late = []

    deliver_late_pattern = [re.search(deliver_regex, line) for line in deliver_late]
    deliver_late_pattern = filter(lambda x: x is not None, deliver_late_pattern)
    deliver_late_parsed = [pattern.groups() for pattern in deliver_late_pattern]
    for (late, unit) in deliver_late_parsed:
        late_secs = to_secs(float(late), unit)
        deliver_late_metrics.append((file_name, late_secs))

arrive_late_df = pd.DataFrame(arrive_late_metrics, columns=["log", "src", "dst", "late_secs"])
deliver_late_df = pd.DataFrame(deliver_late_metrics, columns=["log", "late_secs"])

arrive_late_secs = arrive_late_df[["late_secs"]]
arrive_late_count = arrive_late_secs.shape[0]
arrive_late_avg = arrive_late_secs.mean()
print("arrive late count: ", arrive_late_count)
print("arrive late mean: ", arrive_late_avg)

deliver_late_secs = deliver_late_df[["late_secs"]]
deliver_late_count = deliver_late_secs.shape[0]
deliver_late_avg = deliver_late_secs.mean()
print("deliver late count: ", deliver_late_count)
print("deliver late mean: ", deliver_late_avg)

    