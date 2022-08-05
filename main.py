import matplotlib.pyplot as plt
import numpy as np

x = np.load("data.npy")
x = x.reshape(9, -1).transpose()

plt.plot(x, label=[str(i) for i in range(9)])
plt.legend()
plt.show()

plt.plot(x[1600:1700])
plt.show()

print(x.mean(axis=0))