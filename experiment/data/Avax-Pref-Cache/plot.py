import pandas as pd
import matplotlib.pyplot as plt

plt.rcParams["font.family"] = "Gill Sans"
plt.rcParams["font.size"] = "12"

tput_df = pd.read_csv("agg_results.csv", header=[0,1,2])
tput_df.columns = [c for (_, _, c) in tput_df.columns[:2]] + [(a, b) for (a, b, _) in tput_df.columns[2:]]

pref_df = pd.read_csv("avax_pref_cache.csv", header=[0])

with_cache_tput = tput_df.loc[tput_df["correct-type"] == "Basic"]
without_cache_tput = tput_df.loc[tput_df["correct-type"] == "NoCache"]
with_cache_pref = pref_df.loc[pref_df["type"] == "Basic"]
without_cache_pref = pref_df.loc[pref_df["type"] == "NoCache"]

with_cache_color = "cornflowerblue"
without_cache_color = "crimson"

fig, (tput_ax, pref_ax) = plt.subplots(2, 1, sharex = True, figsize=(3, 4))

# plot tput
tput_ax.plot(with_cache_tput["num-faulty"], with_cache_tput[("avg_tput", "mean")] / 1000, "o-", color=with_cache_color, label="With Cache")
tput_ax.plot(without_cache_tput["num-faulty"], without_cache_tput[("avg_tput", "mean")] / 1000, "o-", color=without_cache_color, label="Without Cache")
tput_ax.set_ylabel('Tput (ktps.)', labelpad=4.5)
tput_ax.set_ylim((0, 2.9))
tput_ax.legend(ncol=2, loc=(-0.03, 0.98), columnspacing=0.8, handlelength=1.4, handletextpad=0.3, frameon=False)

# plot pref
barwidth = 0.8
# pref_ratio_ax = pref_ax.twinx()
# pref_ratio_ax.bar(with_cache_pref["num-faulty"] - barwidth/2, with_cache_pref["pref-checks-count"] / 1e6, barwidth, color=with_cache_color, alpha=0.35, zorder=0, label="W/ Cache")
# pref_ratio_ax.bar(without_cache_pref["num-faulty"] + barwidth/2, without_cache_pref["pref-checks-count"] / 1e6, barwidth, color=without_cache_color, alpha=0.35, zorder=0, label="W/ Cache")
pref_ax.bar(with_cache_pref["num-faulty"] - barwidth/2, with_cache_pref["checks-per-call"], barwidth, color=with_cache_color, label="W/ Cache")
pref_ax.bar(without_cache_pref["num-faulty"] + barwidth/2, without_cache_pref["checks-per-call"], barwidth, color=without_cache_color, label="W/o Cache")
pref_ax.set_ylabel('Txns Traversed / Txn Query', labelpad=-1.5)

pref_ax.set_xlabel('# Faulty', labelpad=-2)
pref_ax.set_xticks(tput_df["num-faulty"])
pref_ax.tick_params(axis='x', labelrotation=0, length=2, pad=2)


plt.subplots_adjust(left=0.135, right=0.99, top=0.94, bottom=0.09, wspace=0, hspace=0.07)
# fig.tight_layout()
fig.savefig("plot.pdf", format="pdf")