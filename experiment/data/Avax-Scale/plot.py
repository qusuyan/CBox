import pandas as pd
import matplotlib.pyplot as plt

plt.rcParams["font.family"] = "Times New Roman"

data = pd.read_csv("agg_results.csv")
ecdsa_df = data.loc[data["crypto"] == "dummy-ecdsa"]
dummy_df = data.loc[data["crypto"] == "dummy"]

ecdsa_color = "cornflowerblue"
ecdsa_color_dark = "royalblue"
dummy_color = "mediumpurple"
dummy_color_dark = "rebeccapurple"

fig, (tput_ax, sched_ax, msg_ax, cpu_ax) = plt.subplots(4, 1, sharex = True, figsize=(4, 6))
bar_width = 4

# plot tput
tput_ax.bar(ecdsa_df["num-nodes"]-bar_width/2, ecdsa_df["avg_tput"] / 1000, bar_width, color=ecdsa_color)
tput_ax.bar(dummy_df["num-nodes"]+bar_width/2, dummy_df["avg_tput"] / 1000, bar_width, color=dummy_color)
tput_ax.set_ylabel('Throughput (ktps.)', labelpad=1)
tput_ax.set_ylim((0, 12))

# plot sched delay
sched_ax.axvline(x = 20, color=dummy_color_dark, linestyle="--", linewidth=0.7)
sched_ax.axvline(x = 50, color=ecdsa_color_dark, linestyle="--", linewidth=0.7)
sched_ax.plot(ecdsa_df["num-nodes"], ecdsa_df["sched_dur_ms"], 'o-', color=ecdsa_color, label="With Signature")
sched_ax.plot(dummy_df["num-nodes"], dummy_df["sched_dur_ms"], 's-', color=dummy_color, label="Without Signature")
sched_ax.set_ylabel('Sched Delay (ms)', labelpad=6)
sched_ax.set_ylim((0, 2.5))
sched_ax.legend()

# plot msg delay
msg_ax.axvline(x = 20, color=dummy_color_dark, linestyle="--", linewidth=0.7)
msg_ax.axvline(x = 50, color=ecdsa_color_dark, linestyle="--", linewidth=0.7)
msg_delay_ax = msg_ax.twinx()
msg_delay_ax.bar(ecdsa_df["num-nodes"]-bar_width/2, ecdsa_df["deliver_late_dur_ms"], bar_width, color=ecdsa_color, alpha=0.35)
msg_delay_ax.bar(dummy_df["num-nodes"]+bar_width/2, dummy_df["deliver_late_dur_ms"], bar_width, color=dummy_color, alpha=0.35)
msg_delay_ax.set_ylabel("Msg Late Time (ms)", labelpad=0)
msg_delay_ax.set_ylim((0, 300))
msg_ax.plot(ecdsa_df["num-nodes"], ecdsa_df["deliver_late_chance"]*100, 'o-', color=ecdsa_color)
msg_ax.plot(dummy_df["num-nodes"], dummy_df["deliver_late_chance"]*100, 's-', color=dummy_color)
msg_ax.set_ylabel('Msg Late (%)', labelpad=-1)
msg_ax.set_ylim((0, 0.3))

# plot cpu util
cpu_ax.axvline(x = 20, color=dummy_color_dark, linestyle="--", linewidth=0.7)
cpu_ax.axvline(x = 50, color=ecdsa_color_dark, linestyle="--", linewidth=0.7)
cpu_ax.plot(ecdsa_df["num-nodes"], ecdsa_df["avg_cpu"], 'o-', color=ecdsa_color)
cpu_ax.plot(dummy_df["num-nodes"], dummy_df["avg_cpu"], 's-', color=dummy_color)
cpu_ax.set_ylabel('CPU (%)', labelpad=-4)
cpu_ax.set_ylim((0, 100))

cpu_ax.set_xlabel('# Validators', labelpad=1)
cpu_ax.set_xticks(ecdsa_df["num-nodes"])

plt.subplots_adjust(left=0.1, right=0.89, top=0.99, bottom=0.06, wspace=0, hspace=0.11)
# fig.tight_layout()
fig.savefig("plot.pdf", format="pdf")