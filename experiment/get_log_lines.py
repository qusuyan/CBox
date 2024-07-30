#! /bin/python3

import os, sys, re
from subprocess import check_output

file_regex = "[\d]+\.[\d]+\.[\d]+\.[\d]+\.log"
tput_ts_regex = "(\d\d\d\d-\d\d-\d\dT\d\d:\d\d:\d\d).*Runtime: ([\d]+) s"

def get_log_lines(log_dir, start_rt = None, end_rt = None): # TODO: different files may have different start and end?
    log_files = os.listdir(log_dir)

    line_ranges = {}

    for file_name in log_files:
        if not re.match(file_regex, file_name):
            continue

        log_file = os.path.join(log_dir, file_name)

        try:
            tput_lines = check_output(f"cat {log_file} | grep 'Throughput'", shell=True, encoding="utf-8")
            tput_lines = tput_lines.split('\n')
        except:
            tput_lines = []

        tput_ts_pattern = [re.search(tput_ts_regex, line) for line in tput_lines]
        tput_ts_pattern = filter(lambda x: x is not None, tput_ts_pattern)
        ts_parsed = [pattern.groups() for pattern in tput_ts_pattern]

        start_ts = None
        if start_rt is None:
            pass
        else:
            for (ts, runtime) in ts_parsed:
                if int(runtime) < start_rt:
                    start_ts = ts
                else:
                    break

        start_line = None
        if start_ts is None:
            pass
        else:
            last_match = check_output(f"cat {log_file} | grep -n '{start_ts}' | tail -1", shell=True, encoding="utf-8")
            last_match_idx = last_match.find(":")
            start_line = int(last_match[:last_match_idx]) + 1

        end_ts = None
        if end_rt is None:
            pass
        else:
            for (ts, runtime) in ts_parsed:
                if int(runtime) <= end_rt:
                    end_ts = ts
                else:
                    break

        end_line = None
        if end_ts is None:
            pass
        else:
            last_match = check_output(f"cat {log_file} | grep -n '{end_ts}' | tail -1", shell=True, encoding="utf-8")
            last_match_idx = last_match.find(":")
            end_line = int(last_match[:last_match_idx]) + 1

        line_ranges[log_file] = (start_line, end_line)

    return line_ranges

if __name__ == "__main__":
    log_dir = sys.argv[1]

    if len(sys.argv) > 2:
        start_rt = int(sys.argv[2])
    else:
        start_rt = None

    if len(sys.argv) > 3:
        end_rt = int(sys.argv[3])
    else:
        end_rt = None

    line_ranges = get_log_lines(log_dir, start_rt, end_rt)
    print(line_ranges)
