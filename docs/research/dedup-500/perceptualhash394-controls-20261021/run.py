from pathlib import Path
import re, math, hashlib
root=Path('.dedup-work/perceptualhash394-20261021')
rows=[list(map(int,line.split())) for line in (root/'README.md').read_text().splitlines() if re.fullmatch('[01](?: [01]){63}',line)]
assert len(rows)==3
assert rows[0]==rows[1] and rows[0]!=rows[2]
a=(root/'demo-data/frog.jpeg').read_bytes();b=(root/'demo-data/frog.png').read_bytes()
assert a!=b and hashlib.sha256(a).digest()!=hashlib.sha256(b).digest()
print('PASS documented frog vectors identical for byte-distinct encodings; cat vector differs by',sum(x!=y for x,y in zip(rows[0],rows[2])),'bits (README consistency only, not J execution)')
# Independent scalar evaluation of the coefficient formula, not an execution/translation of J semantics.
n=32
c=[[math.sqrt(2/n)*math.cos((2*x+1)*y*math.pi/(2*n)) for x in range(n)] for y in range(n)]
assert math.isclose(sum(v*v for v in c[0]),2,abs_tol=1e-12)
assert all(math.isclose(sum(v*v for v in row),1,abs_tol=1e-12) for row in c[1:])
assert max(abs(sum(a*b for a,b in zip(c[i],c[j]))) for i in range(n) for j in range(i))<1e-12
print('PASS scalar coefficient formula: orthogonal rows, DC squared norm 2, AC squared norm 1; differing DC normalization is not alone proof of a hash defect')
print('RESULT 2 independent consistency/math controls passed; original J runtime not executed')
