#!/usr/bin/env python3
from pathlib import Path
import hashlib,json,re
R=Path(__file__).resolve().parents[1];D=R/'docs/research';P=D/'dedup-gradient-lanes16-managed-20261014.json';m=json.loads(P.read_text());sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
root=R/'crates/rrrah-dedup/src/gradient.rs';current=root.read_text();baseline=(Path(m['baseline_source'])/'crates/rrrah-dedup/src/gradient.rs').read_text();candidate=(Path(m['source'])/'crates/rrrah-dedup/src/gradient.rs').read_text()
def body(text,name):
    a=text.index('fn '+name);b=text.index('{',a);depth=0
    for i in range(b,len(text)):
        if text[i]=='{':depth+=1
        elif text[i]=='}':
            depth-=1
            if depth==0:return text[a:i+1]
    raise AssertionError(name)
def tokens(text):
    text=''.join(text.split()).replace(',0.,)', ',0.)')
    # rustfmt may wrap this single pure integer expression in closure braces.
    text=text.replace('|a|{u64::try_from(right.len()).ok().and_then(|b|a.checked_mul(b))}', '|a|u64::try_from(right.len()).ok().and_then(|b|a.checked_mul(b))')
    return text
assert tokens(body(current,'distances_four('))==tokens(body(baseline,'distances_four('))
assert tokens(body(current,'distances_sixteen('))==tokens(body(candidate,'distances_sixteen(').replace('let k = base + 0;','let k = base;'))
active='#\[cfg\(all\(target_arch = "aarch64", target_os = "macos"\)\)\]'
inactive='#\[cfg\(not\(all\(target_arch = "aarch64", target_os = "macos"\)\)\)\]'
original=body(current,'match_gradients_selected_with_scratch<')
mac=re.sub(inactive+r'\s*let distances = distances_four\(&a.descriptor, chunk\);','',original,count=1);mac=re.sub(active+r'\s*','',mac,count=1).replace('FIRST_PASS_LANES','16')
other=re.sub(active+r'\s*let distances = distances_sixteen\(&a.descriptor, chunk\);','',original,count=1);other=re.sub(inactive+r'\s*','',other,count=1).replace('FIRST_PASS_LANES','4')
assert tokens(mac)==tokens(body(candidate,'match_gradients_selected_with_scratch<'))
assert tokens(other)==tokens(body(baseline,'match_gradients_selected_with_scratch<'))
assert 'const FIRST_PASS_LANES: usize = if cfg!(all(target_arch = "aarch64", target_os = "macos"))'in current
assert '#[cfg(any(test, all(target_arch = "aarch64", target_os = "macos")))]\n#[allow(clippy::too_many_lines)]'in current
O=D/'dedup-gradient-lanes16-root-code-audit-20261014.json';assert not O.exists();O.write_text(json.dumps(dict(status='verified_target_selected_runtime_source_equivalence',auditor_sha256=sha(__file__),root_source=str(root.relative_to(R)),root_source_sha256=sha(root),candidate_manifest=str(P.relative_to(R)),candidate_manifest_sha256=sha(P),mac_arm_matcher_equals_qualified_candidate_after_cfg_constant_expansion=True,other_target_matcher_equals_original_baseline_after_cfg_constant_expansion=True,four_lane_helper_unchanged=True,sixteen_lane_helper_equals_qualified_candidate_after_identity_expression_normalization=True,format_normalizations=['optional trailing comma in multi-element zero tuple','optional braces around the exact pure u64 work-expression closure'],scope='Token/source equivalence on inspected cfg selections after explicit formatting normalizations, not emitted-machine-code equivalence or runtime performance on unmeasured targets. Actual current root default/raster tests remain independently required. No full native/workspace/corpus/defaultcollection/formats/platform completion claim.'),indent=2)+'\n');print(O)
