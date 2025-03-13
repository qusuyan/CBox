import pandas as pd
import matplotlib.pyplot as plt

plt.rcParams["font.family"] = "Gill Sans"
plt.rcParams["font.size"] = "12"

data = pd.read_csv("agg_results.csv", header=[0,1,2])
data.columns = [c for (_, _, c) in data.columns[:2]] + [(a, b) for (a, b, _) in data.columns[2:]]

avax_df = data.loc[data["chain-type"] == "avalanche"]
btc_df = data.loc[data["chain-type"] == "bitcoin"]
diem_df = data.loc[data["chain-type"] == "diem"]

avax_color = "mediumpurple"
btc_color = "cornflowerblue"
diem_color = "orange"

fig, ax = plt.subplots(figsize=(3, 2.4))

# plot tput
ax.plot(avax_df["num-nodes"], avax_df[("avg_tput", "mean")] / 1000, "o-", color=avax_color, label="Avalanche")
ax.plot(btc_df["num-nodes"], btc_df[("avg_tput", "mean")] / 1000, "s-", color=btc_color, label="Bitcoin")
ax.plot(diem_df["num-nodes"], diem_df[("avg_tput", "mean")] / 1000, "^-", color=diem_color, label="Diem")
ax.set_ylabel('Tput (ktps.)', labelpad=-1)
ax.set_ylim((0, 16))
ax.set_yticks(range(0, 20, 5))

ax.set_xlabel('# Validators', labelpad=-1)
ax.set_xticks(data["num-nodes"])
ax.tick_params(axis='x', labelrotation=0, length=2, pad=2)

ax.legend()

plt.subplots_adjust(left=0.14, right=0.99, top=0.99, bottom=0.14)
# fig.tight_layout()
fig.savefig("plot.pdf", format="pdf")