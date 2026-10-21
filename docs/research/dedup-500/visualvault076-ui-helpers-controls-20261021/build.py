from pathlib import Path
import subprocess
root=Path.cwd();cache=root/'.dedup-work/visualvault076-20261021';old=Path('/Users/themoretheless/.codex/tmp/rrrah-visualvault076-organizer-20261021');dest=Path('/Users/themoretheless/.codex/tmp/rrrah-visualvault076-uihelpers-20261021');(dest/'src').mkdir(parents=True,exist_ok=True)
s=(old/'src/lib.rs').read_text();settings=(cache/'crates/ui/src/settings.rs').read_text();dashboard=(cache/'crates/ui/src/dashboard.rs').read_text()
s+='\nmod settings_helpers {use super::Settings;use std::path::Path;\n'+settings[settings.index('fn get_preview_path('):settings.index('fn get_enhanced_border_style(')]+settings[settings.index('fn truncate_path('):]+'}\n'
s+='\nmod dashboard_helpers {\n'+dashboard[dashboard.index('fn truncate_path('):dashboard.index('// Helper function to format numbers')].replace('fn truncate_path(', 'pub(super) fn truncate_path(')+'}\n'
# Visibility is adapter-only so tests can call exact helper bodies.
s=s.replace('fn get_preview_path(', 'pub(super) fn get_preview_path(').replace('fn truncate_path(path: String', 'pub(super) fn truncate_path(path: String')
s+='''
#[cfg(test)] mod controls {use super::*;use chrono::TimeZone;
#[test] fn control_settings_unicode_truncation_panics() {let path=format!("/{}","я".repeat(60));assert!(std::panic::catch_unwind(||settings_helpers::truncate_path(path,60)).is_err());}
#[test] fn control_dashboard_unicode_truncation_panics() {let path=format!("/{}","я".repeat(30));assert!(std::panic::catch_unwind(||dashboard_helpers::truncate_path(&path,40)).is_err());}
#[test] fn control_video_yearly_preview_disagrees_with_organizer() {let mut s=settings("/destination".into());s.organize_by="yearly".into();s.separate_videos=true;let mut file=media("video.mp4".into());file.file_type=FileType::Video;file.modified=chrono::Local.with_ymd_and_hms(2024,3,15,12,0,0).unwrap();let preview=settings_helpers::get_preview_path(&s,"video.mp4","video");let actual=FileOrganizer::determine_target_directory(&file,Path::new("/destination"),&s).unwrap().join("video.mp4");assert_eq!(preview,"/destination/2024/video.mp4");assert_eq!(actual,PathBuf::from("/destination/Videos/2024/video.mp4"));assert_ne!(PathBuf::from(preview),actual);}
}
'''
(dest/'src/lib.rs').write_text(s);(dest/'src/settings.rs').write_text((old/'src/settings.rs').read_text());(dest/'Cargo.toml').write_text((old/'Cargo.toml').read_text().replace('visualvault076-organizer','visualvault076-uihelpers'))
log=root/'docs/research/dedup-500/visualvault076-ui-helpers-controls-20261021/run.log'
with log.open('w') as f:subprocess.run(['cargo','+1.98.1','test','--manifest-path',str(dest/'Cargo.toml'),'--target-dir',str(root/'.dedup-work/visualvault076-detector-target-20261021'),'controls::'],stdout=f,stderr=subprocess.STDOUT,check=True)
log.write_text(log.read_text().rstrip()+'\n')
