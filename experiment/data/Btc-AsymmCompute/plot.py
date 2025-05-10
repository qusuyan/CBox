import pandas as pd
import matplotlib.pyplot as plt

plt.rcParams["font.family"] = "Gill Sans"
plt.rcParams["font.size"] = "8"
plt.rcParams["lines.markersize"] = 4
plt.rcParams["axes.linewidth"] = 0.6
plt.rcParams["xtick.major.size"] = 2
plt.rcParams["ytick.major.size"] = 2
plt.rcParams["xtick.major.width"] = 0.6
plt.rcParams["ytick.major.width"] = 0.6
plt.rcParams["xtick.major.pad"] = 2
plt.rcParams["ytick.major.pad"] = 2
plt.rcParams["legend.frameon"] = False
plt.rcParams["legend.labelspacing"] = 0.5
plt.rcParams["legend.columnspacing"] = 1.5
plt.rcParams["legend.handlelength"] = 1.4
plt.rcParams["legend.handletextpad"] = 0.5
plt.rcParams["legend.labelspacing"] = 0.2

reward_df = pd.read_csv("rewards.csv")

rewards_color = "crimson"
compute_color = "black"
hash_power_color = "gray"

fig, ax = plt.subplots(figsize=(3.33, 1.2))
ax.plot(reward_df["validator_0_ecores"], reward_df["comp_power"] * 100, color=compute_color, zorder=0, label="Compute Resource")
ax.plot(reward_df["validator_0_ecores"], reward_df["rewards"] * 100, 'o-', color=rewards_color, zorder=2, label="Reward")
ax.plot(reward_df["validator_0_ecores"], reward_df["hash_power"] * 100, 's-', color=hash_power_color, zorder=1, label="Hash Power")
ax.legend(ncol=3, loc=(0.005, 1))
ax.set_xlabel("Validator 0 eCores", labelpad=0)
ax.set_ylabel("Validator 0 (%)", labelpad=0)
ax.set_ylim((0, 35))

ax.tick_params(axis='x', length=2, pad=2)
ax.tick_params(axis='y', length=2, pad=2)

plt.subplots_adjust(left=0.09, right=0.93, top=0.87, bottom=0.21)
fig.savefig("plot.pdf", format="pdf")