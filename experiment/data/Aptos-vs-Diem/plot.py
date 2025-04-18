import numpy as np
import pandas as pd
import matplotlib.pyplot as plt

plt.rcParams["font.family"] = "Gill Sans"
plt.rcParams["font.weight"] = "light"
plt.rcParams['axes.labelweight'] = 'light'
plt.rcParams["font.size"] = "12"

data = pd.read_csv("agg_results.csv", header=[0,1,2])
data.columns = [c for (_, _, c) in data.columns[:3]] + [(a, b) for (a, b, _) in data.columns[3:]]

bottleneck = pd.read_csv("aptos_diem_bottleneck.csv", header=[0])

diem_tput_df = data.loc[data["chain-type"] == "diem"]
aptos_tput_df = data.loc[data["chain-type"] == "aptos"]
aptos_1_tput_df = aptos_tput_df.loc[data["correct-config"] == "narwhal_blk_size=1048576+diem_blk_len=1"]
aptos_4_tput_df = aptos_tput_df.loc[data["correct-config"] == "narwhal_blk_size=262144+diem_blk_len=4"]
aptos_16_tput_df = aptos_tput_df.loc[data["correct-config"] == "narwhal_blk_size=65536+diem_blk_len=16"]

diem_color = "mediumpurple"
aptos_1_color = "cornflowerblue"
aptos_4_color = "orange"
aptos_16_color = "crimson"

fig, (tput_ax, bottleneck_ax) = plt.subplots(1, 2, figsize=(6, 2), gridspec_kw={'width_ratios': [1, 2]})

# plot tput
tput_ax.plot(diem_tput_df["num-nodes"], diem_tput_df[("avg_tput", "mean")] / 1000, "o-", color=diem_color, label="Diem")
tput_ax.plot(aptos_1_tput_df["num-nodes"], aptos_1_tput_df[("avg_tput", "mean")] / 1000, "s-", color=aptos_1_color, label="Aptos 1MB")
tput_ax.plot(aptos_4_tput_df["num-nodes"], aptos_4_tput_df[("avg_tput", "mean")] / 1000, "^-", color=aptos_4_color, label="Aptos 256KB")
tput_ax.plot(aptos_16_tput_df["num-nodes"], aptos_16_tput_df[("avg_tput", "mean")] / 1000, "v-", color=aptos_16_color, label="Aptos 64KB")
tput_ax.set_ylabel('Tput (ktps.)', labelpad=-1)
tput_ax.set_ylim((0, 50))
tput_ax.legend(ncol = 4, loc = (0.2, 0.97), labelspacing=0.5, columnspacing=0.8, handlelength=1.4, handletextpad=0.3, frameon=False)

tput_ax.set_xlabel('# Validators', labelpad=-1)
tput_ax.set_xticks(data["num-nodes"])
tput_ax.tick_params(axis='x', labelrotation=0, length=2, pad=2)

# plot bottlneck
max_width = 0.8
space_between = 0.1
stage_pos = [5, 4, 3, 2, 1, 0]
for (_, record) in bottleneck.iterrows():
    txns = np.array([record["txns-sent"], record["txn-validation"], record["block-management"], record["block-dissemination"], record["decide"], record["commit"]])
    if record["chain-type"] == "diem":
        offset = record["num-nodes"] - 1.5*(max_width + space_between)
        style = "o-"
        color = diem_color
    elif record["chain-type"] == "aptos":
        if record["correct-config"] == "narwhal_blk_size=1048576+diem_blk_len=1":
            offset = record["num-nodes"] - 0.5*(max_width + space_between)
            style = "s-"
            color = aptos_1_color
        elif record["correct-config"] == "narwhal_blk_size=262144+diem_blk_len=4":
            offset = record["num-nodes"] + 0.5*(max_width + space_between)
            style = "^-"
            color = aptos_4_color
        elif record["correct-config"] == "narwhal_blk_size=65536+diem_blk_len=16":
            offset = record["num-nodes"] + 1.5*(max_width + space_between)
            style = "v-"
            color = aptos_16_color
    txns = txns / txns.max() * max_width / 2
    bottleneck_ax.fill_betweenx(stage_pos, offset - txns, offset + txns, color=color, lw=0.5,alpha=0.65, zorder=0)

for y in stage_pos:
    bottleneck_ax.hlines(y, 0, 30, colors='black', linestyles='dashed', linewidth=0.5, alpha=0.35, zorder=1)

bottleneck_ax.yaxis.tick_right()
bottleneck_ax.set_yticks([4.5, 3.5, 2.5, 1.5, 0.5])
bottleneck_ax.set_yticklabels(["Txn Valid", "Blk Mng", "Blk Dissem", "Decide", "Commit"])
bottleneck_ax.tick_params(axis='y', labelrotation=0, length=0, pad=2)
bottleneck_ax.set_xlabel('# Validators', labelpad=-1)
bottleneck_ax.set_xticks(data["num-nodes"])
bottleneck_ax.set_xlim((2, 26))
bottleneck_ax.tick_params(axis='x', labelrotation=0, length=2, pad=2)


plt.subplots_adjust(left=0.07, right=0.865, top=0.885, bottom=0.165, wspace=0.02, hspace=0)
# fig.tight_layout()
fig.savefig("plot.pdf", format="pdf")