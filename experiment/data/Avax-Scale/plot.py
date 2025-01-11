import pandas as pd
import matplotlib.pyplot as plt

plt.rcParams["font.family"] = "GillSansC"
plt.rcParams["font.size"] = "12"

data = pd.read_csv("agg_results.csv", header=[0,1,2])
data.columns = [c for (_, _, c) in data.columns[:2]] + [(a, b) for (a, b, _) in data.columns[2:]]

ecdsa_df = data.loc[data["crypto"] == "dummy-ecdsa"]
dummy_df = data.loc[data["crypto"] == "dummy"]

ecdsa_color = "cornflowerblue"
ecdsa_color_dark = "royalblue"
dummy_color = "mediumpurple"
dummy_color_dark = "rebeccapurple"

fig, (tput_ax, sched_ax, msg_ax, cpu_ax) = plt.subplots(4, 1, sharex = True, figsize=(3.5, 5))
bar_width = 1.5

# plot tput
tput_ax.bar(ecdsa_df["num-nodes"]-bar_width/2, ecdsa_df[("avg_tput", "mean")] / 1000, bar_width, color=ecdsa_color)
tput_ax.bar(dummy_df["num-nodes"]+bar_width/2, dummy_df[("avg_tput", "mean")] / 1000, bar_width, color=dummy_color)
tput_ax.set_ylabel('Tput (ktps.)', labelpad=2)
tput_ax.set_ylim((0, 16))
tput_ax.set_yticks(range(0, 20, 5))

# plot sched delay
sched_ax.axvline(x = 12.5, color=dummy_color_dark, linestyle="--", linewidth=0.7)
sched_ax.axvline(x = 32.5, color=ecdsa_color_dark, linestyle="--", linewidth=0.7)
sched_ax.plot(ecdsa_df["num-nodes"], ecdsa_df[("sched_dur_ms", "mean")], 'o-', color=ecdsa_color, label="Signature")
sched_ax.plot(dummy_df["num-nodes"], dummy_df[("sched_dur_ms", "mean")], 's-', color=dummy_color, label="No Signature")
sched_ax.set_ylabel('Sched Delay (ms)', labelpad=8)
sched_ax.set_ylim((0, 2.5))
sched_ax.legend()

# plot msg delay
msg_ax.axvline(x = 12.5, color=dummy_color_dark, linestyle="--", linewidth=0.7)
msg_ax.axvline(x = 32.5, color=ecdsa_color_dark, linestyle="--", linewidth=0.7)
msg_delay_ax = msg_ax.twinx()
msg_delay_ax.bar(ecdsa_df["num-nodes"]-bar_width/2, ecdsa_df[("deliver_late_dur_ms", "mean")], bar_width, color=ecdsa_color, alpha=0.35)
msg_delay_ax.bar(dummy_df["num-nodes"]+bar_width/2, dummy_df[("deliver_late_dur_ms", "mean")], bar_width, color=dummy_color, alpha=0.35)
msg_delay_ax.set_ylabel("Msg Late Time (ms)", labelpad=0)
msg_delay_ax.set_ylim((0, 300))
msg_ax.plot(ecdsa_df["num-nodes"], ecdsa_df[("deliver_late_chance", "mean")]*100, 'o-', color=ecdsa_color)
msg_ax.plot(dummy_df["num-nodes"], dummy_df[("deliver_late_chance", "mean")]*100, 's-', color=dummy_color)
msg_ax.set_ylabel('Msg Late (%)', labelpad=-0.2)
msg_ax.set_ylim((0, 0.3))

# plot cpu util
cpu_ax.axvline(x = 12.5, color=dummy_color_dark, linestyle="--", linewidth=0.7)
cpu_ax.axvline(x = 32.5, color=ecdsa_color_dark, linestyle="--", linewidth=0.7)
cpu_ax.plot(ecdsa_df["num-nodes"], ecdsa_df[("avg_cpu", "mean")], 'o-', color=ecdsa_color)
cpu_ax.plot(dummy_df["num-nodes"], dummy_df[("avg_cpu", "mean")], 's-', color=dummy_color)
cpu_ax.set_ylabel('CPU (%)', labelpad=-4)
cpu_ax.set_ylim((0, 100))

cpu_ax.set_xlabel('# Validators', labelpad=-1)
cpu_ax.set_xticks(ecdsa_df["num-nodes"])
cpu_ax.tick_params(axis='x', labelrotation=0, length=2, pad=2)

plt.subplots_adjust(left=0.13, right=0.855, top=0.995, bottom=0.065, wspace=0, hspace=0.14)
# fig.tight_layout()
fig.savefig("plot.pdf", format="pdf")