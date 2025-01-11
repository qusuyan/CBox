#! /bin/python3

import argparse, json

from random import sample

def parse_config(config: str):
    if config == "":
        return {}
    pairs = [pair.split("=") for pair in config.split('+')]
    return dict([(pair[0], eval(pair[1])) for pair in pairs])


def gen_node_config(nodes: list[int], node_type: str, config: dict, max_concurrency: int):
    return {
            "nodes": nodes,
            "config": {
                node_type: {
                    "config": config,
                }
            },
            "max_concurrency": max_concurrency,
        }

def gen_simple_node_config(nodes: list[int], config: dict, max_concurrency: int):
    return {
        "nodes": nodes,
        "config": config,
        "max_concurrency": max_concurrency,
    }

def gen_validator_configs(nodes: list[int], 
                          num_faulty: int, 
                          correct_type: str,
                          faulty_type: str, 
                          correct_config: str, 
                          faulty_config: str, 
                          max_concurrency: int = None):
    faulty_nodes = sample(nodes, num_faulty)
    correct_nodes = [node for node in nodes if node not in faulty_nodes]

    correct_node_config = parse_config(correct_config)
    faulty_node_config = parse_config(faulty_config)

    if correct_type == "" and faulty_type == "":
        return [gen_simple_node_config(nodes, correct_node_config, max_concurrency)]

    config = [gen_node_config(correct_nodes, correct_type, correct_node_config, max_concurrency)] if num_faulty == 0 else \
        [gen_node_config(correct_nodes, correct_type, correct_node_config, max_concurrency), 
         gen_node_config(faulty_nodes, faulty_type, faulty_node_config, max_concurrency)]

    return config

if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("num_nodes", type=int, help="number of local nodes")
    parser.add_argument("faulty_nodes", type=int, help="number of local nodes that are faulty")
    parser.add_argument("-t", "--correct_type", type=str, default="Correct", help="type of correct node")
    parser.add_argument("-f", "--faulty-type", type=str, default="", help="type of faulty node")
    parser.add_argument("-c", "--config", type=str, default="", help="config of correct nodes")
    parser.add_argument("-k", "--konfig", type=str, default="", help="config of faulty nodes")
    parser.add_argument("-m", "--concurrency", type=int, required=False, help="maximum allowed concurrency per node")
    parser.add_argument("-o", "--output", type=str, required=False, help="output location")
    args = parser.parse_args()

    if args.num_nodes < args.faulty_nodes:
        print("Error: more faulty nodes than total number of nodes")
    elif args.faulty_nodes > 0 and args.faulty_type == "":
        print("Error: faulty node type not provided")
    else:
        local_nodes = list(range(args.num_nodes))
        config = gen_validator_configs(local_nodes, 
                                       args.faulty_nodes, 
                                       args.correct_type,
                                       args.faulty_type, 
                                       args.config, 
                                       args.konfig, 
                                       args.concurrency)
        print(config)
        if args.output != None:
            with open(args.output, mode="w") as f:
                json.dump(config, f)