import pandas as pd
import matplotlib.pyplot as plt

plt.rcParams["font.family"] = "Gill Sans"
plt.rcParams["font.size"] = "12"

data = pd.read_csv("agg_results.csv", header=[0,1,2])
data.columns = [c for (_, _, c) in data.columns[:2]] + [(a, b) for (a, b, _) in data.columns[2:]]

with_cache_df = data.loc[data["correct-type"] == "Basic"]
without_cache_df = data.loc[data["correct-type"] == "NoCache"]

with_cache_color = "cornflowerblue"
without_cache_color = "crimson"

fig, ax = plt.subplots(figsize=(3, 2.4))

# plot tput
ax.plot(with_cache_df["num-faulty"], with_cache_df[("avg_tput", "mean")] / 1000, "o-", color=with_cache_color, label="W/ Cache")
ax.plot(without_cache_df["num-faulty"], without_cache_df[("avg_tput", "mean")] / 1000, "o-", color=without_cache_color, label="W/o Cache")
ax.set_ylabel('Tput (ktps.)', labelpad=0)
ax.set_ylim((0, 2.9))

ax.set_xlabel('# Faulty', labelpad=-1)
ax.set_xticks(data["num-faulty"])
ax.tick_params(axis='x', labelrotation=0, length=2, pad=2)

ax.legend()

plt.subplots_adjust(left=0.155, right=0.99, top=0.99, bottom=0.14)
# fig.tight_layout()
fig.savefig("plot.pdf", format="pdf")