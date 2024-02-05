#! /bin/python3

import json, time, sys, signal, datetime, os

from dist_make import Cluster, Configuration, Experiment
from dist_make.logging import MetaLogger
from dist_make.benchmark import benchmark_main

ENGINE = "home-runner"

def benchmark(params: dict[str, any], collect_statistics: bool,
              result_printer, verbose=False):
    assert params["num-nodes"] >= params["num-machines"]

    exp_name = f"Experiment-{datetime.now()}"
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
    if params["num-nodes"] % params["num-machines"] > 0:
        params["num-nodes"] -= params["num-nodes"] % params["num-machines"]
        logger.print(f'Running {params["num-nodes"]} blockchain nodes instead')

    # generating machine and network config files
    num_nodes_per_machine = int(params["num-nodes"] / params["num-machines"])
    addrs = exp_machines.get_addrs()
    machine_config = {}
    for (idx, addr) in enumerate(addrs):
        base = idx << 12
        node_list = [base + id for id in range(num_nodes_per_machine)]
        machine_config[idx] = {
            "addr": f"{addr}:15500",
            "node_list": node_list,
        }
    with open("bench_machines.json", "w") as f:
        json.dump(machine_config, f)

    network_config = {
        "default_delay_millis": params["network-delay"],
        "default_bandwidth": params["network-bw"],
        "pipes": []
    }
    with open("bench_network.json", "w") as f:
        json.dump(network_config, f)

    for addr in addrs: 
        cluster.copy_to(addr, "bench_machines.json", f'{cluster.workdir}/bench_machines.json')
        cluster.copy_to(addr, "bench_network.json", f'{cluster.workdir}/bench_network.json')

    # start mailbox
    mailbox_task = exp_machines.run_background(config, "mailbox", args=[params["build-type"], "@POS"], engine=ENGINE, verbose=verbose)
    tasks.append(mailbox_task)

    time.sleep(5)

    for local_id in range(num_nodes_per_machine):
        run_args = [params["build-type"], "@POS", local_id, params["chain-type"], 
                    int(params["num-accounts"] / params["num-nodes"]), 
                    int(params["max-inflight-txns"] / params["num-nodes"]), 
                    int(params["frequency"] / params["num-nodes"]), params["config"]]
        node_task = exp_machines.run_background(config, "node", args=run_args, engine=ENGINE, verbose=verbose)
        tasks.append(node_task)

    # wait for timeout
    time.sleep(params["exp-time"])
    cleanup()

    # collect stats
    for machine in machine_config:
        for node in machine["node_list"]:
            stats_file = f"copycat_node_{node}.csv"
            cluster.copy_from(machine["addr"], f"/tmp/{stats_file}", f"./results/{exp_name}/{stats_file}")


if __name__ == "__main__":
    DEFAULT_PARAMS = {
        "build-type": "release",
        "num-nodes": 4,
        "num-machines": 1,
        "network-delay": 150, # in millis
        "network-bw": 25000000, # in B/s
        "chain-type": "bitcoin",
        "exp-time": 300, # in s
        "num-accounts": 10000,
        "max-inflight-txns": 100000,
        "frequency": 0,
        "config": "",
    }

    benchmark_main(DEFAULT_PARAMS, benchmark, cooldown_time=10)