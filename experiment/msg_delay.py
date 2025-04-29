#! /bin/python3

import os, sys, re
from subprocess import check_output

import pandas as pd
import json

meta_log = "META.log"
deliver_regex = "Worker (\d+): (\d+) message get delivered later than it should \(([0-9\.]+)(.*)\). "
arrive_regex = "Worker (\d+): (\d+) message arrived later than it should \(([0-9\.]+)(.*)\). "
msgs_sent_recv_regex = "copycat::peers::peers: \((\d+)\) In the last minute: (\d+) msgs sent and (\d+) msgs recved"


def parse_msg_delay(log_dir, line_ranges = {}):
    log_files = os.listdir(log_dir)

    arrive_late_metrics = []
    deliver_late_metrics = []
    msgs_sent = {}
    msgs_recv = {}

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
        if file_name == meta_log:
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

        # number of msgs sent and recved in total
        try:
            msgs_sent_recv = check_output(f"{print_command} | grep 'copycat::peers:'", shell=True, encoding="utf-8")
            msgs_sent_recv = msgs_sent_recv.split('\n')
        except:
            msgs_sent_recv = []

        msgs_sent_recv_pattern = [re.search(msgs_sent_recv_regex, line) for line in msgs_sent_recv]
        msgs_sent_recv_pattern = filter(lambda x: x is not None, msgs_sent_recv_pattern)
        msgs_sent_recv_parsed = [pattern.groups() for pattern in msgs_sent_recv_pattern]
        for (node, send, recv) in msgs_sent_recv_parsed:
            msgs_sent[node] = msgs_sent.get(node, 0) + int(send)
            msgs_recv[node] = msgs_recv.get(node, 0) + int(recv)

        try:
            arrive_late = check_output(f"{print_command} | grep 'message arrived later than it should'", shell=True, encoding="utf-8")
            arrive_late = arrive_late.split('\n')
        except:
            arrive_late = []

        arrive_late_pattern = [re.search(arrive_regex, line) for line in arrive_late]
        arrive_late_pattern = filter(lambda x: x is not None, arrive_late_pattern)
        arrive_late_parsed = [pattern.groups() for pattern in arrive_late_pattern]
        for (worker, count, late, unit) in arrive_late_parsed:
            late_secs = to_secs(float(late), unit)
            arrive_late_metrics.append((file_name, worker, int(count), late_secs))
        
        try:
            deliver_late = check_output(f"{print_command} | grep 'message get delivered later than it should'", shell=True, encoding="utf-8")
            deliver_late = deliver_late.split('\n')
        except:
            deliver_late = []

        deliver_late_pattern = [re.search(deliver_regex, line) for line in deliver_late]
        deliver_late_pattern = filter(lambda x: x is not None, deliver_late_pattern)
        deliver_late_parsed = [pattern.groups() for pattern in deliver_late_pattern]
        for (worker, count, late, unit) in deliver_late_parsed:
            late_secs = to_secs(float(late), unit)
            deliver_late_metrics.append((file_name, worker, int(count), late_secs))

    arrive_late_df = pd.DataFrame(arrive_late_metrics, columns=["log", "worker", "late_count", "late_secs"])
    deliver_late_df = pd.DataFrame(deliver_late_metrics, columns=["log", "worker", "late_count", "late_secs"])

    arrive_late = arrive_late_df[["late_count", "late_secs"]].sum()
    arrive_late_count = arrive_late["late_count"]
    arrive_late_secs = arrive_late["late_secs"]
    arrive_late_avg = arrive_late_secs / arrive_late_count * 1000

    deliver_late = deliver_late_df[["late_count", "late_secs"]].sum()
    deliver_late_count = deliver_late["late_count"]
    deliver_late_secs = deliver_late["late_secs"]
    deliver_late_avg = deliver_late_secs / deliver_late_count * 1000

    msgs_sent_total = sum(msgs_sent.values())
    msgs_recv_total = sum(msgs_recv.values())

    arrive_late_chance = arrive_late_count / msgs_recv_total if msgs_recv_total > 0 else 0
    deliver_late_chance = deliver_late_count / msgs_recv_total if msgs_recv_total > 0 else 0

    return {"msgs_sent": msgs_sent_total, "msgs_recv": msgs_recv_total, 
            "arrive_late_count": arrive_late_count, "arrive_late_chance": arrive_late_chance, "arrive_late_ms": arrive_late_avg,
            "deliver_late_count": deliver_late_count, "deliver_late_chance": deliver_late_chance, "deliver_late_ms": deliver_late_avg}


if __name__ == "__main__":
    log_dir = sys.argv[1]
    msg_delay = parse_msg_delay(log_dir)
    print(json.dumps(msg_delay, indent = 2))
