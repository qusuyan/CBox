#! /bin/python3

import argparse, json

import math
import numpy as np

def gen_topo(nodes, degree, skewness):
    if degree >= len(nodes) - 1:
        return all_to_all(nodes)

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

    # edge_count = degree / 2 # since edges are undirected
    edge_count = degree
    edges = set()
    for (idx, node) in enumerate(nodes):
        neighbor_idx = {nidx + (nidx >= idx) for nidx in sampler(rng, edge_count, len(nodes) - 1)}
        neighbors = {nodes[nidx] for nidx in neighbor_idx}
        out_edges = {(node, neighbor) if neighbor > node else (neighbor, node) for neighbor in neighbors}
        edges = edges.union(out_edges)

    # find connected graphs and connect them together
    compartments = []
    unvisited_nodes = nodes.copy()
    while len(unvisited_nodes) > 0:
        frontier = [unvisited_nodes.pop(0)]
        connected = []
        while len(frontier) > 0:
            cur_node = frontier.pop(0)
            connected.append(cur_node)
            for (src, dst) in edges:
                if src == cur_node and dst in unvisited_nodes:
                    frontier.append(dst)
                    unvisited_nodes.remove(dst)
                if dst == cur_node and src in unvisited_nodes:
                    frontier.append(src)
                    unvisited_nodes.remove(src)
        compartments.append(connected)

    for idx1 in range(len(compartments)):
        for idx2 in range(idx1 + 1, len(compartments)):
            graph1 = compartments[idx1]
            graph2 = compartments[idx2]

            rand1_1 = math.floor(np.random.uniform() * len(graph1))
            node1_1 = graph1[rand1_1]
            rand1_2 = math.floor(np.random.uniform() * len(graph2))
            node1_2 = graph2[rand1_2]
            edges.add((node1_1, node1_2) if node1_2 > node1_1 else (node1_2, node1_1))

            rand2_1 = math.floor(np.random.uniform() * len(graph1) - 1) + (rand2_1 >= rand1_1)
            node2_1 = graph1[rand2_1]
            rand2_2 = math.floor(np.random.uniform() * len(graph2) - 1) + (rand2_2 >= rand1_2)
            node2_2 = graph2[rand2_2]
            edges.add((node2_1, node2_2) if node2_2 > node2_1 else (node2_2, node2_1))

    return list(edges)

def all_to_all(nodes):
    return [(a, b) for a in nodes for b in nodes if a != b]

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
