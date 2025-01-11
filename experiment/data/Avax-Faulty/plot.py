import re

import pandas as pd
import matplotlib.pyplot as plt

plt.rcParams["font.family"] = "GillSansC"
plt.rcParams["font.size"] = "12"

df = pd.read_csv("agg_results.csv", header=[0,1,2])
df.columns = [c for (_, _, c) in df.columns[:2]] + [(a, b) for (a, b, _) in df.columns[2:]]

avax_df = df.loc[df["correct-type"] == "Basic"]
bliz_df = df.loc[df["correct-type"] == "Blizzard"]

avax_color = "cornflowerblue"
bliz_color = "crimson"

fig, ax = plt.subplots(figsize=(3, 2.4))
ax.plot(bliz_df["num-faulty"], bliz_df[("avg_tput", "mean")] / 1000, "s-", color=bliz_color, label="Blizzard")
ax.plot(avax_df["num-faulty"], avax_df[("avg_tput", "mean")] / 1000, "o-", color=avax_color, label="Avalanche")
ax.set_ylim((0, 3.6))
ax.legend()
ax.set_xlabel("# Faulty Validators", labelpad=0)
ax.set_ylabel("Throughput (ktps.)", labelpad=0)
ax.set_xticks(df["num-faulty"])

ax.tick_params(axis='x', length=2, pad=2)
ax.tick_params(axis='y', length=2, pad=2)

plt.subplots_adjust(left=0.1, right=0.99, top=0.99, bottom=0.15)
fig.savefig("plot.pdf", format="pdf")