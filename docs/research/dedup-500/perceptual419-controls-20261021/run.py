"""Independent scalar characterization of source formulas; NOT Lisp execution."""
def thumbnail(height,width,value,size):
 sums=[[0]*size for _ in range(size)];counts=[[0]*size for _ in range(size)]
 for y in range(height):
  for x in range(width):
   oy=y*size//height;ox=x*size//width
   sums[oy][ox]+=value;counts[oy][ox]+=1
 return [[sums[y][x]//max(1,counts[y][x]) for x in range(size)] for y in range(size)]
def ahash(a):
 flat=sum(a,[]);mean=sum(flat)/len(flat)
 return [int(v>=mean) for v in flat]
def dhash(a):
 return [int(2*a[y][x]>=a[y][x+1]+a[y+1][x]) for y in range(32) for x in range(32)]
small=thumbnail(1,1,255,32);large=thumbnail(32,32,255,32)
assert sum(v!=0 for row in small for v in row)==1
assert sum(ahash(small))==1 and sum(ahash(large))==1024
assert sum(a!=b for a,b in zip(ahash(small),ahash(large)))==1023
print('PASS modeled solid white 1x1 versus 32x32 aHash differs in 1023/1024 bits')
for h,w in [(32,32),(33,35),(64,96)]:
 assert all(v==137 for row in thumbnail(h,w,137,32) for v in row)
print('PASS modeled constant images at or above target dimensions preserve intensity')
assert dhash(thumbnail(33,33,0,33))==dhash(thumbnail(33,33,255,33))==[1]*1024
assert ahash(thumbnail(32,32,0,32))==ahash(large)==[1]*1024
print('PASS modeled flat black and white share aHash and dHash; signatures cannot prove identity')
print('3 independent formula controls passed; original Lisp/Imago/decoder/upstream suite NOT executed')
