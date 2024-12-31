import pandas as pd
import matplotlib.pyplot as plt

plt.rcParams["font.family"] = "GillSansC"
plt.rcParams["font.size"] = "12"

df = pd.read_csv("tput_timeline.csv")

normal_color = "cornflowerblue"
unlimited_color = "crimson"
unlimited_no_timeout_color = "darkorange"

fig, ax = plt.subplots(figsize=(4.2, 2.4))
ax.plot(df["time"], df["40_inflight"] / 1000, "o-", color=normal_color, label="40 Inflight")
ax.plot(df["time"], df["unlimited_inflight"] / 1000, "s-", color=unlimited_color, label="Unlimited Inflight")
ax.plot(df["time"], df["unlimited_inflight_no_timeout"] / 1000, "^-", color=unlimited_no_timeout_color, label="Unlimited Inflight (No Timeout)")
ax.set_ylim((0, 9))
ax.legend(loc="upper right")
ax.set_xlabel("Time (s)", labelpad=0)
ax.set_ylabel("Throughput (ktps.)", labelpad=1)
ax.set_xticks(df["time"])

ax.tick_params(axis='x', length=2, pad=2)
ax.tick_params(axis='y', length=2, pad=2)

plt.subplots_adjust(left=0.075, right=0.99, top=0.99, bottom=0.15)
fig.savefig("plot.pdf", format="pdf")