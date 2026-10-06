import arph

img = arph.underlying.Image('/home/sam/Pictures/carina-nebula.jpg')

# phash only, no PPH
phash = arph.Perceptual('phash', 256)
h1 = phash.hash(img, as_hex=True)
print('phash only:    ', h1)
