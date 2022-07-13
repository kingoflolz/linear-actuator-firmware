import matplotlib.pyplot as plt
import numpy as np

x = np.load("save.npy")
x = x.reshape(-1, 4)
u = np.unwrap(x, axis=0)
d = u - u.mean(axis=1, keepdims=True)

plt.plot(d)
plt.show()

plt.plot(x[1600:1700])
plt.show()

print(x.mean(axis=0))