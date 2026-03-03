import pandas as pd
import matplotlib.pyplot as plt
import numpy as np

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

tput_data = pd.read_csv("agg_results.csv", header=[0,1,2])
tput_data.columns = [c for (_, _, c) in tput_data.columns[0:2]] + [(a, b) for (a, b, _) in tput_data.columns[2:]]

no_comp_df = tput_data.loc[tput_data["script-runtime"] == 0.000]
comp_df = tput_data.loc[tput_data["script-runtime"] == 0.0002]

no_comp_color = "tab:orange"
comp_color = "tab:brown"

fig, ((tput_ax, msg_ax), (sched_ax, cpu_ax)) = plt.subplots(2, 2, sharex = True, figsize=(3.5, 2.2))
bar_width = 14

# plot tput
# tput_ax.axvline(x = 210, color=cbox_color_dark, linestyle="--", linewidth=0.7)
tput_ax.plot(no_comp_df["num-nodes"], no_comp_df[("avg_tput", "mean")] / 1000, 'o-', color=no_comp_color, zorder=1, label = "Txn Exec Time = 0")
tput_ax.plot(comp_df["num-nodes"], comp_df[("avg_tput", "mean")] / 1000, 's-', color=comp_color, zorder=0, label = "Txn Exec Time = 200μs")
tput_ax.set_ylabel('Tput (ktps.)', labelpad=0)
tput_ax.set_ylim((0, 2))
tput_ax.legend(ncol = 2, loc=(0.15, 1))

# plot sched delay
# sched_ax.axvline(x = 210, color=cbox_color_dark, linestyle="--", linewidth=0.7)
sched_ax.plot(no_comp_df["num-nodes"], no_comp_df[("sched_dur_ms", "mean")], 'o-', color=no_comp_color, zorder=1)
sched_ax.plot(comp_df["num-nodes"], comp_df[("sched_dur_ms", "mean")], 's-', color=comp_color, zorder=0)
sched_ax.set_ylabel('Sched Delay (ms)', labelpad=0)
sched_ax.set_ylim((0, 2))

sched_ax.set_xlabel('# Validators', labelpad=0)

# plot msg delay
# msg_ax.axvline(x = 210, color=cbox_color_dark, linestyle="--", linewidth=0.7)
msg_delay_ax = msg_ax.twinx()
msg_delay_ax.bar(no_comp_df["num-nodes"] - bar_width/2, no_comp_df[("deliver_late_dur_ms", "mean")], bar_width, color=no_comp_color, alpha=0.35, zorder=1)
msg_delay_ax.bar(comp_df["num-nodes"] + bar_width/2, comp_df[("deliver_late_dur_ms", "mean")], bar_width, color=comp_color, alpha=0.35, zorder=0)
msg_delay_ax.set_ylabel("Msg Late Time (ms)", labelpad=0.5)
msg_delay_ax.set_ylim((0, 80))
msg_ax.plot(no_comp_df["num-nodes"], no_comp_df[("deliver_late_chance", "mean")]*100, 'o-', color=no_comp_color, zorder=11)
msg_ax.plot(comp_df["num-nodes"], comp_df[("deliver_late_chance", "mean")]*100, 's-', color=comp_color, zorder=10)
msg_ax.set_ylabel('Msg Late (%)', labelpad=0)
msg_ax.set_ylim((0, 1.2))

# plot cpu util
# cpu_ax.axvline(x = 210, color=cbox_color_dark, linestyle="--", linewidth=0.7)
cpu_ax.plot(no_comp_df["num-nodes"], no_comp_df[("avg_cpu", "mean")], 'o-', color=no_comp_color, zorder=1)
cpu_ax.plot(comp_df["num-nodes"], comp_df[("avg_cpu", "mean")], 'o-', color=comp_color, zorder=0)
cpu_ax.set_ylabel('CPU (%)', labelpad=-2)
cpu_ax.set_ylim((0, 100))

cpu_ax.set_xticks(list(range(0, 257, 64)))
cpu_ax.set_xlabel('# Validators', labelpad=0)
# cpu_ax.set_xlim((30, 310))

plt.subplots_adjust(left=0.062, right=0.92, top=0.93, bottom=0.115, wspace=0.27, hspace=0.13)
# fig.tight_layout()
fig.savefig("plot.pdf", format="pdf")

# plot latency graph
lat_data = pd.read_csv("diem_latency_breakdown.csv")
lat_data = lat_data[lat_data["num-nodes"] >= 32]
no_comp_lat = lat_data[lat_data["script-runtime"] == 0]
comp_lat = lat_data[lat_data["script-runtime"] == 0.0002]

lat_fig, (lat_ax_no_comp, lat_ax_comp) = plt.subplots(1, 2, sharex = True, sharey=True, figsize=(3.5, 1.2))

barwidth = 24
build_block_color = "tab:olive"
propose_block_color = "tab:gray"
validate_block_color = "tab:cyan"
vote_color = "tab:pink"

base = np.zeros(no_comp_lat.shape[0])
lat_ax_no_comp.bar(no_comp_lat["num-nodes"], no_comp_lat["build_time"], barwidth, base, color=build_block_color, label="Leader Build Block")
base += no_comp_lat["build_time"]
lat_ax_no_comp.bar(no_comp_lat["num-nodes"], no_comp_lat["propose_time"], barwidth, base, color=propose_block_color, label="Leader Broadcast Block")
base += no_comp_lat["propose_time"]
lat_ax_no_comp.bar(no_comp_lat["num-nodes"], no_comp_lat["validate_time"], barwidth, base, color=validate_block_color, label="Follower Validate Block")
base += no_comp_lat["validate_time"]
lat_ax_no_comp.bar(no_comp_lat["num-nodes"], no_comp_lat["vote_time"], barwidth, base, color=vote_color, label="Follower Send Vote")
lat_ax_no_comp.legend(ncol=2, loc=(0.1, 0.97), handlelength=0.7)

lat_ax_no_comp.set_xlabel("Txn Exec Time = 0", labelpad=0)
lat_ax_no_comp.tick_params(axis='x', labelrotation=10, pad=0)

base = np.zeros(comp_lat.shape[0])
lat_ax_comp.bar(comp_lat["num-nodes"], comp_lat["build_time"], barwidth, base, color=build_block_color, label="Leader Build Block")
base += comp_lat["build_time"]
lat_ax_comp.bar(comp_lat["num-nodes"], comp_lat["propose_time"], barwidth, base, color=propose_block_color, label="Leader Broadcast Block")
base += comp_lat["propose_time"]
lat_ax_comp.bar(comp_lat["num-nodes"], comp_lat["validate_time"], barwidth, base, color=validate_block_color, label="Follower Validate Block")
base += comp_lat["validate_time"]
lat_ax_comp.bar(comp_lat["num-nodes"], comp_lat["vote_time"], barwidth, base, color=vote_color, label="Follower Send Vote")

lat_ax_comp.set_xlabel("Txn Exec Time = 200μs", labelpad=0)
lat_ax_comp.tick_params(axis='x', labelrotation=10, pad=0)

lat_ax_no_comp.set_yticks(np.arange(0, 1.1, 0.2))
lat_ax_no_comp.set_ylabel("Latency (s)", labelpad=2)
lat_ax_no_comp.set_xticks(range(32, 257, 32))

plt.subplots_adjust(left=0.1, right=0.92, top=0.78, bottom=0.215, wspace=0, hspace=0)

lat_fig.savefig("latency.pdf", format="pdf")