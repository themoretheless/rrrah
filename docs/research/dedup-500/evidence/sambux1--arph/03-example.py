import arph

p = arph.Perceptographic('phash', 'log_threshold', 256, 200, 1)
img = arph.underlying.Image('/home/sam/Downloads/carina-nebula.jpg')
h = p.hash(img)
print(h)

import numpy as np
x = np.random.randint(0, 2, size=(20))

'''
pph = arph.pph.LogThreshold(20, 2)
pph.save_description()
print(pph.hash(x))
'''

pph2 = arph.pph.LogThreshold()
print(pph2.hash(x))
