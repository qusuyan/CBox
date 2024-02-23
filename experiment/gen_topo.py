#! /bin/python3

import argparse, json

import math
import numpy as np

def gen_topo(nodes, degree, skewness):
    if skewness <= 1:
        rng = lambda max: math.floor(np.random.uniform() * max)
    else:
        rng = lambda max: (np.random.zipf(skewness) - 1) % max
    
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

    edge_count = degree / 2 # since edges are undirected
    edges = set()
    for (idx, node) in enumerate(nodes):
        neighbor_idx = {nidx + (nidx >= idx) for nidx in sampler(rng, edge_count, len(nodes) - 1)}
        neighbors = {nodes[nidx] for nidx in neighbor_idx}
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

    return list(edges)

if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("num_nodes", type=int, help="number of local nodes")
    parser.add_argument("-d", "--degree", type=int, default=3, help="average number of edges per node")
    parser.add_argument("-s", "--skewness", type=float, default=0.0, help="edge skewness, zipf if > 1 and uniform otherwise")
    parser.add_argument("-o", "--output", type=str, default="test_topo.json", help="output JSON file")
    args = parser.parse_args()

    nodes = list(range(args.num_nodes))
    edges = gen_topo(nodes, args.degree, args.skewness)
    with open(args.output, "w") as f:
        json.dump(edges, f)
