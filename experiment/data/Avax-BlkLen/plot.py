import pandas as pd
import matplotlib.pyplot as plt

plt.rcParams["font.family"] = "Times New Roman"

data = pd.read_csv("agg_results.csv")
b20i20_df = data.loc[data["correct-config"] == "blk_len=20+max_inflight_blk=20"]
b20i40_df = data.loc[data["correct-config"] == "blk_len=20+max_inflight_blk=40"]
b40i20_df = data.loc[data["correct-config"] == "blk_len=40+max_inflight_blk=20"]

b20i20_color = "powderblue"
b20i40_color = "lightskyblue"
b40i20_color = "cornflowerblue"

fig, (tput_ax, sched_ax, msg_ax, cpu_ax) = plt.subplots(4, 1, sharex = True, figsize=(4, 6))
bar_width = 2.5

# plot tput
tput_ax.bar(b20i20_df["num-nodes"]-bar_width, b20i20_df["avg_tput"] / 1000, bar_width, color=b20i20_color)
tput_ax.bar(b20i40_df["num-nodes"], b20i40_df["avg_tput"] / 1000, bar_width, color=b20i40_color)
tput_ax.bar(b40i20_df["num-nodes"]+bar_width, b40i20_df["avg_tput"] / 1000, bar_width, color=b40i20_color)
tput_ax.set_ylabel('Throughput (ktps.)', labelpad=1)
tput_ax.set_ylim((0, 6))

# plot sched delay
sched_ax.plot(b20i20_df["num-nodes"], b20i20_df["sched_dur_ms"], 'o-', color=b20i20_color, label="20 txns/block, 20 blocks inflight")
sched_ax.plot(b20i40_df["num-nodes"], b20i40_df["sched_dur_ms"], '^-', color=b20i40_color, label="20 txns/block, 40 blocks inflight")
sched_ax.plot(b40i20_df["num-nodes"], b40i20_df["sched_dur_ms"], 's-', color=b40i20_color, label="40 txns/block, 20 blocks inflight")
sched_ax.set_ylabel('Scheduling Delay (ms)', labelpad=6)
sched_ax.set_ylim((0, 2.5))
sched_ax.legend()

# plot msg delay
msg_delay_ax = msg_ax.twinx()
msg_delay_ax.bar(b20i20_df["num-nodes"]-bar_width, b20i20_df["deliver_late_dur_ms"], bar_width, color=b20i20_color, alpha=0.35)
msg_delay_ax.bar(b20i40_df["num-nodes"], b20i40_df["deliver_late_dur_ms"], bar_width, color=b20i40_color, alpha=0.35)
msg_delay_ax.bar(b40i20_df["num-nodes"]+bar_width, b40i20_df["deliver_late_dur_ms"], bar_width, color=b40i20_color, alpha=0.35)
msg_delay_ax.set_ylabel("Msg Late Time (ms)", labelpad=0)
msg_delay_ax.set_ylim((0, 300))
msg_ax.plot(b20i20_df["num-nodes"], b20i20_df["deliver_late_chance"]*100, 'o-', color=b20i20_color)
msg_ax.plot(b20i40_df["num-nodes"], b20i40_df["deliver_late_chance"]*100, '^-', color=b20i40_color)
msg_ax.plot(b40i20_df["num-nodes"], b40i20_df["deliver_late_chance"]*100, 's-', color=b40i20_color)
msg_ax.set_ylabel('Msg Late Chance (%)', labelpad=-1)
msg_ax.set_ylim((0, 0.3))

# plot cpu util
cpu_ax.plot(b20i20_df["num-nodes"], b20i20_df["avg_cpu"], 'o-', color=b20i20_color)
cpu_ax.plot(b20i40_df["num-nodes"], b20i40_df["avg_cpu"], '^-', color=b20i40_color)
cpu_ax.plot(b40i20_df["num-nodes"], b40i20_df["avg_cpu"], 's-', color=b40i20_color)
cpu_ax.set_ylabel('CPU Utilization (%)', labelpad=-4)
cpu_ax.set_ylim((0, 100))

cpu_ax.set_xlabel('# Validators', labelpad=1)
cpu_ax.set_xticks(b20i20_df["num-nodes"])

plt.subplots_adjust(left=0.1, right=0.89, top=0.99, bottom=0.06, wspace=0, hspace=0.11)
# fig.tight_layout()
fig.savefig("plot.pdf", format="pdf")