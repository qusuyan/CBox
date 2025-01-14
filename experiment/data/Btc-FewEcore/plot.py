import pandas as pd
import matplotlib.pyplot as plt

plt.rcParams["font.family"] = "GillSansC"
plt.rcParams["font.size"] = "12"

sim_df = pd.read_csv("simulation.csv")
sim_df["avg_tput"] = sim_df["avg-chain-length"] * 1957 / 900
sim_adjusted_df = pd.read_csv("simulation_adjusted.csv")
sim_adjusted_df["avg_tput"] = sim_adjusted_df["avg-chain-length"] * 1957 / 900
cbox_df = pd.read_csv("agg_results.csv", header=[0,1,2])
cbox_df.columns = [cbox_df.columns[0][2]] + [(a, b) for (a, b, _) in cbox_df.columns[1:]]
cbox_df = cbox_df.iloc[1:]

sim_color = "black"
sim_adjusted_color = "grey"
cbox_color = "cornflowerblue"
cbox_color_dark = "royalblue"

fig, (tput_ax, sched_ax, msg_ax, cpu_ax) = plt.subplots(4, 1, sharex = True, figsize=(3.5, 5))
bar_width = 8

# plot tput
tput_ax.plot(sim_df["num-nodes"], sim_df["avg_tput"], 'o-', color=sim_color, label = "BlockSim")
tput_ax.plot(sim_adjusted_df["num-nodes"], sim_adjusted_df["avg_tput"], 'o-', color=sim_adjusted_color, label = "BlockSim Adjusted")
tput_ax.plot(cbox_df["num-nodes"], cbox_df[("avg_tput", "mean")], 's-', color=cbox_color, label = "CBox")
tput_ax.set_ylabel('Tput (ktps.)', labelpad=-4)
tput_ax.set_ylim((0, 110))
tput_ax.legend(loc=(0.25, -0.91))
tput_ax.set_zorder(999)

# plot sched delay
sched_ax.plot(cbox_df["num-nodes"], cbox_df[("sched_dur_ms", "mean")], 'o-', color=cbox_color, label="eCore=1")
sched_ax.set_ylabel('Sched Delay (ms)', labelpad=7)
sched_ax.set_ylim((0, 8))


# plot msg delay
msg_delay_ax = msg_ax.twinx()
msg_delay_ax.bar(cbox_df["num-nodes"], cbox_df[("deliver_late_dur_ms", "mean")], bar_width, color=cbox_color, alpha=0.35, zorder=0)
msg_delay_ax.set_ylabel("Msg Late Time (ms)", labelpad=0)
msg_delay_ax.set_ylim((0, 300))
msg_ax.plot(cbox_df["num-nodes"], cbox_df[("deliver_late_chance", "mean")]*100, 'o-', color=cbox_color, zorder=10)
msg_ax.set_ylabel('Msg Late (%)', labelpad=0)
msg_ax.set_ylim((0, 0.299))

# plot cpu util
cpu_ax.plot(cbox_df["num-nodes"], cbox_df[("avg_cpu", "mean")], 'o-', color=cbox_color)
cpu_ax.set_ylabel('CPU (%)', labelpad=-5)
cpu_ax.set_ylim((0, 100))

cpu_ax.set_xticks(cbox_df["num-nodes"])
cpu_ax.tick_params(axis='x', labelrotation=0, length=2, pad=2)
cpu_ax.set_xlabel('# Validators', labelpad=-1)
cpu_ax.set_xlim((35, 165))

plt.subplots_adjust(left=0.13, right=0.855, top=0.995, bottom=0.065, wspace=0, hspace=0.14)
# fig.tight_layout()
fig.savefig("plot.pdf", format="pdf")