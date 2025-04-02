import pandas as pd
import matplotlib.pyplot as plt

plt.rcParams["font.family"] = "Gill Sans"
plt.rcParams["font.size"] = "12"

cbox_df = pd.read_csv("agg_results.csv", header=[0,1,2])
cbox_df.columns = [cbox_df.columns[0][2]] + [(a, b) for (a, b, _) in cbox_df.columns[1:]]

cbox_color = "cornflowerblue"
cbox_color_dark = "royalblue"

fig, (tput_ax, sched_ax, msg_ax, cpu_ax) = plt.subplots(4, 1, sharex = True, figsize=(3.5, 5))
bar_width = 4

# plot tput
# tput_ax.axvline(x = 210, color=cbox_color_dark, linestyle="--", linewidth=0.7)
tput_ax.plot(cbox_df["num-nodes"], cbox_df[("avg_tput", "mean")] / 1000, 'o-', color=cbox_color, label = "CBox")
tput_ax.set_ylabel('Tput (ktps.)', labelpad=8)
tput_ax.set_ylim((0, 25))
# tput_ax.legend(loc=(0.5, 0.05))

# plot sched delay
# sched_ax.axvline(x = 210, color=cbox_color_dark, linestyle="--", linewidth=0.7)
sched_ax.plot(cbox_df["num-nodes"], cbox_df[("sched_dur_ms", "mean")], 'o-', color=cbox_color, label="eCore=1")
sched_ax.set_ylabel('Sched Delay (ms)', labelpad=-1)
sched_ax.set_ylim((0, 1))

# plot msg delay
# msg_ax.axvline(x = 210, color=cbox_color_dark, linestyle="--", linewidth=0.7)
msg_delay_ax = msg_ax.twinx()
msg_delay_ax.bar(cbox_df["num-nodes"], cbox_df[("deliver_late_dur_ms", "mean")], bar_width, color=cbox_color, alpha=0.35, zorder=0)
msg_delay_ax.set_ylabel("Msg Late Time (ms)", labelpad=0)
msg_delay_ax.set_ylim((0, 300))
msg_ax.plot(cbox_df["num-nodes"], cbox_df[("deliver_late_chance", "mean")]*100, 'o-', color=cbox_color, zorder=10)
msg_ax.set_ylabel('Msg Late (%)', labelpad=-0.5)
msg_ax.set_ylim((0, 0.25))

# plot cpu util
# cpu_ax.axvline(x = 210, color=cbox_color_dark, linestyle="--", linewidth=0.7)
cpu_ax.plot(cbox_df["num-nodes"], cbox_df[("avg_cpu", "mean")], 'o-', color=cbox_color)
cpu_ax.set_ylabel('CPU (%)', labelpad=-4)
cpu_ax.set_ylim((0, 100))

cpu_ax.set_xticks(cbox_df["num-nodes"].iloc[::2])
cpu_ax.tick_params(axis='x', labelrotation=0, length=2, pad=2)
cpu_ax.set_xlabel('# Validators', labelpad=-1)
# cpu_ax.set_xlim((30, 310))

plt.subplots_adjust(left=0.13, right=0.855, top=0.995, bottom=0.065, wspace=0, hspace=0.14)
# fig.tight_layout()
fig.savefig("plot.pdf", format="pdf")