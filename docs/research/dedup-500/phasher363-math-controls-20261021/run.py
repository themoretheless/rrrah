import math
def source_compare(a,b):return 100*sum(x==y for x,y in zip(a,b))/len(a)
assert source_compare('0','1')==0
bit_similarity=100*(4-(int('0',16)^int('1',16)).bit_count())/4
assert bit_similarity==75
print('PASS independent comparison counterexample:hexcharacter0% versusbit75% for0/1')
h=8;y=0
assert h-y==8 and not 0<=h-y<h
assert 0<=h-1-y<h
print('PASS independent verticalmirror counterexample:y0 maps8 outside0..7')
def source_dct(v,k):
 n=len(v);s=sum(v[k]*math.cos(k*math.pi*(2*i+1)/n) for i in range(n))*math.sqrt(2/n)
 return s/math.sqrt(2) if k==0 else s
a=[1,0,0,0];b=[1,10,10,10]
assert source_dct(a,0)==source_dct(b,0)
assert sum(a)/math.sqrt(4)!=sum(b)/math.sqrt(4)
print('PASS independent DC counterexample:samefirstsample gives sameDC despite different sum')
print('TOTAL3independent mathematical source-formula counterexamples;NOT PHP/GD or originalfunction execution')
