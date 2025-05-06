import pandas as pd
import matplotlib.pyplot as plt

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
data.columns = [c for (_, _, c) in data.columns[:3]] + [(a, b) for (a, b, _) in data.columns[3:]]

# data = data.loc[data["correct-config"] == "blk_len=40+max_inflight_blk=40"]
data = data.loc[data["correct-config"] == "blk_len=40+max_inflight_blk=2147483647"]
ecdsa_df = data.loc[data["crypto"] == "dummy-ecdsa"]
dummy_df = data.loc[data["crypto"] == "dummy"]

# pmaker_data = data.loc[data["correct-config"] == "blk_len=40+max_inflight_blk=40"]
# dummy_pmaker_df = pmaker_data.loc[pmaker_data["crypto"] == "dummy"]

ecdsa_color = "cornflowerblue"
dummy_color = "mediumpurple"
# dummy_pmaker_color = "mediumpurple"

fig, ((tput_ax, msg_ax), (sched_ax, cpu_ax)) = plt.subplots(2, 2, sharex = True, figsize=(3.5, 2.2))
bar_width = 3.6

# plot tput
tput_ax.bar(ecdsa_df["num-nodes"]-bar_width/2, ecdsa_df[("avg_tput", "mean")] / 1000, bar_width, color=ecdsa_color)
tput_ax.bar(dummy_df["num-nodes"]+bar_width/2, dummy_df[("avg_tput", "mean")] / 1000, bar_width, color=dummy_color)
# tput_ax.bar(dummy_pmaker_df["num-nodes"]+bar_width, dummy_pmaker_df[("avg_tput", "mean")] / 1000, bar_width, color=dummy_pmaker_color)
tput_ax.set_ylabel('Tput (ktps.)', labelpad=-0.5)
tput_ax.set_ylim((0, 15))
tput_ax.set_yticks(range(0, 20, 5))

# plot sched delay
sched_ax.plot(ecdsa_df["num-nodes"], ecdsa_df[("sched_dur_ms", "mean")], 'o-', color=ecdsa_color, zorder=2)
sched_ax.plot(dummy_df["num-nodes"], dummy_df[("sched_dur_ms", "mean")], 's-', color=dummy_color, zorder=0)
# sched_ax.plot(dummy_pmaker_df["num-nodes"], dummy_pmaker_df[("sched_dur_ms", "mean")], '^-', color=dummy_pmaker_color, zorder=1)
sched_ax.set_ylabel('Sched Delay (ms)', labelpad=3.5)
sched_ax.set_ylim((0, 2))

sched_ax.set_xlabel('# Validators', labelpad=0)

# plot msg delay
msg_delay_ax = msg_ax.twinx()
msg_delay_ax.bar(ecdsa_df["num-nodes"]-bar_width/2, ecdsa_df[("deliver_late_dur_ms", "mean")], bar_width, color=ecdsa_color, alpha=0.35)
msg_delay_ax.bar(dummy_df["num-nodes"]+bar_width/2, dummy_df[("deliver_late_dur_ms", "mean")], bar_width, color=dummy_color, alpha=0.35)
# msg_delay_ax.bar(dummy_pmaker_df["num-nodes"]+bar_width, dummy_pmaker_df[("deliver_late_dur_ms", "mean")], bar_width, color=dummy_pmaker_color, alpha=0.35)
msg_delay_ax.set_ylabel("Msg Late Time (ms)", labelpad=0)
msg_delay_ax.set_ylim((0, 80))
msg_ax.plot(ecdsa_df["num-nodes"], ecdsa_df[("deliver_late_chance", "mean")]*100, 'o-', color=ecdsa_color, label="Signature", zorder=2)
msg_ax.plot(dummy_df["num-nodes"], dummy_df[("deliver_late_chance", "mean")]*100, 's-', color=dummy_color, label="No Signature", zorder=0)
# msg_ax.plot(dummy_pmaker_df["num-nodes"], dummy_pmaker_df[("deliver_late_chance", "mean")]*100, '^-', color=dummy_pmaker_color, label="No Signature (w/ Pacemaker)", zorder=1)
msg_delay_ax.set_zorder(5)
msg_ax.set_zorder(10)
msg_ax.patch.set_visible(False)
msg_ax.set_ylabel('Msg Late (%)', labelpad=-0.2)
msg_ax.set_ylim((0, 10))

msg_ax.legend(ncol=3, loc=(-0.75, 1))

# plot cpu util
cpu_ax.plot(ecdsa_df["num-nodes"], ecdsa_df[("avg_cpu", "mean")], 'o-', color=ecdsa_color, zorder=2)
cpu_ax.plot(dummy_df["num-nodes"], dummy_df[("avg_cpu", "mean")], 's-', color=dummy_color, zorder=0)
# cpu_ax.plot(dummy_pmaker_df["num-nodes"], dummy_pmaker_df[("avg_cpu", "mean")], '^-', color=dummy_pmaker_color, zorder=1)
cpu_ax.set_ylabel('CPU (%)', labelpad=-4)
cpu_ax.set_ylim((0, 100))

cpu_ax.set_xlabel('# Validators', labelpad=0)
cpu_ax.set_xticks(ecdsa_df["num-nodes"])

plt.subplots_adjust(left=0.075, right=0.92, top=0.93, bottom=0.115, wspace=0.24, hspace=0.13)
# fig.tight_layout()
fig.savefig("plot.pdf", format="pdf")