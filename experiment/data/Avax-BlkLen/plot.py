import pandas as pd
import matplotlib.pyplot as plt

plt.rcParams["font.family"] = ["Gill Sans", "DejaVu Sans"]
plt.rcParams["font.weight"] = "light"
plt.rcParams['axes.labelweight'] = 'light'
plt.rcParams["font.size"] = "10"
plt.rcParams['axes.linewidth'] = 0.1
plt.rcParams['xtick.major.width'] = 0.1
plt.rcParams['ytick.major.width'] = 0.1

data = pd.read_csv("agg_results.csv", header=[0,1,2])
data.columns = [c for (_, _, c) in data.columns[:2]] + [(a, b) for (a, b, _) in data.columns[2:]]

b20_df = data.loc[data["correct-config"] == "blk_len=20+max_inflight_blk=2147483647"]
b40_df = data.loc[data["correct-config"] == "blk_len=40+max_inflight_blk=2147483647"]

# b20i20_color = "powderblue"
b20_color = "lightskyblue"
b40_color = "cornflowerblue"

fig, (tput_ax, sched_ax, msg_ax, cpu_ax) = plt.subplots(4, 1, sharex = True, figsize=(3.5, 5))
bar_width = 1.5

# plot tput
tput_ax.bar(b20_df["num-nodes"]-bar_width/2, b20_df[("avg_tput", "mean")] / 1000, bar_width, color=b20_color)
tput_ax.bar(b40_df["num-nodes"]+bar_width/2, b40_df[("avg_tput", "mean")] / 1000, bar_width, color=b40_color)
tput_ax.set_ylabel('Tput (ktps.)', labelpad=7)
tput_ax.set_ylim((0, 5))

# plot sched delay
sched_ax.plot(b20_df["num-nodes"], b20_df[("sched_dur_ms", "mean")], '^-', color=b20_color, label="20 txns/block")
sched_ax.plot(b40_df["num-nodes"], b40_df[("sched_dur_ms", "mean")], 's-', color=b40_color, label="40 txns/block")
sched_ax.set_ylabel('Sched Delay (ms)', labelpad=7)
sched_ax.set_ylim((0, 2.5))
sched_ax.legend()

# plot msg delay
msg_delay_ax = msg_ax.twinx()
msg_delay_ax.bar(b20_df["num-nodes"]-bar_width/2, b20_df[("deliver_late_dur_ms", "mean")], bar_width, color=b20_color, alpha=0.45)
msg_delay_ax.bar(b40_df["num-nodes"]+bar_width/2, b40_df[("deliver_late_dur_ms", "mean")], bar_width, color=b40_color, alpha=0.45)
msg_delay_ax.set_ylabel("Msg Late Time (ms)", labelpad=0)
msg_delay_ax.set_ylim((0, 100))
msg_ax.plot(b20_df["num-nodes"], b20_df[("deliver_late_chance", "mean")]*10000, '^-', color=b20_color)
msg_ax.plot(b40_df["num-nodes"], b40_df[("deliver_late_chance", "mean")]*10000, 's-', color=b40_color)
msg_ax.set_ylabel('Msg Late (‱)', labelpad=-0.2)
msg_ax.set_ylim((0, 1.2))

# plot cpu util
cpu_ax.plot(b20_df["num-nodes"], b20_df[("avg_cpu", "mean")], '^-', color=b20_color)
cpu_ax.plot(b40_df["num-nodes"], b40_df[("avg_cpu", "mean")], 's-', color=b40_color)
cpu_ax.set_ylabel('CPU (%)', labelpad=-4)
cpu_ax.set_ylim((0, 100))

cpu_ax.set_xlabel('# Validators', labelpad=-1)
cpu_ax.set_xticks(b40_df["num-nodes"])
cpu_ax.tick_params(axis='x', labelrotation=0, length=2, pad=2)


plt.subplots_adjust(left=0.115, right=0.87, top=0.995, bottom=0.057, wspace=0, hspace=0.1)
# fig.tight_layout()
fig.savefig("plot.pdf", format="pdf")