import pandas as pd
import matplotlib.pyplot as plt

plt.rcParams["font.family"] = "Times New Roman"

data = pd.read_csv("agg_results.csv")
ecore1_df = data.loc[data["per-node-concurrency"] == 1]
ecore2_df = data.loc[data["per-node-concurrency"] == 2]

ecore1_color = "cornflowerblue"
ecore1_color_dark = "royalblue"
ecore2_color = "crimson"
ecore2_color_dark = "darkred"

fig, (tput_ax, sched_ax, msg_ax, cpu_ax) = plt.subplots(4, 1, sharex = True, figsize=(4, 6))
bar_width = 16

# plot tput
ecore1_max = ecore1_df["avg_tput"].max()
ecore2_max = ecore2_df["avg_tput"].max()
tput_exp_ax = tput_ax.twinx()
tput_exp_ax.axhline(y = ecore1_max / 1000, color=ecore1_color_dark, linestyle="--", linewidth=0.7)
tput_exp_ax.axhline(y = ecore2_max / 1000, color=ecore2_color_dark, linestyle="--", linewidth=0.7)
tput_exp_ax.set_ylim((0, 100))
tput_exp_ax.set_yticks([ecore1_max / 1000], ["87k"])
tput_ax.plot(ecore1_df["num-nodes"], ecore1_df["avg_tput"] / 1000, 'o-', color=ecore1_color, label="eCore=1")
tput_ax.plot(ecore2_df["num-nodes"], ecore2_df["avg_tput"] / 1000, 's-', color=ecore2_color, label="eCore=2")
tput_ax.set_ylabel('Throughput (ktps.)', labelpad=-3.5)
tput_ax.set_ylim((0, 100))
tput_ax.legend(loc="lower right")

# plot sched delay
sched_ax.axvline(x = 200, color=ecore1_color_dark, linestyle="--", linewidth=0.7)
sched_ax.axvline(x = 200, color=ecore2_color_dark, linestyle="--", linewidth=0.7)
sched_ax.plot(ecore1_df["num-nodes"], ecore1_df["sched_dur_ms"], 'o-', color=ecore1_color)
sched_ax.plot(ecore2_df["num-nodes"], ecore2_df["sched_dur_ms"], 's-', color=ecore2_color)
sched_ax.set_ylabel('Scheduling Delay (ms)', labelpad=6)
sched_ax.set_ylim((0, 2.5))

# plot msg delay
msg_ax.axvline(x = 200, color=ecore1_color_dark, linestyle="--", linewidth=0.7)
msg_ax.axvline(x = 200, color=ecore2_color_dark, linestyle="--", linewidth=0.7)
msg_delay_ax = msg_ax.twinx()
msg_delay_ax.bar(ecore1_df["num-nodes"]-bar_width/2, ecore1_df["deliver_late_dur_ms"], bar_width, color=ecore1_color, alpha=0.35, zorder=0)
msg_delay_ax.bar(ecore2_df["num-nodes"]+bar_width/2, ecore2_df["deliver_late_dur_ms"], bar_width, color=ecore2_color, alpha=0.35, zorder=0)
msg_delay_ax.set_ylabel("Msg Late Time (ms)", labelpad=0)
msg_delay_ax.set_ylim((0, 300))
msg_ax.plot(ecore1_df["num-nodes"], ecore1_df["deliver_late_chance"]*100, 'o-', color=ecore1_color, zorder=10)
msg_ax.plot(ecore2_df["num-nodes"], ecore2_df["deliver_late_chance"]*100, 's-', color=ecore2_color, zorder=10)
msg_ax.set_ylabel('Msg Late Chance (%)', labelpad=-1)
msg_ax.set_ylim((0, 0.3))

# plot cpu util
cpu_ax.axvline(x = 200, color=ecore1_color_dark, linestyle="--", linewidth=0.7)
cpu_ax.axvline(x = 200, color=ecore2_color_dark, linestyle="--", linewidth=0.7)
cpu_ax.plot(ecore1_df["num-nodes"], ecore1_df["avg_cpu"], 'o-', color=ecore1_color)
cpu_ax.plot(ecore2_df["num-nodes"], ecore2_df["avg_cpu"], 's-', color=ecore2_color)
cpu_ax.set_ylabel('CPU Utilization (%)', labelpad=-3)
cpu_ax.set_ylim((0, 100))

cpu_ax.set_xlabel('# Validators', labelpad=1)
cpu_ax.set_xticks(ecore2_df["num-nodes"])

plt.subplots_adjust(left=0.1, right=0.89, top=0.99, bottom=0.06, wspace=0, hspace=0.11)
# fig.tight_layout()
fig.savefig("plot.pdf", format="pdf")