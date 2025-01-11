import math
import numpy as np
import pandas as pd
import matplotlib.pyplot as plt

plt.rcParams["font.family"] = "GillSansC"
plt.rcParams["font.size"] = "12"

lat = {}
for size in range(10, 31, 5):
    df = pd.read_csv(f"{size}.csv")
    lat[size] = df["Latency (s)"]

range = (math.log(0.2), math.log(12))
bin_count = 300
histograms = {k: np.histogram(v.map(math.log), bins=bin_count, range=range) for k, v in lat.items()}

fig, ax = plt.subplots(figsize=(6, 2))
bin_len = (range[1] - range[0]) / bin_count

for size, (counts, bins) in histograms.items():
    bin_median = [bin + bin_len / 2 for bin in bins[:-1]]
    density = counts / lat[size].count() * 50
    lefts = size - density
    ax.barh(bin_median, density * 2, left=lefts, height=bin_len, color="cornflowerblue")

ticks = [0.2, 0.5, 1, 2, 5, 10]
ax.set_yticks(ticks=[math.log(tick) for tick in ticks], labels=ticks)
ax.set_ylabel("Latency (s)", labelpad=0)
ax.set_xlim((7.5, 32.5))
ax.set_xlabel("# Validators", labelpad=0)

ax.tick_params(axis='x', length=2, pad=2)
ax.tick_params(axis='y', length=2, pad=2)

plt.subplots_adjust(left=0.07, right=0.995, top=0.99, bottom=0.18)
fig.savefig("plot.pdf", format="pdf")
