import pandas as pd
import matplotlib.pyplot as plt

plt.rcParams["font.family"] = "Gill Sans"
plt.rcParams["font.size"] = "12"

data = pd.read_csv("agg_results.csv", header=[0,1,2])
data.columns = [c for (_, _, c) in data.columns[:2]] + [(a, b) for (a, b, _) in data.columns[2:]]

df1 = data.loc[data["correct-config"] == "blk_size=1048576+max_inflight_blk=1"]
df10 = data.loc[data["correct-config"] == "blk_size=104857+max_inflight_blk=10"]
df20 = data.loc[data["correct-config"] == "blk_size=52428+max_inflight_blk=20"]
df40 = data.loc[data["correct-config"] == "blk_size=26214+max_inflight_blk=40"]

df1_color = "cornflowerblue"
df10_color = "mediumpurple"
df20_color = "orange"
df40_color = "crimson"

fig, ax = plt.subplots(figsize=(3, 2.4))

# plot tput
ax.plot(df1["num-nodes"], df1[("avg_tput", "mean")] / 1000, "o-", color=df1_color, label="1MB Block")
ax.plot(df10["num-nodes"], df10[("avg_tput", "mean")] / 1000, "s-", color=df10_color, label="102.4KB Block")
ax.plot(df20["num-nodes"], df20[("avg_tput", "mean")] / 1000, "^-", color=df20_color, label="51.2KB Block")
ax.plot(df40["num-nodes"], df40[("avg_tput", "mean")] / 1000, "x-", color=df40_color, label="25.6KB Block")
ax.set_ylabel('Tput (ktps.)', labelpad=-1)
ax.set_ylim((0, 16))
ax.set_yticks(range(0, 20, 5))

ax.set_xlabel('# Validators', labelpad=-1)
ax.set_xticks(data["num-nodes"])
ax.tick_params(axis='x', labelrotation=0, length=2, pad=2)

ax.legend()

plt.subplots_adjust(left=0.14, right=0.99, top=0.99, bottom=0.14)
# fig.tight_layout()
fig.savefig("plot.pdf", format="pdf")