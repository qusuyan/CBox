import pandas as pd
import matplotlib.pyplot as plt

plt.rcParams["font.family"] = "GillSansC"
plt.rcParams["font.size"] = "12"

reward_df = pd.read_csv("rewards.csv")

rewards_color = "cornflowerblue"
hash_power_color = "black"

fig, ax = plt.subplots(figsize=(3.5, 2))
ax.plot(reward_df["validator_0_ecores"], reward_df["hash_power"] * 100, color=hash_power_color, label="Expected")
ax.plot(reward_df["validator_0_ecores"], reward_df["rewards"] * 100, 'o-', color=rewards_color, label="Actual")
ax.legend()
ax.set_xlabel("Validator 0 eCores", labelpad=-1)
ax.set_ylabel("Validator 0 Rewards (%)", labelpad=0)
ax.set_ylim((0, 35))

ax.tick_params(axis='x', length=2, pad=2)
ax.tick_params(axis='y', length=2, pad=2)

plt.subplots_adjust(left=0.11, right=0.99, top=0.99, bottom=0.165)
fig.savefig("plot.pdf", format="pdf")