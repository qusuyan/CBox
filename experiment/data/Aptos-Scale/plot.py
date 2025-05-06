import numpy as np
import pandas as pd
import matplotlib.pyplot as plt
from matplotlib.patches import Polygon

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

data = pd.read_csv("agg_results.csv", header=[0,1,2])
data.columns = [c for (_, _, c) in data.columns[:2]] + [(a, b) for (a, b, _) in data.columns[2:]]

bottleneck = pd.read_csv("bottleneck.csv", header=[0])
bottleneck = bottleneck.loc[bottleneck["num-nodes"] <= 32]

aptos_1_tput_df = data.loc[data["correct-config"] == "narwhal_blk_size=1048576+diem_blk_len=1"]
aptos_4_tput_df = data.loc[data["correct-config"] == "narwhal_blk_size=262144+diem_blk_len=4"]
aptos_16_tput_df = data.loc[data["correct-config"] == "narwhal_blk_size=65536+diem_blk_len=16"]

aptos_1_color = "gold"
aptos_4_color = "olive"
aptos_16_color = "tab:green"
bar_width = 1

fig, ((tput_ax, msg_ax), (sched_ax, cpu_ax)) = plt.subplots(2, 2, sharex = True, figsize=(3.5, 2.2))

# plot tput
tput_ax.plot(aptos_1_tput_df["num-nodes"], aptos_1_tput_df[("avg_tput", "mean")] / 1000, "o-", color=aptos_1_color, zorder=2, label="1×1MB")
tput_ax.plot(aptos_4_tput_df["num-nodes"], aptos_4_tput_df[("avg_tput", "mean")] / 1000, "s-", color=aptos_4_color, zorder=1, label="4×256KB")
tput_ax.plot(aptos_16_tput_df["num-nodes"], aptos_16_tput_df[("avg_tput", "mean")] / 1000, "^-", color=aptos_16_color, zorder=0, label="16×64KB")
tput_ax.set_ylabel('Tput (ktps.)', labelpad=-0.5)
tput_ax.set_ylim((0, 60))
tput_ax.legend(ncol = 4, loc = (0.2, 0.97))

# plot sched delay
sched_ax.plot(aptos_1_tput_df["num-nodes"], aptos_1_tput_df[("sched_dur_ms", "mean")], 'o-', color=aptos_1_color, zorder=2)
sched_ax.plot(aptos_4_tput_df["num-nodes"], aptos_4_tput_df[("sched_dur_ms", "mean")], 's-', color=aptos_4_color, zorder=1)
sched_ax.plot(aptos_16_tput_df["num-nodes"], aptos_16_tput_df[("sched_dur_ms", "mean")], '^-', color=aptos_16_color, zorder=0)
sched_ax.set_ylabel('Sched Delay (ms)', labelpad=3)
sched_ax.set_ylim((0, 2))

sched_ax.set_xlabel('# Validators', labelpad=0)

# plot msg delay
msg_delay_ax = msg_ax.twinx()
msg_delay_ax.bar(aptos_1_tput_df["num-nodes"] - bar_width, aptos_1_tput_df[("deliver_late_dur_ms", "mean")], bar_width, color=aptos_1_color, alpha=0.35, zorder=2)
msg_delay_ax.bar(aptos_4_tput_df["num-nodes"], aptos_4_tput_df[("deliver_late_dur_ms", "mean")], bar_width, color=aptos_4_color, alpha=0.35, zorder=1)
msg_delay_ax.bar(aptos_16_tput_df["num-nodes"] + bar_width, aptos_16_tput_df[("deliver_late_dur_ms", "mean")], bar_width, color=aptos_16_color, alpha=0.35, zorder=0)
msg_delay_ax.set_ylabel("Msg Late Time (ms)", labelpad=0.5)
msg_delay_ax.set_ylim((0, 80))
msg_ax.plot(aptos_1_tput_df["num-nodes"], aptos_1_tput_df[("deliver_late_chance", "mean")]*100, 'o-', color=aptos_1_color, zorder=12)
msg_ax.plot(aptos_4_tput_df["num-nodes"], aptos_4_tput_df[("deliver_late_chance", "mean")]*100, 's-', color=aptos_4_color, zorder=11)
msg_ax.plot(aptos_16_tput_df["num-nodes"], aptos_16_tput_df[("deliver_late_chance", "mean")]*100, '^-', color=aptos_16_color, zorder=10)
msg_delay_ax.set_zorder(5)
msg_ax.set_zorder(10)
msg_ax.patch.set_visible(False)
msg_ax.set_ylabel('Msg Late (%)', labelpad=0)
msg_ax.set_ylim((0, 12))

# plot cpu util
cpu_ax.plot(aptos_1_tput_df["num-nodes"], aptos_1_tput_df[("avg_cpu", "mean")], 'o-', color=aptos_1_color, zorder=2)
cpu_ax.plot(aptos_4_tput_df["num-nodes"], aptos_4_tput_df[("avg_cpu", "mean")], 's-', color=aptos_4_color, zorder=1)
cpu_ax.plot(aptos_16_tput_df["num-nodes"], aptos_16_tput_df[("avg_cpu", "mean")], '^-', color=aptos_16_color, zorder=0)
cpu_ax.set_ylabel('CPU (%)', labelpad=-4)
cpu_ax.set_ylim((0, 100))

cpu_ax.set_xticks(list(range(8, 65, 8)))
cpu_ax.set_xlabel('# Validators', labelpad=0)

plt.subplots_adjust(left=0.075, right=0.92, top=0.93, bottom=0.115, wspace=0.24, hspace=0.13)
# fig.tight_layout()
fig.savefig("plot.pdf", format="pdf")


# plot bottlneck
bottleneck_fig, bottleneck_ax = plt.subplots(1, 1, figsize=(3.5, 1.2))

max_width = 1
space_between = 0.1
stage_pos = [5, 4, 3, 2, 1, 0]
for (_, record) in bottleneck.iterrows():
    txns = np.array([record["txns-sent"], record["txn-validation"], record["block-management"], record["block-dissemination"], record["decide"], record["commit"]])
    if record["correct-config"] == "narwhal_blk_size=1048576+diem_blk_len=1":
        offset = record["num-nodes"] - (max_width + space_between)
        style = "s-"
        color = aptos_1_color
        label = "1×1MB"
    elif record["correct-config"] == "narwhal_blk_size=262144+diem_blk_len=4":
        offset = record["num-nodes"]
        style = "^-"
        color = aptos_4_color
        label = "4×256KB"
    elif record["correct-config"] == "narwhal_blk_size=65536+diem_blk_len=16":
        offset = record["num-nodes"] + (max_width + space_between)
        style = "v-"
        color = aptos_16_color
        label = "16×64KB"
    txns_norm = txns / txns.max()
    bottleneck_ax.fill_betweenx(stage_pos, offset - txns_norm * max_width / 2, offset + txns_norm * max_width / 2, color=color, lw=0.5,alpha=1, zorder=0, label=label)
    stage_diff = txns_norm[0:5] - txns_norm[1:6]
    bottleneck_color = 'firebrick'
    for (idx, diff) in reversed(list(enumerate(stage_diff))):
        if diff > 0.1:
            # x = [offset - txns_norm[idx+1] * max_width / 2, offset + txns_norm[idx+1] * max_width / 2] 
            # y = [5-(idx+1), 5-(idx+1)] 
            points = [[offset - txns_norm[idx+1] * max_width / 2, 5-(idx+1)], [offset + txns_norm[idx+1] * max_width / 2, 5-(idx+1)], [offset + txns_norm[idx] * max_width / 2, 5-(idx)], [offset - txns_norm[idx] * max_width / 2, 5-(idx)]]
            p = Polygon(points, color=bottleneck_color, fill=False, zorder=2)
            bottleneck_ax.add_patch(p) 
            bottleneck_color = 'black'


for y in stage_pos:
    bottleneck_ax.hlines(y, 0, 30, colors='black', linestyles='dashed', linewidth=0.5, alpha=0.35, zorder=1)

bottleneck_ax.legend(ncol=3, loc=(0.15, 0.97), handlelength=0.7)

# bottleneck_ax.yaxis.tick_right()
bottleneck_ax.set_yticks([4.5, 3.5, 2.5, 1.5, 0.5])
bottleneck_ax.set_yticklabels(["Txn Valid", "Blk Mng", "Blk Dissem", "Decide", "Commit"])
bottleneck_ax.set_ylim(0, 5)
bottleneck_ax.tick_params(axis='y', labelrotation=0, length=0, pad=2)
bottleneck_ax.set_xlabel('# Validators', labelpad=0)
bottleneck_ax.set_xticks(data["num-nodes"])
bottleneck_ax.set_xlim((2, 26))


plt.subplots_adjust(left=0.15, right=0.95, top=0.88, bottom=0.21, wspace=0, hspace=0)
# fig.tight_layout()
bottleneck_fig.savefig("bottleneck.pdf", format="pdf")