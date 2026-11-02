"""Build the batch report from recorded results and retained command logs."""
import collections,hashlib,json,platform,re,statistics,subprocess
from pathlib import Path
root=Path(__file__).resolve().parents[3];out=Path(__file__).resolve().parent
registry=json.loads((root/'docs/improvements-1000.json').read_text());tasks=registry['tasks'][30:60]
if tasks[25].get('execution',{}).get('status')=='verified':
 raise SystemExit('Historical partial report: preserve this snapshot and read the RR-0056 bounded follow-up.')
native=json.loads((out/'native-results.json').read_text())
dedup_path=out/'dedup-decode-results.json'
dedup=json.loads(dedup_path.read_text())if dedup_path.exists()else None
alloc={}
for name in ['dicom','mng','webp']:
 a=json.loads((out/f'{name}-allocation-results.json').read_text())
 alloc[name]={variant:{key:statistics.median(row[key]for row in a['runs']if row['variant']==variant) for key in ['malloc','calloc','realloc','aligned','peak_rss_bytes']}for variant in ['legacy','candidate']}
counts=dict(collections.Counter(t['execution']['status']for t in tasks))
extra=['scripts/qualification_command.py','scripts/test-improvements-31-60.py','scripts/refresh-improvements-1000.py','scripts/validate-improvements-1000.py','scripts/test-improvements-1000.py','crates/rrrah-mesh/src/rr31_60_qualification.rs','crates/rrrah-mesh/src/lib.rs']
paths=sorted(set(t['evidence']['path']for t in tasks)|set(extra))
qualification=dict(date='2026-10-25',scope='RR-0031–RR-0060; CPU, local macOS only; existing dirty checkout preserved',host=dict(system=platform.system(),release=platform.release(),machine=platform.machine(),python=platform.python_version(),cpu=subprocess.check_output(['sysctl','-n','machdep.cpu.brand_string'],text=True).strip(),rustc=subprocess.check_output(['rustc','--version'],text=True).strip()),summary=counts,native_suites=native,dedup_decode=dedup,allocation_medians=alloc,dicom_pairs=json.loads((out/'dicom-summary.json').read_text()),swap_pairs=json.loads((out/'swap-ttl-summary.json').read_text()),source_sha256={p:hashlib.sha256((root/p).read_bytes()).hexdigest()for p in paths},limitations=['26 rejected are narrow hypotheses, not full benchmark acceptance or optimizations','RR-0056 partial: full drain metadata is unadmitted O(expired count); production worker unchanged','Zero-test default-feature dedup selections excluded; decode-feature suites reported separately','No VoiceOver, physical HDR, CUDA/NVIDIA, foreign enclosure, Windows or camera-oracle qualification','Managed capacity accounting and per-process physical RSS are distinct','Preliminary stopped DICOM run retained and excluded; instrumented allocation processes excluded from timing','Finite synthetic metadata-rich DICOM workload does not establish viewer latency or all-DICOM speedup'])
(out/'qualification.json').write_text(json.dumps(qualification,ensure_ascii=False,indent=2)+'\n')
md='''# RR-0031–RR-0060: проверка гипотез и внедрение

Дата: 2026-10-25. Проверен текущий dirty checkout; существующие изменения сохранены. VoiceOver, физический HDR, CUDA/NVIDIA, Windows и другое оборудование исключены по указанию пользователя. Системные настройки не менялись.

**Итог: 3 verified, 26 rejected, 1 partial.** Внедрены изменения RR-0034, RR-0036, RR-0055. RR-0056 требует безопасного по памяти продолжения. Это не 30 внедрённых оптимизаций.

`rejected` означает отклонение конкретного предположения исходного статического аудита. Где код уже делает exact reserve или коллекция владеет результатом, повторное измерение не превращает это в новую оптимизацию. Для таких пунктов сохранён анализ и выполнены correctness suites, но полный первоначальный allocation/RSS/performance acceptance не объявлен пройденным. Для опасных кандидатов сохранены отрицательные воспроизводимые проверки. Исходные acceptance/proposed_change и evidence_status сохранены в реестре; execution отражает отдельный результат проверки.

Машинные результаты: [qualification.json](qualification.json), [текущие основания](source-review.json), [исходный контекст этого прохода](source-audit.json). Команды и ограничения сохранены в журналах. Перегенерация Markdown реестра не изменяет исходные критерии задач.

## Внедрённые изменения

### RR-0034: deadline для внешних команд X3F qualification

`scripts/qualify-x3f-sd10.py` использовал синхронный subprocess без локального deadline. Теперь все команды проходят через общий process-tree watchdog (`qualification_command.run_logged` → `qualify-viewer-storage-gpu.run_suite`). Добавлен `--timeout` в секундах, default=1800 **на команду**, не на весь qualification run. Положительное конечное значение проверяется до fixture I/O. Timeout и ненулевой exit сохраняются в командном журнале и завершаются диагностической ошибкой; timeout не становится success даже если родитель успел вернуть 0. Вывод объединяет stdout/stderr как ранее. Публичные аргументы и report schema сохранены, флаг additive.

На локальном POSIX проверены нормальный процесс, exit=7 со stderr и зависающий процесс с SIGTERM-resistant потомком; после deadline потомок не выполняется. Watchdog имеет дополнительный срок завершения дерева после deadline. Принудительно отсоединившиеся от process group потомки не входят в доказанную область. Полный прогон pinned SD10/SD14 camera/oracle/Metal этим изменением не повторялся; Windows taskkill путь не квалифицирован.

### RR-0036: DICOM tag membership без дерева

До изменения strict-order decode одновременно хранил предыдущий tag и BTreeSet всех tags. При строго возрастающем порядке повтор и убывание определяются одним previous; четыре обязательных Part 10 meta tag учитываются битовой маской. Дубликаты/порядок, обязательные поля, pixel output, отмена и отказ по бюджету сохранены. Проверены все последовательности длины 3 из набора 8 граничных tags против прежнего правила и отдельные случаи пропущенных обязательных meta tags. Старый BTreeSet путь оставлен только под cfg(test) с thread-local переключением для сравнения в одном бинарнике; публичный API не менялся.

Benchmark — полный CPU decode явного float32 DICOM из fixture, дополненного 0/64/4096 упорядоченными private OB tags. Чередование small/large/small/medium, 120 decode на вариант, 3 warmup и 10 чередующихся A/A и A/B пар; две сохранённые серии. Перед измерением проверяется совпадение результата старого/нового пути; managed peak=96 B, final=0; tight budget=1 отвергается с final=0. Median candidate/baseline total ratios: **0.1318 / 0.1284** (в каждой серии 10/10 выигрышей), A/A **1.0018 / 0.9982**. Это измерение маленького pixel payload с большим metadata; ускорение обычного открытия, всего viewer или любого DICOM из него не следует.

[Серия 1](dicom-pairs-1.log), [серия 2](dicom-pairs-2.log), [отношения пар](dicom-summary.json). Предварительный запуск [dicom-pairs-preliminary.log](dicom-pairs-preliminary.log) был приостановлен во время внешнего окна квалификации и исключён; файл не удалён. Во время принятых CPU пар собственные тяжёлые correctness suites не запускались; A/A оставлен как контроль шума. Это локальные измерения, без обещания изоляции всей системы.

Allocation probes выполнялись отдельными процессами с 300 decode и DYLD interpose libc allocation APIs. Это process-wide call counts с общим setup/test overhead, **не число только Rust heap allocations и не число живых bytes**. `wait4` даёт per-child macOS peak RSS в bytes, отдельно от managed counter. Инструментированные прогоны исключены из timing. Сохранены C исходник, [драйвер](probe-process.py), SHA бинарников и 10 чередующихся process pairs для каждого сценария.

| Сценарий | Вариант | malloc / calloc / realloc / aligned, median | Peak RSS, median B |
|---|---|---|---|
'''
for name,data in alloc.items():
 for variant,r in data.items():
  md+=f"| {name} | {variant} | "+' / '.join(str(int(r[k]))for k in ['malloc','calloc','realloc','aligned'])+f" | {int(r['peak_rss_bytes'])} |\n"
md+='''
MNG/WebP rows относятся к отрицательным вариантам предварительного выделения 4096 кадров, а не к внедрённому ускорению. Heap capacity нельзя приравнивать к RSS: страницы не обязаны быть затронуты, RSS содержит библиотеки/stack/setup и имеет шум. Основное основание отказа здесь — расход до валидации header без допуска бюджета; небольшое изменение RSS не интерпретируется как точный размер буфера.

### RR-0055: целая публикация JSON отчёта

Финальный `--json-out` читает следующий запуск `--baseline-json` и другие consumers. Прямая запись могла усечь старый документ до завершения новой записи. Теперь уникальный tempfile создаётся в том же каталоге, закрывается после flush/fsync, затем публикуется os.replace. Сохраняются прежние file mode и symlink-target semantics; новые файлы подчиняются umask. На POSIX постоянный per-destination flock inode сериализует cooperating writers, ожидание ограничено 5 с. Следующая публикация под lock удаляет только managed orphan names с точным 32-hex суффиксом; проверены имена с glob-метасимволами.

Реальные SIGKILL перед/после replace оставляют старый/новый целый JSON. Конкурентный reader при 49 публикациях не увидел частичный документ. Проверены replace failure, сохранность mode/старого JSON, bounded lock wait и запрет удаления temp активного writer. После SIGKILL tempfile может остаться **до следующего writer**; постоянный `.lock` оставлен намеренно. Это локальная process-interruption atomicity, не power-loss durability (directory fsync не добавлялся), не NFS/Windows qualification и не защита от стороннего writer, игнорирующего протокол.

[Новые Python regressions](python-regressions.log): 8 passed. [Существующие bench-report tests](bench-report-existing-tests.log): 7 passed.

## Отрицательные проверки

| ID | Кандидат | Наблюдение |
|---|---|---|
| RR-0033 | reserve PLY offsets по недоверенному объявлению | invalid body, managed peak 1151→4001151 B, final=0 |
| RR-0038 | reserve MNG 4096 frames до проверки header | capacity 0→4096, requested heap 0→819200 B; та же ошибка; managed peak=0 |
| RR-0045 | второй 16 KiB stream buffer поверх PLY scalar blocks | managed peak 2900→19284 B; прежний tight budget пропускает baseline и отвергает буфер |
| RR-0049 | reserve всех CacheKey prefetch plan | при cache count limit=0 и 4096 paths requested heap 0→131072 B до первого decode; managed final=0 |
| RR-0051 | reserve WebP 4096 frames до проверки header | capacity 0→4096, requested heap 0→163840 B; та же ошибка; managed peak=0 |
| RR-0053 | второй 16 KiB stream buffer поверх OBJ blocks | managed peak 1039→17423 B; tight admission нарушен |

Эти кандидаты присутствуют только под cfg(test), production-путь сохранён. Для OBJ/PLY проверены roundtrip/wire bytes, short/Interrupted reads, framing с NEXT sentinel, усечение на нескольких позициях, ошибка отмены после 24 B и corrupt magic; credits возвращаются в 0. Это memory-stream/admission проверки, не замер cold/warm диска. MNG/WebP считаются структурными отказами, не доказательством нулевой heap памяти всего decode.

## Воспроизведение проверок

```sh
python3 scripts/test-improvements-31-60.py
python3 scripts/test_bench.py
python3 docs/research/rr-0031-0060-2026-10-25/run-native-checks.py
python3 docs/research/rr-0031-0060-2026-10-25/run-dedup-checks.py
cargo test -p rrrah-scientific --lib dicom::rr36_qualification::full_decode_tag_set_pairs -- --ignored --show-output --test-threads=1
cargo test -p rrrah-cache --lib rr56_qualification::ttl_release_outside_lock_pairs -- --ignored --show-output --test-threads=1
cc -dynamiclib -O2 docs/research/rr-0031-0060-2026-10-25/allocation-calls.c -o /tmp/rrrah-31-60-allocation-calls.dylib
python3 docs/research/rr-0031-0060-2026-10-25/probe-process.py
RRRAH_PROBE_NAME=mng python3 docs/research/rr-0031-0060-2026-10-25/probe-process.py
RRRAH_PROBE_NAME=webp python3 docs/research/rr-0031-0060-2026-10-25/probe-process.py
```

Перед allocation probe должны существовать соответствующие native binaries и журналы Running unittests: драйвер сохраняет SHA и не скачивает fixtures. Повторный запуск этих исследовательских драйверов перезаписывает одноимённые журналы; для независимого повторения сначала сохраните текущую папку результатов. При пересборке native бинарника нужно обновить журнал его запуска, иначе это новая идентичность измерения.

## RR-0056: подтверждённое ожидание, реализация не принята

TTL maintenance действительно уничтожает StoredPayload под общим mutex, а Drop SwapHandle может удалить файл. В [benchmark](swap-ttl-pairs.log) используются реальные SwapStore файлы (8×32 B), истёкшие TTL и 1/2/4 читателя, синхронно ожидающих index; 3 warmup, затем 10 alternating A/A и A/B пар по 10 rounds. Candidate вызывает drain_expired, отпускает mutex, затем уничтожает payloads. Проверено: index пуст после публикации, внешний held handle остаётся читаемым, store usage=1 object/32 B до release и 0/0 после него.

Median p95 wait ratios candidate/baseline: 0.0335 / 0.0271 / 0.0455; A/A: 0.871 / 0.820 / 0.985. Выигрыш синхронизированных readers большой, но это не production throughput. Дополнительная capacity Vec=256 B на 8 entries, растёт O(expired count) и не имеет admission. Cleanup p95 ratios 1.072 / 1.016 / 0.835 с нестабильными хвостами, в том числе отдельными длинными candidate samples; raw samples сохранены в полном виде. Поэтому production worker пока не изменён; **partial**. Следующий шаг — bounded extraction/release protocol без произвольного дополнительного heap и проверка больших batches, worker put/get, отмены/ошибок, lease/pin/quota и парные замеры. Нельзя закрыть задачу только по уменьшению времени mutex.

## Проверки по подсистемам

Все команды перечислены в [native-results.json](native-results.json) и соответствующих `*-tests.log`. Watchdog: 240 с на основную suite, 600 с на feature-enabled dedup run. Ни ignored tests, ни zero-test runs не являются проверкой заявленной функциональности. Основные тесты CPU; Metal/device tests не запускались.

| Suite | Passed | Failed | Ignored | Квалифицирован ненулевой набор |
|---|---:|---:|---:|---|
'''
for suite in native:
 summaries=suite['summaries'];passed=sum(int(r[1])for r in summaries);failed=sum(int(r[2])for r in summaries);ignored=sum(int(r[3])for r in summaries)
 md+=f"| {suite['name']} | {passed} | {failed} | {ignored} | {'да'if suite['executed_nonzero'] and suite['exit_code']==0 and not suite['timed_out'] else 'нет'} |\n"
if dedup:
 md+=f"\nDecode-feature dedup run: exit={dedup['exit_code']}, timed_out={dedup['timed_out']}; [журнал](dedup-decode-tests.log), [команда/результаты](dedup-decode-results.json).\n\n"
 md+='| Integration target (в порядке фактического выполнения) | Passed | Failed | Ignored |\n|---|---:|---:|---:|\n'
 for row in dedup['targets']:md+=f"| {row['name']} | {row['passed']} | {row['failed']} | {row['ignored']} |\n"
else:md+='\n**Feature-enabled dedup run ещё не завершён; не считать его успешным.**\n'
md+='''
Нулевые default-feature `dedup-model`, `dedup-anchor`, `dedup-collection` оставлены как диагностические попытки и исключены из coverage. Они заменяются отдельными integration targets с `--features decode`; это CPU providers/fixtures, без CUDA/LoFTR hardware claim. Native suites подтверждают указанные конечные наборы, не универсальность всех форматов, бюджетов или публичных API.

## Индивидуальные результаты

'''
for t in tasks:
 e=t['evidence'];x=t['execution']
 md+=f"### {t['id']} — {x['status']}\n\n[{e['path']}:{e['line']}](../../../{e['path']}#L{e['line']}), `{e['symbol']}`. {x['result']}\n\nГраницы: {x['remaining']}\n\nПроверки: "+', '.join(f'[{Path(p).name}]({Path(p).name})'for p in x['evidence_files'])+'.\n\n'
md+='''## Реестр и совместимость

По всем 30 ID записаны execution status/result/remaining/evidence/report без подмены исходных acceptance. Первые 30 summary сохранены отдельно; добавлена execution_batches секция, её счётчики, ID и report проверяются валидатором. RR-0010 остаётся partial и помечена deferred_by_user: VoiceOver сейчас не нужен пользователю, повторного разрешения не ожидаем. Этот проход не меняет прежние результаты оборудования.

Проверки: точные 1000 ID, уникальность, обязательные поля, зависимости/циклы, source anchors/SHA, execution references и batch counters; тесты валидатора включают искажение batch summary, duplicate/unknown IDs, отсутствие execution и unsafe report path. [Registry tests](registry-tests.log), [strict validator](registry-validation.log). Проверка документа не заменяет runtime proof. Новых публичных Rust API нет; изменения CLI описаны выше. Производственные изменения ограничены X3F watchdog, DICOM tag bookkeeping и bench-report publication.
'''
(out/'REPORT.md').write_text(md)
print(counts,'dedup_completed',dedup is not None)
