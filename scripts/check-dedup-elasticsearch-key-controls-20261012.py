#!/usr/bin/env python3
"""Execute only original key grouping; never initialize Elasticsearch/client."""
import ast
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
DOC = ROOT / 'docs/research/dedup-500'


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def main():
    manifest = DOC / 'deduplicate-elasticsearch-full-source-20261012.json'
    metadata = json.loads(manifest.read_text())
    source = Path(metadata['source']) / 'deduplicate-elaticsearch.py'
    parsed = ast.parse(source.read_text())
    selected = []
    for node in parsed.body:
        if isinstance(node, ast.Assign) and any(isinstance(target, ast.Name) and target.id in
                {'dict_of_duplicate_docs', 'keys_to_include_in_hash'} for target in node.targets):
            selected.append(node)
        elif isinstance(node, ast.FunctionDef) and node.name == 'populate_dict_of_duplicate_docs':
            selected.append(node)
    assert len(selected) == 3
    namespace = dict(hashlib=hashlib)
    exec(compile(ast.Module(body=selected, type_ignores=[]), str(source), 'exec'), namespace)
    grouping = namespace['dict_of_duplicate_docs']
    function = namespace['populate_dict_of_duplicate_docs']
    keys = namespace['keys_to_include_in_hash']
    assert keys == ['CAC', 'FTSE', 'SMI']
    rows = []
    for name, left, right in [
        ('field_boundary_erased_strings', ['ab', 'c', 'd'], ['a', 'bc', 'd']),
        ('field_boundary_erased_numbers', [1, 23, 4], [12, 3, 4]),
        ('number_and_string_types_erased', [1, 2, 3], ['1', '2', '3']),
        ('null_and_string_types_erased', [None, 2, 3], ['None', 2, 3]),
    ]:
        grouping.clear()
        assert left != right
        function(dict(_id='left', _source=dict(zip(keys, left))))
        function(dict(_id='right', _source=dict(zip(keys, right))))
        assert list(grouping.values()) == [['left', 'right']]
        rows.append(dict(control=name, left=left, right=right, buckets=1))
    grouping.clear()
    try:
        function(dict(_id='missing', _source={'CAC': 1, 'FTSE': 2}))
    except KeyError as error:
        assert error.args == ('SMI',) and grouping == {}
        rows.append(dict(control='missing_field_raises_before_group_insertion', exception='KeyError', field='SMI'))
    else:
        raise AssertionError('missing field')
    hit = dict(_id='same', _source=dict(zip(keys, [1, 2, 3])))
    function(hit)
    function(hit)
    assert list(grouping.values()) == [['same', 'same']]
    rows.append(dict(control='repeated_document_id_is_counted_twice', ids=['same', 'same']))
    grouping.clear()
    for identity, extra in [('left', 'USD'), ('right', 'EUR')]:
        function(dict(_id=identity, _source=dict(hit['_source'], currency=extra)))
    assert list(grouping.values()) == [['left', 'right']]
    rows.append(dict(control='unselected_fields_are_outside_key_scope', currencies=['USD', 'EUR']))
    grouping.clear()
    function(hit)
    function(dict(_id='other', _source=dict(zip(keys, [4, 5, 6]))))
    assert len(grouping) == 2
    rows.append(dict(control='different_encoded_keys_have_distinct_buckets', buckets=2))
    assert len(rows) == 8
    output = DOC / 'deduplicate-elasticsearch-runtime-controls-20261012.json'
    assert not output.exists()
    pins = {str(path): sha(path) for path in [manifest, source, Path(__file__)]}
    output.write_text(json.dumps(dict(status='complete_original_key_function8_controls',
        authored_controls=8, upstream_tests=0, controls=rows, pins=pins,
        extraction='Unchanged original2Assign and1FunctionDef; real hashlib, no client/module-main execution.',
        scope='Original key grouping only: structural key collisions and repeated-id behavior. No Elasticsearch/scan/mget/delete, external server or full module qualification; source does not implement equality confirmation or DELETE.'), indent=2) + '\n')
    print('8 original-key-function controls passed')


if __name__ == '__main__':
    main()
