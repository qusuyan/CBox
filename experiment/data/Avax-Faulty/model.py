import math
import numpy as np
import pandas as pd
import matplotlib.pyplot as plt

pd.options.display.float_format = '{:.2f}'.format

num_nodes = 40
k = 10
alpha = 0.8

num_faulty = range(0, 21, 2)
df = pd.DataFrame([[n] for n in num_faulty], columns=["f"])
def success_chance(num_faulty): 
    num_correct = num_nodes - num_faulty
    success_faulty_num = k - math.ceil(k * alpha)
    success_combs = 0
    for i in range(0, success_faulty_num + 1):
        success_combs += math.comb(num_faulty, i) * math.comb(num_correct-1, k-1-i)
    return float(success_combs) / math.comb(num_nodes-1, k-1)

beta = 11

df["p"] = df["f"].apply(success_chance)
df["p^beta"] = df["p"] ** beta
df["avax_commit_rounds"] = np.where(df["p"] < 1, (1-df["p^beta"]) / (1-df["p"]) / df["p^beta"], beta)
df["bliz_commit_rounds"] = 11 / df["p"]

sigma_over_a = 1000
c_over_a = 2400 * (beta + sigma_over_a)
df["avax_tput"] = c_over_a / (df["avax_commit_rounds"] + sigma_over_a)
df["bliz_tput"] = c_over_a / (df["bliz_commit_rounds"] + sigma_over_a)

print(df)

avax_color = "cornflowerblue"
bliz_color = "crimson"

fig, ax = plt.subplots(figsize=(4.2, 2.4))
ax.plot(df["f"], df["bliz_tput"] / 1000, "s-", color=bliz_color, label="Blizzard")
ax.plot(df["f"], df["avax_tput"] / 1000, "o-", color=avax_color, label="Avalanche")
ax.set_ylim((0, 3.6))
ax.legend()
ax.set_xlabel("# Faulty Validators", labelpad=0)
ax.set_ylabel("Throughput (ktps.)", labelpad=0)
ax.set_xticks(df["f"])

ax.tick_params(axis='x', length=2, pad=2)
ax.tick_params(axis='y', length=2, pad=2)

plt.subplots_adjust(left=0.1, right=0.99, top=0.99, bottom=0.15)
fig.savefig("model.pdf", format="pdf")