"""Record this batch's reviewed outcomes; retain initial acceptance and evidence."""
import collections,hashlib,json,platform,re
from pathlib import Path
root=Path(__file__).resolve().parents[3];out=Path(__file__).resolve().parent
rel=out.relative_to(root).as_posix();registry=root/'docs/improvements-1000.json'
d=json.loads(registry.read_text())
if d['tasks'][55].get('execution',{}).get('status')=='verified':
 raise SystemExit('Historical partial snapshot: refusing to overwrite verified RR-0056; use the registry refresher.')
rows={}
def result(n,text,logs,status='rejected',remaining=None):
 rows[n]=dict(status=status,result=text,logs=logs,remaining=remaining or 'Отклонена указанная конкретная гипотеза; production не изменён. Это не квалификация всех возможных scratch/pool оптимизаций и не заявление об ускорении. Полный исходный performance acceptance не заявляется выполненным.')
result(31,'PVR сначала считает metadata records, затем try_reserve_exact(records); reservation корректируется по фактической capacity. Повторный рост этой коллекции отсутствует; возвращаемые metadata принадлежат результату.', ['texture-tests.log'])
result(32,'WMF object table уже создаётся vec![None; count] после ограничения count≤4096. Предлагаемое точное резервирование дублирует действующий путь; смена владельца/pool потребовала бы отдельной API-гипотезы.', ['wmf-tests.log'])
result(33,'Проверен кандидат reserve(element.count+1) для PLY list offsets. На усечённом теле с объявлением 1 000 000 элементов пик managed bytes вырос 1151→4001151 при одинаковом отказе и final=0. Текущий постепенный рост оставлен.', ['mesh-tests.log'])
result(34,'Все внешние команды X3F qualification переведены на общий watchdog дерева процессов. Добавлен --timeout (по умолчанию 1800 с на команду), запрещены неположительные/NaN/Inf. Проверены успешный exit, exit=7 со stderr и зависший SIGTERM-resistant потомок; timeout не считается успехом.', ['python-regressions.log','x3f-help.log'], 'verified', 'Проверен локальный POSIX watchdog и CLI. Полный camera/oracle/Metal прогон X3F и Windows process cleanup этим изменением повторно не квалифицированы; настройки VoiceOver не менялись.')
result(35,'Выход linear sRGB уже имеет try_reserve_exact(count), фактическую capacity и credit; LUT ограничен и допускает scalar fallback. Выходной Vec удерживается raster, а не является временным scratch.', ['develop-tests.log'])
result(36,'Убрано BTreeSet всех DICOM tags: строгий порядок проверяет previous, четыре обязательных meta tag — битовая маска. Совпадают ошибки/пиксели и бюджет. Две серии по 10 чередующихся пар: total ratio 0.132/0.128 при A/A≈1; malloc 54140→1190 за 300 декодирований в отдельном allocation probe.', ['scientific-tests.log','dicom-pairs-1.log','dicom-pairs-2.log','dicom-summary.json','dicom-allocation-results.json'], 'verified', 'Выигрыш относится к CPU full-decode корпусу с 0/64/4096 private tags и малым pixel payload, не ко всем DICOM/просмотрщику. Managed peak=96 B, per-process RSS измерен отдельно. API, допустимые форматы и pixel output сохранены; физический HDR не проверялся.')
result(37,'Обе DPX output ветви уже заранее резервируют полное количество samples и учитывают capacity в credit. Коллекция становится владельцем выходных пикселей; повторного роста отмеченного Vec нет.', ['vfx-tests.log'])
result(38,'MNG: предварительный reserve 4096 кадров на одном 8-byte signature выделяет 819200 B вместо 0, хотя header отвергается одинаково. Managed counters=0 не покрывают этот heap. Сохранён отрицательный regression и отдельные allocation/RSS пробы.', ['mng-tests.log','mng-allocation-results.json'])
result(39,'KTX заранее вычисляет output/level sizes, резервирует выход точно и проверяет бюджет. Указанный decode anchor не доказывает временный growing scratch; новое предварительное резервирование повторяет существующее.', ['texture-tests.log'])
result(40,'PSD raw planes заимствованы из входа; compressed planes выделяются по известному total, RGBA output сразу получает pixels*4 capacity. Эти выделения удерживают данные результата, не независимый повторно используемый scratch.', ['layered-tests.log'])
result(41,'Pixel-selected model banks используют фиксированные 25 slots и предварительно рассчитанные/допущенные размеры. Повторный рост отмеченного массива отсутствует; перераспределение кредитов не даёт доказанного выигрыша.', ['dedup-decode-tests.log'])
result(42,'Gradient matcher получает явный scratch budget, заранее выделяет forward/reverse, учитывает distance/runners. Наличие Vec::new само по себе не доказывает рост: границы и capacity заданы до заполнения.', ['dedup-gradient-tests.log','dedup-decode-tests.log'])
result(43,'Anchor-rank aw/bw резервируются по числу points; их reservations переходят в сохраняемое evidence. Пул потребовал бы изменить владельца результата, точное резервирование уже реализовано.', ['dedup-decode-tests.log'])
result(44,'JNG z.write_all пишет 2/4 байта локального metadata carrier 1×1 в память через ZlibEncoder. Файловый backend здесь отсутствует; объединение disk I/O и cold/warm benchmark к отмеченному вызову неприменимы.', ['animation-tests.log'])
result(45,'PLY swap уже читает scalar blocks по 16 KiB. Дополнительный BufReader повышает managed peak 2900→19284 B и не проходит прежний tight budget. Проверены roundtrip, framing NEXT, короткие/Interrupted чтения, усечение, отмена, corrupt magic; final=0.', ['mesh-tests.log'])
result(46,'CR2 decode_pixels заранее резервирует total и передаёт выделение в sensor raster с credit. Отмеченный Vec — выход, а не забытый growing scratch; предлагаемое exact reserve уже есть.', ['cr2-tests.log'])
result(47,'Gradient collection заранее резервирует entries и owners по числу файлов и принимает фактическую capacity в бюджет. Время жизни owners требуется для сохранения результатов; общий пул не является локальной заменой Vec.', ['dedup-decode-tests.log'])
result(48,'decode_fits — ограниченная read/decode обёртка. parse_header делает count pass, заимствует cards из bytes и резервирует budget buffer(count); metadata упорядочены. Предположение о неизвестном growing temporary не подтверждено.', ['scientific-tests.log'])
result(49,'Prefetch protected содержит CacheKey, не pixel leases. При cache count limit=0 текущий worker прерывается до первого push. Eager reserve плана 4096 путей выделяет 131072 B вместо 0; managed final=0 в обеих ветках. Кандидат отклонён.', ['gallery-tests.log'])
result(50,'Одноэлементный Vec gamma/TRC передаётся во владеющий ICC profile, не растёт в цикле; gamma=1 использует пустую identity кривую. Предложенное повторное использование scratch не сохраняет эту семантику владения.', ['animation-tests.log'])
result(51,'WebP: eager reserve 4096 frames выделяет 163840 B до отказа на неполном VP8X input; текущая capacity=0 и та же ошибка. Managed counters не видят это выделение. Сохранены regression и отдельные allocation/RSS пробы.', ['animation-tests.log','webp-allocation-results.json'])
result(52,'Anchor-rank opposite swapped points заранее резервируются по точному числу proposals; reservation переносится в retained evidence. Исходный набор освобождается после передачи; локальный growing scratch не обнаружен.', ['dedup-decode-tests.log'])
result(53,'OBJ swap уже объединяет scalar/record чтения блоками 16 KiB. Дополнительный BufReader повышает managed peak 1039→17423 B и нарушает прежний tight admission; негативные streaming/cancel/framing сценарии проходят без него.', ['mesh-tests.log'])
result(54,'slots создаются заново внутри develop_with_cancel; AtomicUsize.fetch_add выдаёт каждому worker уникальный idx. Каждый mutex принадлежит одному worker, конкурирующие вызовы имеют разные slots. Удержание lock не создаёт предполагаемую очередь между запросами.', ['develop-tests.log'])
result(55,'JSON отчёт публикуется через уникальный tempfile в том же каталоге, flush/fsync и os.replace; прежний mode сохраняется. POSIX flock сериализует writers с deadline 5 с и позволяет следующему writer удалить собственные orphan temp. Реальные SIGKILL до/после replace и конкурентный читатель не видят частичный JSON.', ['python-regressions.log','bench-report-existing-tests.log'], 'verified', 'Квалифицирован локальный POSIX/macOS filesystem. SIGKILL может оставить tempfile до следующей публикации; постоянный .lock намеренно сохраняется. Нет заявления о power-loss durability, NFS, Windows или несогласованных сторонних writers. CLI/report schema сохранены.')
result(56,'Подтверждён unlink swap payload внутри TTL index mutex. Реальные файлы, A/A и A/B при 1/2/4 читателях: p95 wait ratio 0.033/0.027/0.046 при drain+drop после unlock. Этот кандидат не внедрён: дополнительный Vec всех expired payloads не допущен бюджетом, а cleanup tails нестабильны. Held handles остаются читаемыми; quota возвращается в 0.', ['cache-swap-tests.log','swap-ttl-pairs.log','swap-ttl-summary.json'], 'partial', 'Нужен ограниченный по памяти протокол извлечения/удаления expired payloads и повторные парные замеры с большими TTL batches, конкурирующими put/get, отменой и admission gates. Малый синхронизированный benchmark не квалифицирует production worker или общее ускорение. Production TTL путь пока сохранён.')
result(57,'curve_error mutex создаётся на один PDF decode; lock выполняет только присвоение ошибки budget callback. В нормальной ветке lock не берётся, независимые decode не делят mutex. Гипотеза длинной общей критической секции не подтверждена.', ['pdf-tests.log'])
result(58,'HEIF RGBA output уже создаётся with_capacity(width*height*4) после проверки размера и reservation. Выход удерживает pixel storage; точное резервирование не новое изменение. Нативные libheif tests выполнены на этой машине.', ['heif-tests.log'])
result(59,'STI заранее резервирует samples, reconciles capacity и credit; выход удерживается raster. Отмеченный Vec::new не означает повторных аллокаций.', ['texture-tests.log'])
result(60,'HDR CPU conversion заранее резервирует output(count). PQ LUT ограничен бюджетом, scalar fallback сохраняет работоспособность при тесном бюджете. Повторное exact reserve уже реализовано; тесты не требуют физического HDR дисплея.', ['develop-tests.log'])
assert len(rows)==30
for t in d['tasks'][30:60]:
 n=int(t['id'][3:]);r=rows[n]
 t['execution']={k:r[k] for k in ['status','result','remaining']}
 t['execution'].update(updated_at='2026-10-25',evidence_files=[rel+'/'+v for v in ['source-review.json']+r['logs']],report=rel+'/REPORT.md',qualification_scope='Конкретная гипотеза по текущему коду; rejected не является выполненной оптимизацией или полным performance gate.')
 # Original code observation remains in evidence, initial task acceptance stays intact.
counts=dict(collections.Counter(r['status']for r in rows.values()))
batch=dict(name='Проверка RR-0031–RR-0060',ids=[t['id']for t in d['tasks'][30:60]],summary=counts,report=rel+'/REPORT.md',note='Проверены 30 исходных гипотез. verified — внедрённое и проверенное изменение; rejected — конкретная гипотеза отклонена анализом или негативной проверкой, не успешная оптимизация; partial — подтверждение есть, безопасная реализация не завершена. VoiceOver и другое оборудование исключены по указанию пользователя.')
d['execution_batches']=[b for b in d.get('execution_batches',[])if b.get('name')!=batch['name']]+[batch]
d['tasks'][9]['execution']['remaining']='VoiceOver и требующие другого оборудования проверки отложены пользователем (2026-10-25), повторного разрешения не ожидают. Это не полная screen-reader/contrast квалификация всех режимов, форматов и платформ; physical HDR display не проверен.'
d['tasks'][9]['execution']['deferred_by_user']=True
registry.write_text(json.dumps(d,ensure_ascii=False,indent=2)+'\n')
# Capture source context for current symbols; retained source-audit.json is the earlier observation.
audit=[]
for t in d['tasks'][30:60]:
 e=t['evidence'];path=root/e['path'];lines=path.read_text().splitlines()
 matches=[i for i,line in enumerate(lines)if line.strip()==e['excerpt']]
 if not matches: matches=[i for i,line in enumerate(lines)if e['symbol']in line]
 assert matches,t['id']
 i=min(matches,key=lambda i:abs(i+1-e['line']))
 audit.append(dict(id=t['id'],path=e['path'],symbol=e['symbol'],line=i+1,sha256=hashlib.sha256(path.read_bytes()).hexdigest(),source_context='\n'.join(f'{j+1}: {lines[j]}'for j in range(max(0,i-8),min(len(lines),i+33))),execution=rows[int(t['id'][3:])]))
(out/'source-review.json').write_text(json.dumps(audit,ensure_ascii=False,indent=2)+'\n')
print(counts)
