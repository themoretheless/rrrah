#!/usr/bin/env python3
"""Synthetic CSV controls; no user vaults, credentials, services or CLI interaction."""
from pathlib import Path
import ast
import contextlib
import hashlib
import io
import json
import os
import tempfile
import types
from urllib.parse import urlparse

import pandas as pd

R=Path(__file__).resolve().parents[1];E=R/'docs/research/dedup-500/evidence/niqifan007--bitwarden_deduplicator'
O=R/'docs/research/dedup-vault172-controls-20261014.json';assert not O.exists()
names={'.gitignore':'03-gitignore','requirements.txt':'04-requirements.txt','LICENSE':'01-LICENSE','README.md':'00-README.md','bitwarden_deduplicator.py':'02-bitwarden_deduplicator.py'}
for item in json.loads((E/'tree.json').read_text())['tree']:
    data=(E/names[item['path']]).read_bytes();assert hashlib.sha1(b'blob '+str(len(data)).encode()+b'\0'+data).hexdigest()==item['sha']
tree=ast.parse((E/'02-bitwarden_deduplicator.py').read_text())
namespace={'pd':pd,'os':os,'urlparse':urlparse,'chardet':types.SimpleNamespace(detect=lambda data:{'encoding':'utf-8'})}
exec(compile(ast.Module(body=[node for node in tree.body if isinstance(node,ast.FunctionDef)],type_ignores=[]),str(E/'02-bitwarden_deduplicator.py'),'exec'),namespace)
checks=[]

def row(**changes):
    value=dict(type='login',name='Synthetic example',login_uri='https://example.test/a',login_username='TEST_USER',login_password='TEST_ONLY_SECRET_A');value.update(changes);return value

def check(name,condition):
    assert condition,name;checks.append(name)

with tempfile.TemporaryDirectory(prefix='rrrah-vault172-',dir='/private/tmp') as temporary:
    root=Path(temporary);number=0
    def run(records,domain=False,in_place=False):
        global number
        number+=1;source=root/f'input-{number}.csv';output=source if in_place else root/f'output-{number}.csv'
        pd.DataFrame(records).to_csv(source,index=False);before=source.read_bytes()
        with contextlib.redirect_stdout(io.StringIO()):result=namespace['deduplicate_bitwarden_csv'](str(source),str(output),domain)
        assert result is None and output.exists()
        return pd.read_csv(output,dtype=str,keep_default_na=False),source,output,before
    result,*_=run([row(),row()]);check('identical_selected_keys_collapse',len(result)==1)
    result,*_=run([row(),row(login_password='TEST_ONLY_SECRET_B')]);check('different_text_passwords_remain_distinct',len(result)==2)
    for key in ['notes','folder','login_totp']:
        result,*_=run([row(**{key:'FIRST_SYNTHETIC_VALUE'}),row(**{key:'SECOND_SYNTHETIC_VALUE'})])
        check('different_'+key+'_discarded_by_subset_identity',len(result)==1 and result.iloc[0][key]=='FIRST_SYNTHETIC_VALUE')
    result,*_=run([row(login_password='0012'),row(login_password='12')]);check('numeric_password_strings_coerce_and_collapse',len(result)==1 and result.iloc[0]['login_password']=='12')
    result,*_=run([row(login_username='001'),row(login_username='1')]);check('numeric_username_strings_coerce_and_collapse',len(result)==1 and result.iloc[0]['login_username']=='1')
    result,*_=run([row(login_password='NA'),row(login_password='')]);check('literal_na_password_and_empty_collapse',len(result)==1 and result.iloc[0]['login_password']=='')
    result,*_=run([row(),row(login_uri='https://example.test/b')]);check('full_uri_mode_retains_different_paths',len(result)==2)
    result,*_=run([row(),row(login_uri='https://example.test/b')],domain=True);check('domain_mode_collapses_different_paths',len(result)==1)
    check('domain_mode_adds_export_column',list(result.columns)[-1]=='domain')
    result,*_=run([row(),row(login_uri='http://example.test/a')],domain=True);check('domain_mode_collapses_different_schemes',len(result)==1)
    check('domain_parser_retains_userinfo_and_port',namespace['extract_domain']('https://testuser:testpass@example.test:443/a')=='testuser:testpass@example.test:443')
    check('schemeless_uri_returns_entire_original_path',namespace['extract_domain']('example.test/a')=='example.test/a')
    result,*_=run([row(),row(login_uri='https://EXAMPLE.test/a')],domain=True);check('domain_case_is_not_normalized',len(result)==2)
    bad=root/'missing-columns.csv';bad.write_text('name\nSynthetic\n');destination=root/'stale.csv';destination.write_bytes(b'PREEXISTING_SYNTHETIC_OUTPUT')
    with contextlib.redirect_stdout(io.StringIO()):value=namespace['deduplicate_bitwarden_csv'](str(bad),str(destination))
    check('missing_columns_return_none_and_leave_stale_output',value is None and destination.read_bytes()==b'PREEXISTING_SYNTHETIC_OUTPUT')
    try:namespace['deduplicate_bitwarden_csv'](str(root/'missing-input'),str(root/'never.csv'))
    except FileNotFoundError:check('encoding_read_missing_input_raises_outside_read_handler',True)
    else:raise AssertionError('missing input')
    broken=root/'broken.csv';broken.write_text('type,name\n"unterminated\n')
    with contextlib.redirect_stdout(io.StringIO()):value=namespace['deduplicate_bitwarden_csv'](str(broken),str(root/'failed.csv'))
    check('csv_parser_failure_is_swallowed_without_output',value is None and not (root/'failed.csv').exists())
    source=root/'valid.csv';pd.DataFrame([row()]).to_csv(source,index=False)
    try:
        with contextlib.redirect_stdout(io.StringIO()):namespace['deduplicate_bitwarden_csv'](str(source),str(root/'missing-parent/output.csv'))
    except OSError:check('output_parent_failure_propagates',True)
    else:raise AssertionError('missing output parent')
    result,source,output,before=run([row(),row()],in_place=True)
    check('same_input_output_is_permitted_and_overwrites_source',source==output and source.read_bytes()!=before and len(result)==1)
    check('output_uses_utf8_bom',output.read_bytes().startswith(b'\xef\xbb\xbf'))
assert len(checks)==21
O.write_text(json.dumps(dict(status='twenty_one_original_function_controls_with_real_pandas_confirmed',repository='niqifan007/bitwarden_deduplicator',commit='8b6f052ad32dcdc6a6f479b0d5c5e166b8d52ab4',count=len(checks),checks=checks,pandas_version=pd.__version__,pins={str(E/local):hashlib.sha256((E/local).read_bytes()).hexdigest() for local in names.values()},scope='Unchanged original function ASTs and real pandas CSV input/output on synthetic temporary records only. Missing chardet dependency replaced by explicit UTF8 detection adapter; automatic encoding detection, complete CLI/interactive flows and real vault data not executed. No source adoption.'),indent=2)+'\n');print(f'{len(checks)} controls confirmed')
