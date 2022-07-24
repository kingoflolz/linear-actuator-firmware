import matplotlib.pyplot as plt
import numpy as np

x = np.load("save.npy")
x = x.reshape(-1, 16)

plt.plot(x, label=[str(i) for i in range(16)])
plt.legend()
plt.show()

plt.plot(x[1600:1700])
plt.show()

print(x.mean(axis=0))