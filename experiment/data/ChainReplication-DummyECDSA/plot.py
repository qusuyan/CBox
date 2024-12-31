import pandas as pd
import matplotlib.pyplot as plt

plt.rcParams["font.family"] = "GillSansC"
plt.rcParams["font.size"] = "12"

data = pd.read_csv("agg_results.csv")
ecore1_df = data.loc[data["per-node-concurrency"] == 1]
ecore2_df = data.loc[data["per-node-concurrency"] == 2]

ecore1_color = "cornflowerblue"
ecore1_color_dark = "royalblue"
ecore2_color = "crimson"
ecore2_color_dark = "darkred"

fig, (tput_ax, sched_ax, msg_ax, cpu_ax) = plt.subplots(4, 1, sharex = True, figsize=(3.5, 5))
bar_width = 16

# plot tput
tput_ax.axvline(x = 940, color=ecore1_color_dark, linestyle="--", linewidth=0.7)
tput_ax.axvline(x = 820, color=ecore2_color_dark, linestyle="--", linewidth=0.7)
ecore1_max = ecore1_df["avg_tput"].max()
ecore2_max = ecore2_df["avg_tput"].max()
tput_exp_ax = tput_ax.twinx()
tput_exp_ax.axhline(y = ecore1_max / 1000, color=ecore1_color_dark, linestyle="--", linewidth=0.7)
tput_exp_ax.axhline(y = ecore2_max / 1000, color=ecore2_color_dark, linestyle="--", linewidth=0.7)
tput_exp_ax.set_ylim((0, 13))
tput_exp_ax.set_yticks([ecore1_max / 1000, ecore2_max / 1000], ["6.04k", "12.0k"])
tput_ax.plot(ecore1_df["num-nodes"], ecore1_df["avg_tput"] / 1000, 'o-', color=ecore1_color)
tput_ax.plot(ecore2_df["num-nodes"], ecore2_df["avg_tput"] / 1000, 's-', color=ecore2_color)
tput_ax.set_ylabel('Tput (ktps.)', labelpad=2)
tput_ax.set_ylim((0, 13))

# plot sched delay
sched_ax.axvline(x = 940, color=ecore1_color_dark, linestyle="--", linewidth=0.7)
sched_ax.axvline(x = 820, color=ecore2_color_dark, linestyle="--", linewidth=0.7)
sched_ax.plot(ecore1_df["num-nodes"], ecore1_df["sched_dur_ms"], 'o-', color=ecore1_color, label="eCore=1")
sched_ax.plot(ecore2_df["num-nodes"], ecore2_df["sched_dur_ms"], 's-', color=ecore2_color, label="eCore=2")
sched_ax.set_ylabel('Sched Delay (ms)', labelpad=7)
sched_ax.set_ylim((0, 2.5))
sched_ax.legend(loc="upper left")

# plot msg delay
msg_ax.axvline(x = 940, color=ecore1_color_dark, linestyle="--", linewidth=0.7)
msg_ax.axvline(x = 820, color=ecore2_color_dark, linestyle="--", linewidth=0.7)
msg_delay_ax = msg_ax.twinx()
msg_delay_ax.bar(ecore1_df["num-nodes"]-bar_width/2, ecore1_df["deliver_late_dur_ms"], bar_width, color=ecore1_color, alpha=0.35, zorder=0)
msg_delay_ax.bar(ecore2_df["num-nodes"]+bar_width/2, ecore2_df["deliver_late_dur_ms"], bar_width, color=ecore2_color, alpha=0.35, zorder=0)
msg_delay_ax.set_ylabel("Msg Late Time (ms)", labelpad=0)
msg_delay_ax.set_ylim((0, 300))
msg_ax.plot(ecore1_df["num-nodes"], ecore1_df["deliver_late_chance"]*100, 'o-', color=ecore1_color, zorder=10)
msg_ax.plot(ecore2_df["num-nodes"], ecore2_df["deliver_late_chance"]*100, 's-', color=ecore2_color, zorder=10)
msg_ax.set_ylabel('Msg Late (%)', labelpad=0)
msg_ax.set_ylim((0, 0.299))

# plot cpu util
cpu_ax.axvline(x = 940, color=ecore1_color_dark, linestyle="--", linewidth=0.7)
cpu_ax.axvline(x = 820, color=ecore2_color_dark, linestyle="--", linewidth=0.7)
cpu_ax.plot(ecore1_df["num-nodes"], ecore1_df["avg_cpu"], 'o-', color=ecore1_color)
cpu_ax.plot(ecore2_df["num-nodes"], ecore2_df["avg_cpu"], 's-', color=ecore2_color)
cpu_ax.set_ylabel('CPU (%)', labelpad=-5)
cpu_ax.set_ylim((0, 100))

cpu_ax.set_xticks(ecore2_df["num-nodes"].iloc[::2])
cpu_ax.tick_params(axis='x', labelrotation=0, length=2, pad=2)
cpu_ax.set_xlabel('# Validators', labelpad=-1)

plt.subplots_adjust(left=0.13, right=0.855, top=0.995, bottom=0.065, wspace=0, hspace=0.14)
# fig.tight_layout()
fig.savefig("plot.pdf", format="pdf")