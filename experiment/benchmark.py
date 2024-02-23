#! /bin/python3

import json, time, sys, signal, os, math
import numpy as np
from datetime import datetime

from dist_make import Cluster, Configuration, Experiment
from dist_make.logging import MetaLogger
from dist_make.benchmark import benchmark_main

ENGINE = "home-runner"

def benchmark(params: dict[str, any], collect_statistics: bool,
              result_printer, verbose=False):

    datetime_str = datetime.now().strftime("%Y%m%d%H%M%S")
    exp_name = f"Experiment-{datetime_str}"
    os.makedirs(f"./results/{exp_name}")

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

    if params["num-nodes"] < params["num-machines"]:
        logger.print("More machines than nodes, skipping...")
        return True

    # generating machine and network config files
    num_nodes_per_machine = int(params["num-nodes"] / params["num-machines"])
    num_nodes_remainder = params["num-nodes"] % params["num-machines"]
    
    nodes = []

    addrs = exp_machines.get_addrs()
    machine_config = {}
    for (idx, addr) in enumerate(addrs):
        base = idx << 12
        curr_machine_num_nodes = num_nodes_per_machine + (1 if num_nodes_remainder > idx else 0)
        node_list = [base + id for id in range(curr_machine_num_nodes)]
        machine_config[idx] = {
            "addr": f"{addr}:15500",
            "node_list": node_list,
        }
        nodes.append(node_list)
    with open("bench_machines.json", "w") as f:
        json.dump(machine_config, f)

    network_config = {
        "default_delay_millis": params["network-delay"],
        "default_bandwidth": params["network-bw"],
        "pipes": []
    }
    with open("bench_network.json", "w") as f:
        json.dump(network_config, f)

    # generate random network topology
    full_node_list = [node for tup in zip(*nodes) for node in tup]

    if params["topo-skewness"] <= 1:
        rng = lambda max: math.floor(np.random.uniform() * max)
    else:
        rng = lambda max: (np.random.zipf(params["topo-skewness"]) - 1) % max
    
    def sampler(rng, num_samples, max):
        num_samples_base = math.floor(num_samples)
        if np.random.uniform() < num_samples - num_samples_base:
            num_samples = num_samples_base + 1
        else:
            num_samples = num_samples_base

        samples = []
        for i in range(num_samples):
            sample = rng(max - i)
            for prev in samples:
                if sample >= prev:
                    sample += 1
                else:
                    break
            samples.append(sample)
            samples.sort()
        return samples

    edge_count = params["topo-degree"] / 2 # since edges are undirected
    edges = set()
    for (idx, node) in enumerate(full_node_list):
        neighbor_idx = {nidx + (nidx >= idx) for nidx in sampler(rng, edge_count, len(full_node_list) - 1)}
        neighbors = {full_node_list[nidx] for nidx in neighbor_idx}
        out_edges = {(node, neighbor) if neighbor > node else (neighbor, node) for neighbor in neighbors}
        edges = edges.union(out_edges)

    # print(len(edges), edges)
    # for node in full_node_list:
    #     neighbors = []
    #     for (src, dst) in edges:
    #         if src == node:
    #             neighbors.append(dst)
    #         if dst == node:
    #             neighbors.append(src)
    #     print(f"node {node} ({len(neighbors)}): {neighbors}")
        
    with open("bench_topo.json", "w") as f:
        json.dump(list(edges), f)

    for addr in addrs: 
        cluster.copy_to(addr, "bench_machines.json", f'{cluster.workdir}/bench_machines.json')
        cluster.copy_to(addr, "bench_network.json", f'{cluster.workdir}/bench_network.json')
        cluster.copy_to(addr, "bench_topo.json", f'{cluster.workdir}/bench_topo.json')

    # start mailbox
    mailbox_task = exp_machines.run_background(config, "mailbox", args=[params["build-type"], "@POS", params["mailbox-threads"],], engine=ENGINE, verbose=verbose)
    tasks.append(mailbox_task)

    time.sleep(5)

    for local_id in range(num_nodes_per_machine):
        run_args = [params["build-type"], "@POS", local_id, params["node-threads"], params["chain-type"], 
                    int(params["num-accounts"] / params["num-nodes"]), 
                    int(params["max-inflight-txns"] / params["num-nodes"]), 
                    int(params["frequency"] / params["num-nodes"]), params["config"]]
        node_task = exp_machines.run_background(config, "node", args=run_args, engine=ENGINE, verbose=verbose)
        tasks.append(node_task)

    # remaining nodes
    run_args = [params["build-type"], "@POS", num_nodes_per_machine, params["node-threads"], params["chain-type"], 
                    int(params["num-accounts"] / params["num-nodes"]), 
                    int(params["max-inflight-txns"] / params["num-nodes"]), 
                    int(params["frequency"] / params["num-nodes"]), params["config"]]
    if num_nodes_remainder > 0:
        remainder_task = exp_machines.run_background(config, "node", args=run_args, num_machines = num_nodes_remainder, engine=ENGINE, verbose=verbose)
        tasks.append(remainder_task)

    # wait for timeout
    time.sleep(params["exp-time"])
    cleanup()

    # collect stats
    for machine in machine_config.values():
        for node in machine["node_list"]:
            addr = machine["addr"].split(":")[0]
            stats_file = f"copycat_node_{node}.csv"
            cluster.copy_from(addr, f"/tmp/{stats_file}", f"./results/{exp_name}/{stats_file}")

    return True

if __name__ == "__main__":
    DEFAULT_PARAMS = {
        "build-type": "release",
        "num-nodes": 4,
        "num-machines": 1,
        "node-threads": 8,
        "mailbox-threads": 8,
        "network-delay": 150, # in millis
        "network-bw": 25000000, # in B/s
        "chain-type": "bitcoin",
        "exp-time": 300, # in s
        "num-accounts": 10000,
        "max-inflight-txns": 100000,
        "frequency": 0,
        "config": "",
        "topo-degree": 3,
        "topo-skewness": 0.0, # uniform
    }

    benchmark_main(DEFAULT_PARAMS, benchmark, cooldown_time=10)