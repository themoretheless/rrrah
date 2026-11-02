import csv, hashlib, json
from pathlib import Path

ROOT = Path(__file__).resolve().parent
OUTPUT = ROOT / "cross-scene-white-anchor-20261102.json"

def load(scene):
    path = ROOT / f"{scene}-display-pair-samples-20261026.csv"
    raw = path.read_bytes()
    rows = []
    with path.open() as stream:
        for row in csv.DictReader(stream):
            x, y = int(row["x"]), int(row["y"])
            a = [float(row[f"source_{c}"]) for c in "rgb"]
            b = [float(row[f"target_{c}"]) for c in "rgb"]
            rows.append((x, y, a, b))
    return rows, {"path": str(path), "sha256": hashlib.sha256(raw).hexdigest()}

def gains(rows, reverse):
    pairs = [(b, a) if reverse else (a, b) for x,y,a,b in rows
             if (x // 32 + y // 32) % 2 == 0
             and all(0 <= v <= 1 for v in a+b)]
    result = []
    for c in range(3):
        numerator = sum((a[c]-1)*(b[c]-1) for a,b in pairs)
        denominator = sum((a[c]-1)**2 for a,b in pairs)
        assert denominator > 0
        result.append(min(1.1, max(0.9, numerator/denominator)))
    return result, len(pairs)

def agreement(rows, model, reverse):
    passed = total = 0
    for x,y,a,b in rows:
        if (x // 32 + y // 32) % 2 == 0:
            continue
        if reverse: a,b = b,a
        errors = [abs(min(1,max(0,g*a[c]+1-g))-min(1,max(0,b[c])))
                  for c,g in enumerate(model)]
        passed += max(errors) <= 0.03
        total += 1
    assert total > 0
    return {"matched": passed, "compared": total, "agreement": passed/total}

data = {}; inputs = []
for scene in ["2414", "2418"]:
    data[scene], info = load(scene); inputs.append(info)
assert [(r[0],r[1]) for r in data["2414"]] == [(r[0],r[1]) for r in data["2418"]]
rows = []
for training in data:
    for reverse in [False,True]:
        model, count = gains(data[training], reverse)
        for evaluation in data:
            rows.append({"training_scene":training,"evaluation_scene":evaluation,
                         "direction":"reverse" if reverse else "forward",
                         "gains":model,"training_points":count,
                         "result":agreement(data[evaluation],model,reverse)})
        negative = [(x,y,a,other[3]) for (x,y,a,b),other in zip(data["2414"],data["2418"])]
        rows.append({"training_scene":training,"evaluation_scene":"2414-raw-vs-2418-render",
                     "direction":"reverse" if reverse else "forward",
                     "gains":model,"training_points":count,
                     "result":agreement(negative,model,reverse)})
report = {"inputs":inputs,"results":rows,"tolerance":0.03,"gain_bounds":[0.9,1.1],
          "scope":"Diagnostic on retained sparse aligned CSV samples. Fit even checkerboard in-range points; evaluate odd points including all range classes. Cross-scene negative is authored correspondence, not automatic geometry. No new decoding, native acceptance, threshold or recipe change; two scenes do not establish broad precision/recall."}
with OUTPUT.open("x") as stream: json.dump(report,stream,indent=2);stream.write("\n")
for row in rows: print(row["training_scene"],row["evaluation_scene"],row["direction"],row["result"])
