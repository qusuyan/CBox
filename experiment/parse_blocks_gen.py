#! /bin/python3

import os, sys, re, json
from subprocess import check_output

file_regex = "[\d]+\.[\d]+\.[\d]+\.[\d]+\.log"
blk_gen_regex = "copycat::stage::consensus::block_management: \(([\d]+)\) In the last minute: self_blks_sent: ([\d]+)"


def parse_total_blks_gen(log_dir):
    log_files = os.listdir(log_dir)
    blk_total = 0

    for file_name in log_files:
        if not re.match(file_regex, file_name):
            continue

        log_file = os.path.join(log_dir, file_name)

        try:
            raw_metrics = check_output(f"cat {log_file} | grep 'self_blks_sent'", shell=True, encoding="utf-8")
        except:
            print(f"skipping file {log_file}...")
            continue

        raw_lines = raw_metrics.split('\n')
        patterns = [re.search(blk_gen_regex, line) for line in raw_lines]
        patterns = filter(lambda x: x is not None, patterns)
        parsed_metrics = [pattern.groups() for pattern in patterns]

        for (_, blk_cnt) in parsed_metrics:
            blk_total += int(blk_cnt)

    return blk_total

if __name__ == "__main__":
    log_dir = sys.argv[1]
    sched_stats = parse_total_blks_gen(log_dir)
    print(json.dumps(sched_stats, indent = 2))
