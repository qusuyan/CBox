import pandas as pd
import matplotlib.pyplot as plt

plt.rcParams["font.family"] = ["Gill Sans", "DejaVu Sans"]
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

data = pd.read_csv("agg_results.csv", header=[0,1,2])
data.columns = [c for (_, _, c) in data.columns[:2]] + [(a, b) for (a, b, _) in data.columns[2:]]

b20_df = data.loc[data["correct-config"] == "blk_len=20+max_inflight_blk=2147483647"]
b40_df = data.loc[data["correct-config"] == "blk_len=40+max_inflight_blk=2147483647"]

# b20i20_color = "powderblue"
b20_color = "lightskyblue"
b40_color = "cornflowerblue"

fig, ((tput_ax, msg_ax), (sched_ax, cpu_ax)) = plt.subplots(2, 2, sharex = True, figsize=(3.5, 2.2))
bar_width = 3.6

# plot tput
tput_ax.bar(b20_df["num-nodes"]-bar_width/2, b20_df[("avg_tput", "mean")] / 1000, bar_width, color=b20_color)
tput_ax.bar(b40_df["num-nodes"]+bar_width/2, b40_df[("avg_tput", "mean")] / 1000, bar_width, color=b40_color)
tput_ax.set_ylabel('Tput (ktps.)', labelpad=0.5)
tput_ax.set_ylim((0, 8))

# plot sched delay
sched_ax.plot(b20_df["num-nodes"], b20_df[("sched_dur_ms", "mean")], '^-', color=b20_color)
sched_ax.plot(b40_df["num-nodes"], b40_df[("sched_dur_ms", "mean")], 's-', color=b40_color)
sched_ax.set_ylabel('Sched Delay (ms)', labelpad=0.5)
sched_ax.set_ylim((0, 2.5))
sched_ax.set_xlabel('# Validators', labelpad=0)

# plot msg delay
msg_delay_ax = msg_ax.twinx()
msg_delay_ax.bar(b20_df["num-nodes"]-bar_width/2, b20_df[("deliver_late_dur_ms", "mean")], bar_width, color=b20_color, alpha=0.45)
msg_delay_ax.bar(b40_df["num-nodes"]+bar_width/2, b40_df[("deliver_late_dur_ms", "mean")], bar_width, color=b40_color, alpha=0.45)
msg_delay_ax.set_ylabel("Msg Late Time (ms)", labelpad=0)
msg_delay_ax.set_ylim((0, 80))
msg_ax.plot(b20_df["num-nodes"], b20_df[("deliver_late_chance", "mean")]*100, '^-', color=b20_color, label="20 txns/block")
msg_ax.plot(b40_df["num-nodes"], b40_df[("deliver_late_chance", "mean")]*100, 's-', color=b40_color, label="40 txns/block")
msg_delay_ax.set_zorder(5)
msg_ax.set_zorder(10)
msg_ax.patch.set_visible(False)
msg_ax.set_ylabel('Msg Late (%)', labelpad=-0.2)
msg_ax.set_ylim((0, 1.2))

msg_ax.legend(ncol=3, loc=(-0.85, 1))

# plot cpu util
cpu_ax.plot(b20_df["num-nodes"], b20_df[("avg_cpu", "mean")], '^-', color=b20_color)
cpu_ax.plot(b40_df["num-nodes"], b40_df[("avg_cpu", "mean")], 's-', color=b40_color)
cpu_ax.set_ylabel('CPU (%)', labelpad=-4)
cpu_ax.set_ylim((0, 100))

cpu_ax.set_xlabel('# Validators', labelpad=0)
cpu_ax.set_xticks(b40_df["num-nodes"])


plt.subplots_adjust(left=0.065, right=0.92, top=0.93, bottom=0.115, wspace=0.24, hspace=0.13)
# fig.tight_layout()
fig.savefig("plot.pdf", format="pdf")