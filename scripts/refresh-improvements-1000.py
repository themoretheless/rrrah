#!/usr/bin/env python3
"""Refresh source locations and render existing backlog; never generate tasks/statuses."""
import collections
import hashlib
import json
import re
from pathlib import Path
ROOT = Path(__file__).resolve().parents[1]
p = ROOT / 'docs/improvements-1000.json'
d = json.loads(p.read_text())
for t in d['tasks']:
    e = t['evidence']; lines = (ROOT / e['path']).read_text().splitlines()
    old = e.copy()
    if e['symbol'] == 'srgb_byte' and e['path'].endswith('/thumbnail.rs'):
        e['symbol'] = 'srgb_byte_f64'
    matches = [i for i, line in enumerate(lines, 1) if line.strip() == e['excerpt']]
    if not matches:
        matches = [i for i, line in enumerate(lines, 1) if e['symbol'] in line]
        if not matches:
            raise ValueError(f"Missing source anchor: {t['id']}")
        n = min(matches, key=lambda i: abs(i - e['line']))
        e['excerpt'] = lines[n-1].strip()
    else:
        n = min(matches, key=lambda i: abs(i - e['line']))
    e['line'] = n
    if old != e:
        t.setdefault('baseline_evidence', old)
for rel in d['source_sha256']:
    d['source_sha256'][rel] = hashlib.sha256((ROOT / rel).read_bytes()).hexdigest()
p.write_text(json.dumps(d, ensure_ascii=False, indent=2)+'\n')
md = ['# 1000 улучшений rrrah', '', f"Исходный аудит: {d['audit_date']}, база `{d['base_commit']}` плюс dirty checkout.", '', d['scope'], '', '## Выполнение первых 30', '', d['execution_snapshot_note'], '', '; '.join(f'{k}: {v}' for k,v in d['execution_summary'].items())+'.', '', '[Результаты и ограничения](research/first30-2026-10-14/REPORT.md). Статус исходного основания и execution.status независимы. partial не означает выполнение всего acceptance. Остальные 970 задач не квалифицированы повторно.', '', '## Методика исходного аудита', '']
md += ['- '+s for s in d['methodology']]
md += ['', 'Машинный источник: [improvements-1000.json](improvements-1000.json). Исходные SHA сохранены в audit_source_sha256; текущие локации и SHA обновлены для навигации. Проверка: `python3 scripts/validate-improvements-1000.py --strict-snapshot`.', '', '## Первые 30 задач', '', '| ID | Задача | Приоритет | Объём | Выполнение |', '|---|---|---|---|---|']
for t in d['tasks'][:30]:
    md.append(f"| [{t['id']}](#{t['id'].lower()}) | {t['title']} | {t['priority']} | {t['size']} | {t['execution']['status']} |")
for sub in dict.fromkeys(t['subsystem'] for t in d['tasks']):
    md += ['', f'## {sub}', '']
    for t in d['tasks']:
        if t['subsystem'] != sub: continue
        e = t['evidence']
        md += [f'<a id="{t["id"].lower()}"></a>', f"### {t['id']} — {t['title']}", '', f"**{t['type']} · {t['priority']} · {t['size']} · {t['evidence_status']}**", '', f"Основание: [{e['path']}:{e['line']}](../{e['path']}#L{e['line']}), `{e['symbol']}`. {e['observation']}", '', 'Участок: `'+e['excerpt'].replace('`','')+'`', '']
        if 'execution' in t:
            x=t['execution'];md += [f"**Выполнение: {x['status']} ({x['updated_at']}).** {x['result']}", '', f"Осталось/границы: {x['remaining']} [Отчёт]({Path(x['report']).relative_to('docs').as_posix()}).", '']
        md += [f"Изменение: {t['proposed_change']}", '', f"Приёмка: {t['acceptance']}", '', f"API/совместимость: {t['public_api']}", '', f"Зависимости: {', '.join(t['dependencies']) or 'нет установленных'}. Условия: {' '.join(t['verification_conditions'])}", '']
(ROOT / 'docs/IMPROVEMENTS_1000.md').write_text('\n'.join(md)+'\n')
print('Refreshed 1000 existing tasks; preserved acceptance and execution results.')
