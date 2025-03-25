import pandas as pd
import matplotlib.pyplot as plt

plt.rcParams["font.family"] = "Gill Sans"
plt.rcParams["font.size"] = "12"

data = pd.read_csv("timeline.csv")

fast_df = data.loc[data["node"] == 1]
slow_df = data.loc[data["node"] != 1].groupby("node")

fast_color = "crimson"
slow_color = "cornflowerblue"

fig, ax = plt.subplots(figsize=(3, 2.4))

# plot tput
for (_, df) in slow_df:
    ax.plot(df["time"], df["commits"] / 30 / 1000, "s-", color=slow_color)
ax.plot(fast_df["time"], fast_df["commits"] / 30 / 1000, "o-", color=fast_color)
ax.set_ylabel('Tput (ktps.)', labelpad=-1)
ax.set_ylim((0, 11))
ax.set_yticks(range(0, 11, 5))

ax.set_xlabel('Time (s)', labelpad=1)
ax.set_xticks(data["time"].drop_duplicates())
ax.tick_params(axis='x', labelrotation=90, length=2, pad=2)

plt.subplots_adjust(left=0.14, right=0.99, top=0.99, bottom=0.19)
# fig.tight_layout()
fig.savefig("plot.pdf", format="pdf")