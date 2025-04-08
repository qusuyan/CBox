#! /bin/python3

import json, time, sys, signal, os
from datetime import datetime

import pandas as pd

from dist_make import Cluster, Configuration, Experiment
from dist_make.logging import MetaLogger
from dist_make.benchmark import benchmark_main

from gen_topo import gen_topo
from msg_delay import parse_msg_delay
from sched_stats import parse_sched_stats
from get_log_lines import get_log_lines
from gen_validator_config import gen_validator_configs

ENGINE = "home-runner"
SETUP_TIME = 10

def benchmark(params: dict[str, any], collect_statistics: bool,
              result_printer, verbose=False):

    tasks = []
    def cleanup():
        ''' Cleanup function that shuts down all running tasks '''
        for task in tasks:
            if task and task.is_alive():
                task.terminate()
                task.join()

    def quit(signum, frame):
        logger.print("got exit signal, exiting...")
        cleanup()
        sys.exit(1)
        
    for sig in ('TERM', 'HUP', 'INT'):
        signal.signal(getattr(signal, 'SIG'+sig), quit)

    cluster = Cluster()
    config = Configuration(cluster, "copycat")
    machines = cluster.create_slice("machines")

    machines.run(config, "cleanup", verbose=verbose)

    exp_machines = machines.create_subslice(params["num-machines"])
    exp = Experiment(cluster, config, collect_statistics=collect_statistics)

    logger = MetaLogger(exp.log_dir)
    logger.print(f'Running benchmark on with parameters: {params}')

    exp_name = f'{params["exp-prefix"]}experiment-{exp.uid}'
    os.makedirs(f"./results/{exp_name}")
    with open(f"./results/{exp_name}/config.json", "w") as f:
        json.dump(params, f, indent=2)

    if params["num-nodes"] < params["num-machines"]:
        logger.print("More machines than nodes, skipping...")
        return True

    # generating machine and network config files
    num_nodes_per_machine = int(params["num-nodes"] / params["num-machines"])
    num_nodes_remainder = params["num-nodes"] % params["num-machines"]
    
    nodes = []

    internal_addrs = exp_machines.get_internal_addrs()
    public_addrs = exp_machines.get_addrs()
    addrs = list(enumerate(zip(public_addrs, internal_addrs)))
    print(addrs)
    if params["machine-config"] == "":
        machine_config = {}
        for (idx, (_, addr)) in addrs:
            base = idx << 12
            curr_machine_num_nodes = num_nodes_per_machine + (1 if num_nodes_remainder > idx else 0)
            node_list = [base + id for id in range(curr_machine_num_nodes)]
            machine_config[idx] = {
                "addr": f"{addr}:15500",
                "node_list": node_list,
            }
            nodes.append(node_list)
        machine_config_file = "bench_machines.json"
        with open(machine_config_file, "w") as f:
            json.dump(machine_config, f)
    else:
        machine_config_file = params["machine-config"]
        with open(machine_config_file, "r") as f:
            machine_config = json.load(f)
            for machine in machine_config.values():
                nodes.append(machine["node_list"])

    if params["network-config"] == "":
        network_config = {
            "default_delay_millis": params["network-delay"],
            "default_jitter_millis": params["network-jitter"],
            "default_bandwidth": params["network-bw"],
            "default_nic_bandwidth": params["nic-bw"],
            "pipes": [],
            "nics": []
        }
        network_config_file = "bench_network.json"
        with open(network_config_file, "w") as f:
            json.dump(network_config, f)
    else:
        network_config_file = params["network-config"]

    full_node_list = [node for tup in zip(*nodes) for node in tup]

    if params["topo-config"] == "":
        # generate random network topology
        degree = len(full_node_list) - 1 if params["topo-degree"] == 0 else params["topo-degree"]
        edges = gen_topo(full_node_list, degree, params["topo-skewness"])
        topo_config_file = "bench_topo.json"
        with open(topo_config_file, "w") as f:
            json.dump(edges, f)
    else:
        topo_config_file = params["topo-config"]

    if params["validator-config"] == "":
        # generate validator configs
        validator_configs = gen_validator_configs(full_node_list, params["num-faulty"], params["correct-type"], params["faulty-type"], 
                                                params["correct-config"], params["faulty-config"], params["per-node-concurrency"])
        validator_config_file = "bench_validators.json"
        with open(validator_config_file, "w") as f:
            json.dump(validator_configs, f)
    else:
        validator_config_file = params["validator-config"]

    for (_, (addr, _)) in addrs: 
        cluster.copy_to(addr, machine_config_file, f'{cluster.workdir}/bench_machines.json')
        cluster.copy_to(addr, network_config_file, f'{cluster.workdir}/bench_network.json')
        cluster.copy_to(addr, topo_config_file, f'{cluster.workdir}/bench_topo.json')
        cluster.copy_to(addr, validator_config_file, f'{cluster.workdir}/bench_validators.json')

    # compute 
    num_flow_gen = params["num-machines"]
    num_accounts = int(params["num-accounts"] / num_flow_gen)
    max_inflight = int(params["max-inflight-txns"] / num_flow_gen)
    frequency = int(params["frequency"] / num_flow_gen)

    clients_per_machine = int(params["num-clients"] / params["num-machines"])
    clients_remainder = params["num-clients"] % params["num-machines"]

    if params["single-process-cluster"]:
        txn_crypto = params["txn-crypto"] if params["txn-crypto"] is not None else params["crypto"]
        p2p_crypto = params["p2p-crypto"] if params["p2p-crypto"] is not None else params["crypto"]

        run_args = [params["build-type"], "@POS", params["cluster-threads"], params["mailbox-workers"], params["chain-type"], 
                    txn_crypto, p2p_crypto, params["threshold-crypto"], params["conn-multiply"], SETUP_TIME, 
                    clients_per_machine, clients_remainder, num_accounts, max_inflight, frequency, params["conflict_rate"], 
                    params["txn-span"], params["disable-txn-dissem"], params["mailbox-threshold"], params["script-size"]]
        cluster_task = exp_machines.run_background(config, "cluster", args=run_args, engine=ENGINE, verbose=verbose, log_dir=exp.log_dir)
        tasks.append(cluster_task)
    else: 
        # start mailbox
        mailbox_task = exp_machines.run_background(config, "mailbox", args=[params["build-type"], "@POS", params["mailbox-threads"], 
                                                                            params["mailbox-workers"], params["conn-multiply"]], 
                                                   engine=ENGINE, verbose=verbose)
        tasks.append(mailbox_task)

        time.sleep(5)

        for local_id in range(num_nodes_per_machine):
            run_args = [params["build-type"], "@POS", local_id, params["node-threads"], params["mailbox-workers"], params["chain-type"], 
                        params["crypto"], num_accounts, max_inflight, frequency, params["disable-txn-dissem"]]
            node_task = exp_machines.run_background(config, "node", args=run_args, engine=ENGINE, verbose=verbose)
            tasks.append(node_task)

        # remaining nodes
        run_args = [params["build-type"], "@POS", num_nodes_per_machine, params["node-threads"], params["mailbox-workers"], 
                    params["chain-type"], params["crypto"], num_accounts, max_inflight, frequency, params["disable-txn-dissem"]]
        if num_nodes_remainder > 0:
            remainder_task = exp_machines.run_background(config, "node", args=run_args, num_machines = num_nodes_remainder, 
                                                         engine=ENGINE, verbose=verbose)
            tasks.append(remainder_task)

    # wait for timeout
    time.sleep(params["exp-time"])
    cleanup()

    # collect stats
    files = []
    # for (machine_id, machine) in machine_config.items():
    for (machine_id, (addr, _)) in addrs:
        # addr = machine["addr"].split(":")[0]
        if params["single-process-cluster"]:
            stats_file = f"copycat_cluster_{machine_id}.csv"
            lat_file = f"copycat_cluster_{machine_id}_lat.csv"
            files.append((addr, (stats_file, lat_file)))
        else:
            for node in machine_id["node_list"]:
                stats_file = f"copycat_node_{node}.csv"
                lat_file = f"copycat_node_{node}_lat.csv"
                files.append((addr, (stats_file, lat_file)))
    print(files)

    stats = { "peak_tput": 0.0 }
    cumulative = {"tput": 0.0, "cpu_util": 0.0}
    start_rt = None
    end_rt = None
    for (addr, (stats_file, lat_file)) in files:
        cluster.copy_from(addr, f"/tmp/{stats_file}", f"./results/{exp_name}/{stats_file}")
        cluster.copy_from(addr, f"/tmp/{lat_file}", f"./results/{exp_name}/{lat_file}")
        df = pd.read_csv(f"./results/{exp_name}/{stats_file}", dtype=float)
        lat = pd.read_csv(f"./results/{exp_name}/{lat_file}")
        first_commit = df["Throughput (txn/s)"].ne(0).idxmax()
        last_record = (df["Available Memory"] > 3e8).idxmin()  # 300 MB
        last_record = last_record if last_record > 0 else df.shape[0]
        df = df.iloc[first_commit:last_record]
        avg_latency = lat["Latency (s)"].mean()
        df = df.loc[df["Runtime (s)"] > avg_latency + SETUP_TIME]
        df = df.iloc[1::] if df.shape[0] > 0 else df   # skip the first record
        start = int(df.iloc[0]["Runtime (s)"]) if df.shape[0] > 0 else None
        end = int(df.iloc[-1]["Runtime (s)"]) if df.shape[0] > 0 else None
        start_rt = start if start_rt is None else start_rt if start is None else max(start_rt, start)
        end_rt = end if end_rt is None else end_rt if end is None else min(end_rt, end)
        stats["peak_tput"] = max(stats["peak_tput"], df.loc[:, 'Throughput (txn/s)'].max())
        cumulative["tput"] += df['Throughput (txn/s)'].mean()
        cumulative["cpu_util"] += df['Avg CPU Usage'].mean()

    stats["avg_tput"] = cumulative["tput"] / len(files)
    stats["avg_cpu"] = cumulative["cpu_util"] / len(files)
    
    # TODO: parse only logs corresponding to specific range of experiment time
    log_dir = exp.log_dir
    log_line_ranges = get_log_lines(log_dir, start_rt, end_rt)
    msg_delay = parse_msg_delay(log_dir, line_ranges=log_line_ranges)
    stats["arrive_late_chance"] = msg_delay["arrive_late_chance"]
    stats["arrive_late_dur_ms"] = msg_delay["arrive_late_ms"]
    stats["deliver_late_chance"] = msg_delay["deliver_late_chance"]
    stats["deliver_late_dur_ms"] = msg_delay["deliver_late_ms"]

    sched_stats = parse_sched_stats(log_dir, line_ranges=log_line_ranges)
    stats["wakeup_count"] = sched_stats["sched_count"]
    stats["sched_dur_ms"] = sched_stats["sched_dur_ms"]
    stats["poll_dur_ms"] = sched_stats["poll_dur_ms"]

    logger.print(
f'''
msg delay stats:
{json.dumps(msg_delay, indent=2)}

sched stats:
{json.dumps(sched_stats, indent=2)}
''')

    with open(f"./results/{exp_name}/stats.json", "w") as f:
        json.dump(stats, f, indent=2)

    return True

if __name__ == "__main__":
    DEFAULT_PARAMS = {
        "build-type": "release",
        "num-nodes": 4,
        "num-machines": 1,
        "num-clients": 10,
        "node-threads": 8,
        "mailbox-threads": 8,
        "cluster-threads": 40,
        "network-delay": 30, # in millis
        "network-jitter": 10, # in millis
        "network-bw": 12500000, # 100 Mbps = 12.5 MB/s
        "nic-bw": 12500000,
        "chain-type": "bitcoin",
        "exp-time": 300, # in s
        "num-accounts": 10000,
        "max-inflight-txns": 100000,
        "frequency": 0,
        "conflict_rate": 0,
        "txn-span": 1,
        "num-faulty": 0,
        "correct-type": "",
        "faulty-type": "",
        "correct-config": "",
        "faulty-config": "",
        "topo-degree": 0,
        "topo-skewness": 0.0, # uniform
        "disable-txn-dissem": False,
        "crypto": "dummy",
        "txn-crypto": None,
        "p2p-crypto": None,
        "threshold-crypto": "dummy",
        "single-process-cluster": True,
        "conn-multiply": 1,
        "per-node-concurrency": 2,
        "mailbox-workers": 40,
        "machine-config": "",
        "network-config": "",
        "topo-config": "",
        "validator-config": "",
        "exp-prefix": "",
        "script-size": "",
        "mailbox-threshold": 10,
    }

    benchmark_main(DEFAULT_PARAMS, benchmark, cooldown_time=10)