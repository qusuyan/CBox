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

sim_df = pd.read_csv("simulation.csv")
sim_df["avg_tput"] = sim_df["avg-chain-length"] * 1796 / 900
sim_adjusted_df = pd.read_csv("simulation_adjusted.csv")
sim_adjusted_df["avg_tput"] = sim_adjusted_df["avg-chain-length"] * 1796 / 900
cbox_df = pd.read_csv("agg_results.csv", header=[0,1,2])
cbox_df.columns = [cbox_df.columns[0][2]] + [(a, b) for (a, b, _) in cbox_df.columns[1:]]

sim_color = "black"
sim_adjusted_color = "grey"
cbox_color = "crimson"

fig, ((tput_ax, msg_ax), (sched_ax, cpu_ax)) = plt.subplots(2, 2, sharex = True, figsize=(3.5, 2.2))
bar_width = 8

# plot tput
tput_ax.plot(sim_df["num-nodes"], sim_df["avg_tput"], 'o-', color=sim_color, label = "BlockSim")
tput_ax.plot(sim_adjusted_df["num-nodes"], sim_adjusted_df["avg_tput"], 'o-', color=sim_adjusted_color, label = "BlockSim Adjusted")
tput_ax.plot(cbox_df["num-nodes"], cbox_df[("avg_tput", "mean")], 's-', color=cbox_color, label = "CBox")
tput_ax.set_ylabel('Tput (tps.)', labelpad=-4)
tput_ax.set_ylim((0, 110))
tput_ax.legend(ncol=3, loc=(0.15, 1))
tput_ax.set_zorder(999)

# plot sched delay
sched_ax.plot(cbox_df["num-nodes"], cbox_df[("sched_dur_ms", "mean")], 'o-', color=cbox_color, label="eCore=1")
sched_ax.set_ylabel('Sched Delay (ms)', labelpad=4)
sched_ax.set_ylim((0, 3))
sched_ax.set_xlabel('# Validators', labelpad=0)

# plot msg delay
msg_delay_ax = msg_ax.twinx()
msg_delay_ax.bar(cbox_df["num-nodes"], cbox_df[("deliver_late_dur_ms", "mean")], bar_width, color=cbox_color, alpha=0.35, zorder=0)
msg_delay_ax.set_ylabel("Msg Late Time (ms)", labelpad=0)
msg_delay_ax.set_ylim((0, 80))
msg_ax.plot(cbox_df["num-nodes"], cbox_df[("deliver_late_chance", "mean")]*100, 'o-', color=cbox_color, zorder=10)
msg_ax.set_ylabel('Msg Late (%)', labelpad=0)
msg_ax.set_ylim((0, 20))

# plot cpu util
cpu_ax.plot(cbox_df["num-nodes"], cbox_df[("avg_cpu", "mean")], 'o-', color=cbox_color)
cpu_ax.set_ylabel('CPU (%)', labelpad=-5)
cpu_ax.set_ylim((0, 100))

cpu_ax.set_xticks(cbox_df["num-nodes"])
cpu_ax.tick_params(axis='x', labelrotation=0, length=2, pad=2)
cpu_ax.set_xlabel('# Validators', labelpad=0)
cpu_ax.set_xticks(range(40, 170, 20))

plt.subplots_adjust(left=0.08, right=0.92, top=0.93, bottom=0.115, wspace=0.24, hspace=0.13)
# fig.tight_layout()
fig.savefig("plot.pdf", format="pdf")

# plot eCore utilization
util_fig, (util_ax) = plt.subplots(1, 1, figsize=(3.33, 1.0))

util_ax.plot(cbox_df["num-nodes"], cbox_df[("ecore_util", "mean")], 'o-', color=cbox_color)
util_ax.set_xlabel('# Validators', labelpad=0)
util_ax.set_ylabel('eCore', labelpad=2)
util_ax.set_ylim(0.1, 0.32)
util_ax.set_yticks([0.1, 0.2, 0.3])

plt.subplots_adjust(left=0.1, right=0.995, top=0.99, bottom=0.25, wspace=0, hspace=0)
util_fig.savefig("ecore.pdf", format="pdf")