from pathlib import Path
import subprocess
root=Path.cwd();cache=root/'.dedup-work/visualvault076-20261021';dest=Path('/Users/themoretheless/.codex/tmp/rrrah-visualvault076-render-20261021');(dest/'src').mkdir(parents=True,exist_ok=True)
s='extern crate self as visualvault_models;\nextern crate self as visualvault_utils;\nextern crate self as visualvault_app;\n'
for name,p,exports in [('media_file','crates/models/src/media_file.rs','MediaFile,FileType,MediaMetadata'),('state','crates/models/src/state.rs','AppState,InputMode'),('progress_model','crates/utils/src/progress.rs','Progress'),('bytes','crates/utils/src/bytes.rs','format_bytes')]:s+='#[path="'+str(cache/p)+'"] mod '+name+';\npub use '+name+'::{'+exports+'};\n'
s+='''// Authored App field adapter for unchanged drawing functions; no application initialization.
pub struct App {pub progress:std::sync::Arc<tokio::sync::RwLock<Progress>>,pub state:AppState,pub search_input:String,pub input_mode:InputMode,pub search_results:Vec<MediaFile>,pub scroll_offset:usize}
'''
for name,p in [('progress_ui','crates/ui/src/progress.rs'),('search_ui','crates/ui/src/search.rs'),('details_ui','crates/ui/src/file_details.rs')]:s+='#[path="'+str(cache/p)+'"] mod '+name+';\n'
s+='''
#[cfg(test)] mod controls {
use super::*;use ratatui::{Terminal,backend::TestBackend};
fn app()->App {let mut p=Progress::default();p.total=10;p.current=5;App {progress:std::sync::Arc::new(tokio::sync::RwLock::new(p)),state:AppState::Scanning,search_input:"я".into(),input_mode:InputMode::Insert,search_results:vec![],scroll_offset:0}}
#[test] fn control_progress_over_total_panics_in_original_gauge_draw() {
 let app=app();let mut terminal=Terminal::new(TestBackend::new(100,50)).unwrap();terminal.draw(|f|progress_ui::draw_progress_overlay(f,&app)).unwrap();
 app.progress.try_write().unwrap().current=20;
 assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(||{terminal.draw(|f|progress_ui::draw_progress_overlay(f,&app)).unwrap();})).is_err());
}
#[test] fn control_search_cursor_uses_utf8_bytes_instead_of_columns() {
 let app=app();let mut terminal=Terminal::new(TestBackend::new(80,30)).unwrap();terminal.draw(|f|{let area=f.area();search_ui::draw(f,area,&app);}).unwrap();let cursor=terminal.get_cursor_position().unwrap();assert_eq!(cursor.x,3);assert_eq!(cursor.y,1); // Correct cell after one Cyrillic character would be x2.
}
}
'''
(dest/'src/lib.rs').write_text(s)
manifest=Path('/Users/themoretheless/.codex/tmp/rrrah-visualvault076-realmodel-20261021/Cargo.toml').read_text().replace('visualvault076-realmodel','visualvault076-render').replace('"rt-multi-thread"','"rt-multi-thread","sync"')+'\nratatui="=0.29.0"\n';(dest/'Cargo.toml').write_text(manifest)
log=root/'docs/research/dedup-500/visualvault076-ui-render-controls-20261021/run.log'
with log.open('w') as f:subprocess.run(['cargo','+1.98.1','test','--manifest-path',str(dest/'Cargo.toml'),'--target-dir',str(root/'.dedup-work/visualvault076-detector-target-20261021'),'controls::'],stdout=f,stderr=subprocess.STDOUT,check=True)
log.write_text(log.read_text().rstrip()+'\n')
