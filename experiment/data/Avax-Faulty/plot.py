import pandas as pd
import matplotlib.pyplot as plt

plt.rcParams["font.family"] = "Gill Sans"
plt.rcParams["font.size"] = "12"

tput_df = pd.read_csv("agg_results.csv", header=[0,1,2])
tput_df.columns = [c for (_, _, c) in tput_df.columns[:2]] + [(a, b) for (a, b, _) in tput_df.columns[2:]]

success_rate_df = pd.read_csv("avax_query_success.csv", header=[0])

avax_tput_df = tput_df.loc[tput_df["correct-type"] == "Basic"]
bliz_tput_df = tput_df.loc[tput_df["correct-type"] == "Blizzard"]
avax_succ_df = success_rate_df.loc[success_rate_df["correct-type"] == "Basic"]
bliz_succ_df = success_rate_df.loc[success_rate_df["correct-type"] == "Blizzard"]

avax_color = "cornflowerblue"
bliz_color = "crimson"

fig, (tput_ax, success_ax) = plt.subplots(2, 1, sharex=True, figsize=(3, 4))
tput_ax.plot(avax_tput_df["num-faulty"], avax_tput_df[("avg_tput", "mean")] / 1000, "o-", color=avax_color, label="Avalanche", zorder=1)
tput_ax.plot(bliz_tput_df["num-faulty"], bliz_tput_df[("avg_tput", "mean")] / 1000, "s-", color=bliz_color, label="Blizzard", zorder=0)
tput_ax.set_ylabel("Throughput (ktps.)", labelpad=0)
tput_ax.set_ylim((0, 2.2))
tput_ax.legend()

success_ax.plot(bliz_succ_df["num-faulty"], bliz_succ_df["success-rate"] * 100, "s-", color=bliz_color, label="Blizzard")
success_ax.plot(avax_succ_df["num-faulty"], avax_succ_df["success-rate"] * 100, "o-", color=avax_color, label="Avalanche")
success_ax.set_ylabel("Success Rate (%)", labelpad=0)
success_ax.set_ylim((0, 100))

success_ax.set_xlabel("# Faulty Validators", labelpad=0)
success_ax.set_xticks(tput_df["num-faulty"])

tput_ax.tick_params(axis='x', length=2, pad=2)
tput_ax.tick_params(axis='y', length=2, pad=2)
success_ax.tick_params(axis='x', length=2, pad=2)
success_ax.tick_params(axis='y', length=2, pad=2)

plt.subplots_adjust(left=0.1, right=0.99, top=0.99, bottom=0.15)
fig.savefig("plot.pdf", format="pdf")