"""Whole upstream modules,3unchanged local tests, real dependencies/FFmpeg, owned fixtures only."""
from pathlib import Path
import contextlib,hashlib,importlib.util,io,json,math,os,shutil,subprocess,sys,tempfile
R=Path(__file__).resolve().parents[1];W=R/'.dedup-work/video21-20261016';S=W/'source';sys.path[:0]=[str(W/'deps'),str(R/'.dedup-work/near41-20261015/deps'),str(S)]
import numpy as np,imagehash,pytest
from PIL import Image
from videohash.videohash import VideoHash
from videohash.collagemaker import MakeCollage
from videohash.framesextractor import FramesExtractor
from videohash.videoduration import video_duration
from videohash.utils import does_path_exists,get_list_of_all_files_in_dir,create_and_return_temporary_directory
from videohash.exceptions import FFmpegFailedToExtractFrames,StoragePathDoesNotExist
from videohash.tilemaker import slicer
h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 obs=[];original=[];asset=S/'assets/rocket.mkv';meta=json.loads(subprocess.check_output(['ffprobe','-v','error','-count_frames','-show_streams','-show_format','-of','json',str(asset)],text=True));assert len(meta['streams'])==1 and int(meta['streams'][0]['nb_read_frames'])==3120;duration=float(meta['format']['duration']);assert abs(duration-52.079)<0.001
 def record(name,**data):obs.append({'control':name,**data})
 with tempfile.TemporaryDirectory(prefix='native-',dir=W) as tmp:
  root=Path(tmp);prior=tempfile.tempdir;tempfile.tempdir=tmp
  try:
   for filename,func in [('test_collagemaker.py','test_all'),('test_framesextractor.py','test_all'),('test_videoduration.py','test_video_duration')]:
    spec=importlib.util.spec_from_file_location('video21_'+filename[:-3],S/'tests'/filename);m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m);getattr(m,func)();original.append(filename+'::'+func)
   actual=video_duration(str(asset));assert abs(actual-duration)<0.01;record('actual_duration_matches_independent_ffprobe',native_seconds=actual,ffprobe_seconds=duration,decoded_frames=3120)
   storage=root/'storage';storage.mkdir()
   try:VideoHash(path=str(asset),storage_path=str(storage)+os.sep)
   except AttributeError as e:assert 'ANTIALIAS' in str(e);record('whole_native_pipeline_fails_modern_unpinned_pillow',exception='AttributeError: ANTIALIAS',pillow_version=Image.__version__)
   else:raise AssertionError('expected preserved modernPillow incompatibility')
   dirs=list(storage.glob('*/frames'));assert len(dirs)==1;frame_count=len(list(dirs[0].glob('*.jpeg')));assert frame_count>0 and frame_count<3120;record('native_extractor_samples_video_not_complete_frame_coverage',sampled_frames=frame_count,source_frames=3120,default_rate=1)
   try:VideoHash(path=str(asset),storage_path=str(storage))
   except StoragePathDoesNotExist:record('existing_directory_without_trailing_separator_rejected',exception='StoragePathDoesNotExist')
   else:raise AssertionError('expected trailing separator policy')
   bad=root/'bad.mkv';bad.write_bytes(b'not a video');out=root/'seeded';out.mkdir();(out/'existing.txt').write_text('unrelated existing file');FramesExtractor(str(bad),str(out)+os.sep);assert len(list(out.iterdir()))==1;record('failed_decode_accepted_when_output_dir_already_nonempty',generated_frames=0,existing_files=1)
   spaces=root/'folder with spaces';spaces.mkdir();copy=spaces/'video.mkv';shutil.copyfile(asset,copy);out=root/'spaceframes';out.mkdir()
   try:FramesExtractor(str(copy),str(out)+os.sep)
   except FFmpegFailedToExtractFrames:record('actual_space_path_double_quoting_fails',exception='FFmpegFailedToExtractFrames')
   else:raise AssertionError('expected space path quoting failure')
   try:video_duration(str(root/'missing.mkv'))
   except UnboundLocalError:record('missing_duration_regex_leaves_uninitialized_variable',exception='UnboundLocalError')
   else:raise AssertionError('expected missing duration failure')
   fake=VideoHash.__new__(VideoHash);fake.hash='0b'+'0'*64;fake.hash_hex='0x0';fake.bits_in_hash=64;fake.bitlist=[0]*64;fake.similar_percentage=15
   try:fake.hamming_distance(bitlist_a=[],bitlist_b=[])
   except UnboundLocalError:record('empty_bitlists_uninitialized_distance',exception='UnboundLocalError')
   else:raise AssertionError('expected empty bitlist failure')
   assert fake-[2]*64==64;record('nonbinary_list_values_accepted',nonbinary_value=2,reported_distance=64)
   assert fake-('0b'+'2'*64)==64;record('nonbinary_bitstring_digits_accepted',nonbinary_value=2,reported_distance=64)
   assert len(VideoHash.hex2bin('0x10000000000000000',64))-2==65;record('hex_conversion_padding_is_not_maximum_width',requested_bits=64,returned_bits=65)
   assert fake.is_similar('0b'+'1'*10+'0'*54) and not fake.is_similar('0b'+'1'*11+'0'*53);record('similarity_rounds_up_percentage_threshold',configured_percent=15,accepted_bits=10,actual_percent=15.625)
   folder=root/'listing';folder.mkdir();(folder/'a').write_text('x');paths=get_list_of_all_files_in_dir(str(folder));assert len(paths)==1 and not Path(paths[0]).exists();assert get_list_of_all_files_in_dir(str(folder)+os.sep)==[str(folder/'a')];record('list_helper_requires_trailing_separator',missing_separator_produces_nonexistent_path=True)
   original_limit=Image.MAX_IMAGE_PIXELS;png=root/'grid.png';Image.new('RGB',(9,9),'red').save(png);tiles=slicer(str(png),number_tiles=64);assert len(tiles)==1 and Image.MAX_IMAGE_PIXELS is None;Image.MAX_IMAGE_PIXELS=original_limit;record('small_nondivisible_grid_yields_one_tile_and_disables_pixel_limit',requested_tiles=64,actual_tiles=1,global_limit_disabled=True,limit_restored_by_control=True)
   # Explicit compatibility adapter reaches originalcollagebody without pretending defaultmodernenvironment passes.
   p1=root/'frame1.png';p2=root/'frame2.png';Image.new('RGB',(2,2),'red').save(p1);Image.new('RGB',(2,2),'blue').save(p2);assert not hasattr(Image,'ANTIALIAS');Image.ANTIALIAS=Image.Resampling.LANCZOS
   try:MakeCollage([str(p1),str(p2)],str(root/'collage.png'),collage_image_width=20)
   finally:del Image.ANTIALIAS
   im=np.asarray(Image.open(root/'collage.png'));colored=int(np.any(im!=0,axis=2).sum());assert im.shape[:2]==(40,20) and colored==8;record('thumbnail_never_upscales_into_larger_collage_cells',canvas_pixels=800,nonblack_pixels=8,adapter='Pillow ANTIALIAS name bound temporarily to real Resampling.LANCZOS; source unchanged; default failure retained')
   # Verify cleanup boundaries on createdownedstorage only.
   owned=root/'generated';task=owned/'temp_storage_dir'/'task';task.mkdir(parents=True);outside=root/'outside';outside.write_text('keep');o=VideoHash.__new__(VideoHash);o.storage_path=str(task)+os.sep;o._storage_path='';o.delete_storage_path();assert not owned.exists() and outside.exists();record('default_layout_cleanup_stays_in_owned_generated_parent',outside_preserved=True)
   base=root/'explicit';base.mkdir();task=base/'task';task.mkdir();other=base/'other';other.write_text('keep');o.storage_path=str(task)+os.sep;o._storage_path=str(base)+os.sep;o.delete_storage_path();assert not task.exists() and other.exists();record('explicit_storage_cleanup_preserves_sibling',sibling_preserved=True)
  finally:tempfile.tempdir=prior
 files=json.loads((W/'fetch.json').read_text())['tree']['tree'];pins={x['path']:h(S/x['path']) for x in files if x['type']=='blob'};print(json.dumps({'status':'passed_scoped_native_controls_with_preserved_full_pipeline_failure','original_local_test_functions':original,'observations':obs,'source_pins':pins,'dependencies':{'numpy':np.__version__,'Pillow':Image.__version__,'imagehash':imagehash.__version__,'pytest':pytest.__version__},'ffmpeg':subprocess.check_output(['ffmpeg','-version'],text=True).splitlines()[0],'ffmpeg_executable_sha256':h(shutil.which('ffmpeg')),'scope':'Whole unchanged upstream modules and3original localtestfunctions executed, actualFFmpeg/libVP9 publicownedasset/reference only. FullVideoHash reachesmodernPillowANTIALIAS failure retained. One declaredLANCZOSnamecompatibility adapter onlyforselectedcollagecontrol; no fullhash/remoteYouTube/downloader/originalenvironment/fullupstreamsuite/platform/visualquality/RSS/performance/adoption proof. Videoall3120frames andotherassets stillawaitfullvisual review.'},sort_keys=True))
if __name__=='__main__':main()
