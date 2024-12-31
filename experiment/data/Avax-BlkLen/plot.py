import pandas as pd
import matplotlib.pyplot as plt

plt.rcParams["font.family"] = "GillSansC"
plt.rcParams["font.size"] = "12"

data = pd.read_csv("agg_results.csv", header=[0,1,2])
data.columns = [c for (_, _, c) in data.columns[:2]] + [(a, b) for (a, b, _) in data.columns[2:]]

# b20i20_df = data.loc[data["correct-config"] == "blk_len=20+max_inflight_blk=20"]
# b20i40_df = data.loc[data["correct-config"] == "blk_len=20+max_inflight_blk=40"]
# b40i20_df = data.loc[data["correct-config"] == "blk_len=40+max_inflight_blk=20"]
b20i40_df = data.loc[data["correct-config"] == "blk_len=20+max_inflight_blk=2147483647"]
b40i20_df = data.loc[data["correct-config"] == "blk_len=40+max_inflight_blk=2147483647"]

# b20i20_color = "powderblue"
b20i40_color = "lightskyblue"
b40i20_color = "cornflowerblue"

fig, (tput_ax, sched_ax, msg_ax, cpu_ax) = plt.subplots(4, 1, sharex = True, figsize=(3.5, 5))
bar_width = 1.5

# plot tput
# tput_ax.bar(b20i20_df["num-nodes"]-bar_width, b20i20_df["avg_tput"] / 1000, bar_width, color=b20i20_color)
tput_ax.bar(b20i40_df["num-nodes"]-bar_width/2, b20i40_df[("avg_tput", "mean")] / 1000, bar_width, color=b20i40_color)
tput_ax.bar(b40i20_df["num-nodes"]+bar_width/2, b40i20_df[("avg_tput", "mean")] / 1000, bar_width, color=b40i20_color)
tput_ax.set_ylabel('Tput (ktps.)', labelpad=8)
tput_ax.set_ylim((0, 8))

# plot sched delay
# sched_ax.plot(b20i20_df["num-nodes"], b20i20_df["sched_dur_ms"], 'o-', color=b20i20_color, label="20 txns/block, 20 blocks inflight")
sched_ax.axvline(x = 27.5, color=b20i40_color, linestyle="--", linewidth=0.7)
sched_ax.axvline(x = 32.5, color=b40i20_color, linestyle="--", linewidth=0.7)
sched_ax.plot(b20i40_df["num-nodes"], b20i40_df[("sched_dur_ms", "mean")], '^-', color=b20i40_color, label="20 txns/block")
sched_ax.plot(b40i20_df["num-nodes"], b40i20_df[("sched_dur_ms", "mean")], 's-', color=b40i20_color, label="40 txns/block")
sched_ax.set_ylabel('Sched Delay (ms)', labelpad=8)
sched_ax.set_ylim((0, 2.5))
sched_ax.legend()

# plot msg delay
msg_ax.axvline(x = 27.5, color=b20i40_color, linestyle="--", linewidth=0.7)
msg_ax.axvline(x = 32.5, color=b40i20_color, linestyle="--", linewidth=0.7)
msg_delay_ax = msg_ax.twinx()
# msg_delay_ax.bar(b20i20_df["num-nodes"]-bar_width, b20i20_df["deliver_late_dur_ms"], bar_width, color=b20i20_color, alpha=0.35)
msg_delay_ax.bar(b20i40_df["num-nodes"]-bar_width/2, b20i40_df[("deliver_late_dur_ms", "mean")], bar_width, color=b20i40_color, alpha=0.45)
msg_delay_ax.bar(b40i20_df["num-nodes"]+bar_width/2, b40i20_df[("deliver_late_dur_ms", "mean")], bar_width, color=b40i20_color, alpha=0.45)
msg_delay_ax.set_ylabel("Msg Late Time (ms)", labelpad=0)
msg_delay_ax.set_ylim((0, 300))
# msg_ax.plot(b20i20_df["num-nodes"], b20i20_df["deliver_late_chance"]*100, 'o-', color=b20i20_color)
msg_ax.plot(b20i40_df["num-nodes"], b20i40_df[("deliver_late_chance", "mean")]*100, '^-', color=b20i40_color)
msg_ax.plot(b40i20_df["num-nodes"], b40i20_df[("deliver_late_chance", "mean")]*100, 's-', color=b40i20_color)
msg_ax.set_ylabel('Msg Late (%)', labelpad=-0.2)
msg_ax.set_ylim((0, 0.3))

# plot cpu util
# cpu_ax.plot(b20i20_df["num-nodes"], b20i20_df["avg_cpu"], 'o-', color=b20i20_color)
cpu_ax.axvline(x = 27.5, color=b20i40_color, linestyle="--", linewidth=0.7)
cpu_ax.axvline(x = 32.5, color=b40i20_color, linestyle="--", linewidth=0.7)
cpu_ax.plot(b20i40_df["num-nodes"], b20i40_df[("avg_cpu", "mean")], '^-', color=b20i40_color)
cpu_ax.plot(b40i20_df["num-nodes"], b40i20_df[("avg_cpu", "mean")], 's-', color=b40i20_color)
cpu_ax.set_ylabel('CPU (%)', labelpad=-4)
cpu_ax.set_ylim((0, 100))

cpu_ax.set_xlabel('# Validators', labelpad=-1)
cpu_ax.set_xticks(b40i20_df["num-nodes"])
cpu_ax.tick_params(axis='x', labelrotation=0, length=2, pad=2)


plt.subplots_adjust(left=0.13, right=0.855, top=0.995, bottom=0.065, wspace=0, hspace=0.14)
# fig.tight_layout()
fig.savefig("plot.pdf", format="pdf")