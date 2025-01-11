import re

import pandas as pd
import matplotlib.pyplot as plt

plt.rcParams["font.family"] = "GillSansC"
plt.rcParams["font.size"] = "12"

df = pd.read_csv("agg_results.csv", header=[0,1,2])
df.columns = [c for (_, _, c) in df.columns[:2]] + [(a, b) for (a, b, _) in df.columns[2:]]
df["beta"] = df["correct-config"].map(lambda x: int(re.match(".*beta1=(\d+).*", x).groups()[0]))

avax_df = df.loc[df["correct-type"] == "Basic"].sort_values("beta")
bliz_df = df.loc[df["correct-type"] == "Blizzard"].sort_values("beta")

avax_color = "cornflowerblue"
bliz_color = "crimson"

fig, ax = plt.subplots(figsize=(3, 2.4))
ax.plot(avax_df["beta"], avax_df[("avg_tput", "mean")], "o-", color=avax_color, label="Avalanche")
ax.plot(bliz_df["beta"], bliz_df[("avg_tput", "mean")], "s-", color=bliz_color, label="Blizzard")
ax.set_ylim((0, 105))
ax.legend(loc="upper right")
ax.set_xlabel("β", labelpad=-2, family = "DejaVu Sans")
ax.set_ylabel("Throughput (ktps.)", labelpad=-4)
ax.set_xticks([8, 16, 32, 64, 96, 128, 160])

ax.tick_params(axis='x', length=2, pad=2)
ax.tick_params(axis='y', length=2, pad=2)

plt.subplots_adjust(left=0.137, right=0.99, top=0.99, bottom=0.15)
fig.savefig("plot.pdf", format="pdf")