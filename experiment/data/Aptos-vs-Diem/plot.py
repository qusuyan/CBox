import pandas as pd
import matplotlib.pyplot as plt

plt.rcParams["font.family"] = "Gill Sans"
plt.rcParams["font.size"] = "12"

data = pd.read_csv("agg_results.csv", header=[0,1,2])
data.columns = [c for (_, _, c) in data.columns[:3]] + [(a, b) for (a, b, _) in data.columns[3:]]

diem_df = data.loc[data["chain-type"] == "diem"]
aptos_df = data.loc[data["chain-type"] == "aptos"]
aptos_1_df = aptos_df.loc[data["correct-config"] == "narwhal_blk_size=1048576+diem_blk_len=1"]
aptos_4_df = aptos_df.loc[data["correct-config"] == "narwhal_blk_size=262144+diem_blk_len=4"]
aptos_16_df = aptos_df.loc[data["correct-config"] == "narwhal_blk_size=65536+diem_blk_len=16"]

diem_color = "mediumpurple"
aptos_1_color = "cornflowerblue"
aptos_4_color = "orange"
aptos_16_color = "crimson"

fig, ax = plt.subplots(figsize=(3, 2.5))

# plot tput
ax.plot(diem_df["num-nodes"], diem_df[("avg_tput", "mean")] / 1000, "o-", color=diem_color, label="Diem")
ax.plot(aptos_1_df["num-nodes"], aptos_1_df[("avg_tput", "mean")] / 1000, "s-", color=aptos_1_color, label="Aptos 1MB")
ax.plot(aptos_4_df["num-nodes"], aptos_4_df[("avg_tput", "mean")] / 1000, "^-", color=aptos_4_color, label="Aptos 256KB")
ax.plot(aptos_16_df["num-nodes"], aptos_16_df[("avg_tput", "mean")] / 1000, "v-", color=aptos_16_color, label="Aptos 64KB")
ax.set_ylabel('Tput (ktps.)', labelpad=-1)
ax.set_ylim((0, 50))

ax.set_xlabel('# Validators', labelpad=-1)
ax.set_xticks(data["num-nodes"])
ax.tick_params(axis='x', labelrotation=0, length=2, pad=2)

ax.legend(ncol = 2, loc = (0.01, 0.99), labelspacing=0.5, columnspacing=0.8, handlelength=1.4, handletextpad=0.3, frameon=False)

plt.subplots_adjust(left=0.14, right=0.99, top=0.81, bottom=0.13)
# fig.tight_layout()
fig.savefig("plot.pdf", format="pdf")