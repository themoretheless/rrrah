#!/usr/bin/env python3
"""Create a temporary instrumented copy: input.rs output.rs. No work-policy change."""
from pathlib import Path
import sys

source=Path(sys.argv[1]).read_text()
def edit(old,new):
    global source
    assert source.count(old)==1, old
    source=source.replace(old,new)
edit('    let mut pixel = OrderedPixel::new(admit, fragment_limit);',
     '    let trace_cost=std::env::var_os("RRRAH_TRACE_CELL_COST").is_some();\n    let entry_work=*work;\n    let mut pixel = OrderedPixel::new(admit, fragment_limit);')
edit('    for index in 0..candidates.len() {','    let paint_start=*work;\n    for index in 0..candidates.len() {')
edit('            Err(super::PixelPartitionError::Limit) if *work > 0 => {',
     '            Err(super::PixelPartitionError::Limit) if *work > 0 => {\n                if trace_cost { eprintln!("CELL_COST_SPLIT origin={origin:?} side={side} depth={depth} candidates={} processed={index} fragments={} total={} paint={}",candidates.len(),pixel.fragments(),entry_work-*work,paint_start-*work); }')
edit('            Err(error) => {\n                if std::env::var_os("RRRAH_TRACE_NATIVE_REPLAY_EVENT").is_some() {',
     '            Err(error) => {\n                if trace_cost { eprintln!("CELL_COST_FAIL origin={origin:?} side={side} depth={depth} candidates={} processed={index} fragments={} total={} paint={}",candidates.len(),pixel.fragments(),entry_work-*work,paint_start-*work); }\n                if std::env::var_os("RRRAH_TRACE_NATIVE_REPLAY_EVENT").is_some() {')
edit('    drop(candidates);\n    drop(candidate_credit);\n    let mut sum',
     '    if trace_cost && paint_start-*work>=100_000 {eprintln!("CELL_COST_OK origin={origin:?} side={side} depth={depth} candidates={} fragments={} total={} paint={}",candidates.len(),pixel.fragments(),entry_work-*work,paint_start-*work);}\n    drop(candidates);\n    drop(candidate_credit);\n    let mut sum')
edit('    let mut work = 100_000_000usize;',
     '    let trace_cost=std::env::var_os("RRRAH_TRACE_CELL_COST").is_some();\n    if trace_cost {eprintln!("SAMPLE_COST_BEGIN patches={} blocks={} extent={extent:?}",patches.len(),records.len());}\n    let mut work = 100_000_000usize;\n    let outcome=(|| -> Option<MeshSamples> {')
edit('            for block in &records {','            let tile_start=work;\n            for block in &records {')
edit('            for y in ty..y1.min(ty.saturating_add(4)) {',
     '            if trace_cost && tile_start-work>=1_000_000 {eprintln!("TILE_COST tile={tx},{ty} work={} remaining={work}",tile_start-work);}\n            for y in ty..y1.min(ty.saturating_add(4)) {')
edit('    Some(MeshSamples { map, _guard: guard })\n}',
     '    Some(MeshSamples { map, _guard: guard })\n    })();\n    if trace_cost {eprintln!("SAMPLE_COST_END patches={} complete={} spent={} remaining={work}",patches.len(),outcome.is_some(),100_000_000-work);}\n    outcome\n}')
Path(sys.argv[2]).write_text(source)
