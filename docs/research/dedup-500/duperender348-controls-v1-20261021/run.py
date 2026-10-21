"""Original function AST, authored scene/matrix adapters; no Blender/render IO."""
from pathlib import Path
from types import SimpleNamespace as N
import ast,hashlib
R=Path(__file__).resolve().parents[4]
p=R/'docs/research/dedup-500/evidence/samytichadou--dupe_render/03-hash_operator.py'
t=ast.parse(p.read_text());t.body=[n for n in t.body if isinstance(n,ast.FunctionDef)]
ns={'hashlib':hashlib};exec(compile(t,'original-hash-functions','exec'),ns)
a=[[1.0,23.0,0.0,0.0],[0.0,1.0,0.0,0.0],[0.0,0.0,1.0,0.0],[0.0,0.0,0.0,1.0]]
b=[row[:] for row in a];b[0][:2]=[1.02,3.0]
assert a!=b
obj=lambda matrix:N(evaluated_get=lambda dg:N(matrix_world=matrix))
assert ns['world_matrix_to_hash'](obj(a),None)==ns['world_matrix_to_hash'](obj(b),None)
print('PASS different synthetic finite matrices share delimiter-free serialization and hash (not MD5 collision)')
c=[row[:] for row in a];c[0][0]=2.0
assert ns['world_matrix_to_hash'](obj(a),None)!=ns['world_matrix_to_hash'](obj(c),None)
print('PASS ordinary matrix change positive control differs')
def setup():
 state=N(ended=False)
 scene=N(frame_start=1,frame_end=3,frame_current=7,use_audio_scrub=True,
 duperender_properties=N(process_only_visible_object_modifiers=True))
 ctx=N(scene=scene,window_manager=N(progress_begin=lambda *a:None,progress_update=lambda *a:None,
 progress_end=lambda:setattr(state,'ended',True)))
 ns['get_valid_specific_ranges']=lambda:([],[])
 return ctx,state
ctx,state=setup()
def fail(context):
 if context.scene.frame_current==2:raise RuntimeError('authored hash failure')
 return 'a'
ns['get_frame_hash']=fail
try:ns['get_frames_to_render'](ctx)
except RuntimeError:pass
else:raise AssertionError('Expected injected failure')
assert ctx.scene.frame_current==2 and not ctx.scene.use_audio_scrub and not state.ended
print('PASS original frame loop leaves changed frame/audio/progress after injected hash exception')
ctx,state=setup();ns['get_frame_hash']=lambda context:{1:'a',2:'a',3:'b'}[context.scene.frame_current]
assert ns['get_frames_to_render'](ctx)==([1,3],[2])
assert ctx.scene.frame_current==7 and ctx.scene.use_audio_scrub and state.ended
print('PASS successful loop classifies consecutive duplicate and restores state')
print('4 original-function controls passed; Blender scene evaluation, actual renders and filesystem writes NOT executed')
