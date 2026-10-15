"""Independently verify complete unchanged cache integration execution and durable raw evidence."""
from pathlib import Path
import hashlib,json,re,subprocess
R=Path(__file__).resolve().parents[1];D=R/'docs/research';P=D/'dedup-durable-cache-v2-20261016.json';d=json.loads(P.read_text());h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();assert d['passed']==10 and d['failed']==d['ignored']==d['filtered']==0 and len(set(d['inventory']))==10
assert h(R/'scripts/qualify-dedup-durable-cache-v2-20261016.py')==d['producer_sha256']
for p,sha in d['source_pins'].items():assert h(R/p)==sha
for p,sha in d['dependency_pins'].items():assert h(p)==sha
for p,sha in d['logs'].items():assert h(p)==sha
for name in ['binary','library','harness']:assert h(d[name])==d[name+'_sha256']
old_path=R/d['prior_audit'];assert h(old_path)==d['prior_audit_sha256'];old=json.loads(old_path.read_text());assert sorted(d['inventory'])==sorted(old['inventory'])
W=Path(d['binary']).parent
for name,original in [('cache.rs','crates/rrrah-dedup/src/cache.rs'),('pixels.rs','crates/rrrah-dedup/src/pixels.rs'),('integration.rs','crates/rrrah-dedup/tests/cache.rs')]:assert (W/name).read_bytes()==(R/original).read_bytes()
s=(R/'crates/rrrah-dedup/src/local.rs').read_text();error=s[s.index('#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]'):s.index('\n/// Extract deterministic')];helper=s[s.index('pub(crate) fn reserve_slot'):s.index('\n#[allow(clippy::too_many_lines)]')];expected='#[allow(dead_code)]mod local {\n'+error+'\n'+helper+'\n}\n#[path="pixels.rs"]pub mod pixels;\n#[path="cache.rs"]pub mod cache;\n';assert Path(d['harness']).read_text()==expected
for label in ['library-build','integration-build']:
 assert '-Dwarnings' in d['commands'][label] and not (W/(label+'.log')).read_text().strip()
listed=subprocess.check_output([d['binary'],'--list'],cwd=R,text=True);assert sorted(re.findall(r'^(.*): test$',listed,re.M))==sorted(d['inventory'])
log=(W/'tests.log').read_text();results=re.findall(r'^test (\S+) \.\.\. (ok|FAILED|ignored)$',log,re.M);assert sorted(results)==sorted((n,'ok') for n in d['inventory']) and '10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out' in log
assert 'roundtrip_requires_content_recipe_and_frame_identity' in d['inventory'] and 'zero_budget_and_owned_values_preserve_previous_entry' in d['inventory']
assert h(d['binary'])==d['binary_sha256'] and h(d['library'])==d['library_sha256']
out=D/'dedup-durable-cache-v2-audit-20261016.json';assert not out.exists();out.write_text(json.dumps(dict(status='verified_fresh_complete_current_cache_integration_scope',passed=10,failed=0,ignored=0,filtered=0,inventory=d['inventory'],corruption_bit_cases=3264,wire_bit_cases=2048,producer_report=str(P.relative_to(R)),producer_report_sha256=h(P),binary=d['binary'],binary_sha256=d['binary_sha256'],library=d['library'],library_sha256=d['library_sha256'],source_pins=d['source_pins'],auditor_sha256=h(__file__),scope=d['scope'],limits=['Nativefull10integrationtesttarget only, selectedcache/pixels library; notfullCargo/decoder/wholecrate/currentworkspace/platform/physicalpowerloss/hardRSS/performanceproof.','Bitcasecounts derive fromunchangedtestbodyloopbounds andactualcompleteexecutedtestinventory, not independentformatqualitycases.']),indent=2)+'\n');print('Verified10/10 complete currentcache target; nativefreshinventory and durablebinary/source/log hashes')
