// Original status-decision block; synthetic response/value adapter and authored tests.
#[allow(dead_code)] struct DsmResp<T>{success:bool,data:Option<T>}
struct Value{finished:bool}
impl Value{fn get(&self,k:&str)->Option<&Self>{if k=="finished"{Some(self)}else{None}} fn as_bool(&self)->Option<bool>{Some(self.finished)}}
fn observed(status:Result<DsmResp<Value>,()>)->Vec<()> {let errors=vec![];
            if let Ok(s) = status {
                if s.data.and_then(|d| d.get("finished").and_then(|v| v.as_bool())).unwrap_or(false) {
                    return errors; // empty = all succeeded
                }
            }
vec![()] // adapter sentinel: original block did not return success
}
#[test]fn successful_finished(){assert!(observed(Ok(DsmResp{success:true,data:Some(Value{finished:true})})).is_empty());}
#[test]fn failure_envelope_with_finished_also_accepted(){assert!(observed(Ok(DsmResp{success:false,data:Some(Value{finished:true})})).is_empty());}
#[test]fn unfinished_is_not_accepted(){assert_eq!(observed(Ok(DsmResp{success:true,data:Some(Value{finished:false})})).len(),1);}
