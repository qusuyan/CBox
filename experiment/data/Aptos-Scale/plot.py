import pandas as pd
import matplotlib.pyplot as plt

plt.rcParams["font.family"] = "Gill Sans"
plt.rcParams["font.size"] = "12"

cbox_df = pd.read_csv("agg_results.csv", header=[0,1,2])
cbox_df.columns = [c for (_, _, c) in cbox_df.columns[:2]] + [(a, b) for (a, b, _) in cbox_df.columns[2:]]

batch4_df = cbox_df.loc[cbox_df["correct-config"] == "diem_blk_len=4"]
batch16_df = cbox_df.loc[cbox_df["correct-config"] == "diem_blk_len=16"]

batch4_color = "cornflowerblue"
batch4_color_dark = "royalblue"
batch16_color = "orange"
batch16_color_dark = "darkorange"

fig, (tput_ax, sched_ax, msg_ax, cpu_ax) = plt.subplots(4, 1, sharex = True, figsize=(3.5, 5))
bar_width = 3

# plot tput
tput_ax.axvline(x = 20, color="green", linestyle="--", linewidth=0.7)
tput_ax.axvline(x = 36, color="red", linestyle="--", linewidth=0.7)
tput_ax.plot(batch4_df["num-nodes"], batch4_df[("avg_tput", "mean")] / 1000, 'o-', color=batch4_color, label = "4 Batches/Blk")
tput_ax.plot(batch16_df["num-nodes"], batch16_df[("avg_tput", "mean")] / 1000, 's-', color=batch16_color, label = "16 Batches/Blk")
tput_ax.set_ylabel('Tput (ktps.)', labelpad=2.5)
tput_ax.set_ylim((0, 25))
tput_ax.legend()

# plot sched delay
sched_ax.axvline(x = 20, color="green", linestyle="--", linewidth=0.7)
sched_ax.axvline(x = 36, color="red", linestyle="--", linewidth=0.7)
sched_ax.plot(batch4_df["num-nodes"], batch4_df[("sched_dur_ms", "mean")], 'o-', color=batch4_color, label="4 Batches / Blk")
sched_ax.plot(batch16_df["num-nodes"], batch16_df[("sched_dur_ms", "mean")], 's-', color=batch16_color, label="16 Batches / Blks")
sched_ax.set_ylabel('Sched Delay (ms)', labelpad=-1)
sched_ax.set_ylim((0, 1))

# plot msg delay
msg_ax.axvline(x = 20, color="green", linestyle="--", linewidth=0.7)
msg_ax.axvline(x = 36, color="red", linestyle="--", linewidth=0.7)
msg_delay_ax = msg_ax.twinx()
msg_delay_ax.bar(batch4_df["num-nodes"] - bar_width/2, batch4_df[("deliver_late_dur_ms", "mean")], bar_width, color=batch4_color, alpha=0.35, zorder=0)
msg_delay_ax.bar(batch16_df["num-nodes"] + bar_width/2, batch16_df[("deliver_late_dur_ms", "mean")], bar_width, color=batch16_color, alpha=0.35, zorder=0)
msg_delay_ax.set_ylabel("Msg Late Time (ms)", labelpad=0)
msg_delay_ax.set_ylim((0, 300))
msg_ax.plot(batch4_df["num-nodes"], batch4_df[("deliver_late_chance", "mean")]*100, 'o-', color=batch4_color, zorder=10)
msg_ax.plot(batch16_df["num-nodes"], batch16_df[("deliver_late_chance", "mean")]*100, 's-', color=batch16_color, zorder=10)
msg_ax.set_ylabel('Msg Late (%)', labelpad=-0.5)
msg_ax.set_ylim((0, 0.25))

# plot cpu util
cpu_ax.axvline(x = 20, color="green", linestyle="--", linewidth=0.7)
cpu_ax.axvline(x = 36, color="red", linestyle="--", linewidth=0.7)
cpu_ax.plot(batch4_df["num-nodes"], batch4_df[("avg_cpu", "mean")], 'o-', color=batch4_color)
cpu_ax.plot(batch16_df["num-nodes"], batch16_df[("avg_cpu", "mean")], 's-', color=batch16_color)
cpu_ax.set_ylabel('CPU (%)', labelpad=-4)
cpu_ax.set_ylim((0, 100))

cpu_ax.set_xticks(cbox_df["num-nodes"].iloc[::2])
cpu_ax.tick_params(axis='x', labelrotation=0, length=2, pad=2)
cpu_ax.set_xlabel('# Validators', labelpad=-1)
# cpu_ax.set_xlim((30, 310))

plt.subplots_adjust(left=0.13, right=0.855, top=0.995, bottom=0.065, wspace=0, hspace=0.14)
# fig.tight_layout()
fig.savefig("plot.pdf", format="pdf")