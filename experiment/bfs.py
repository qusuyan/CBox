def bfs(graph, src, dst):
    frontier = [(src, 0)]
    visited = {}
    while len(frontier) > 0:
        (cur, dist) = frontier.pop(0)
        if cur == dst:
            return dist
        visited[cur] = dist
        children = [(child, dist+1) for child in graph[cur] if child not in visited]
        frontier.extend(children)
    return None
