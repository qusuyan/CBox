import pandas as pd
import sys
from collections import Counter

files = sys.argv[1:]
queue_len = []

for file in files:
    with open(file, 'r', encoding="utf-8") as content_file:
        content = content_file.read()
    parsed = content.split('\n')
    parsed = list(map(lambda x: int(x), filter(lambda x: x is not None and x != "", parsed)))
    full = [(file, time, queue_len) for (time, queue_len) in enumerate(parsed)]
    queue_len.extend(full)
    max_index, max_value = max(enumerate(parsed), key=lambda x: x[1])
    print(file, max_index, max_value)

# pdf = Counter(queue_len)
# print(pdf)

long_queue = list(filter(lambda x: x[2] > 10, queue_len))
print(long_queue)
