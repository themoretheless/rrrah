'use strict';
const assert=require('node:assert/strict');
const b=require(process.argv[2]);
function raster(width,height,value,alpha=255){return {width,height,data:Uint8Array.from(Array.from({length:width*height},(_,i)=>{const v=value(i%width,Math.floor(i/width));return [v,v,v,alpha];}).flat())};}
const base=raster(5,5,()=>0);const edge=raster(5,5,(x,y)=>y===4?255:0);
const evenBase=b.blockhashData(base,4,1),evenEdge=b.blockhashData(edge,4,1);
const weightedBase=b.blockhashData(base,4,2),weightedEdge=b.blockhashData(edge,4,2);
assert.equal(evenBase,evenEdge);assert.notEqual(weightedBase,weightedEdge);
const invalidDistance=b.hammingDistance('zz','00');assert.equal(invalidDistance,0);
const partial=raster(8,8,(x,y)=>(x*17+y*29)%256,1),opaque=raster(8,8,(x,y)=>(x*17+y*29)%256,255);
assert.equal(b.blockhashData(partial,4,2),b.blockhashData(opaque,4,2));
const invisibleA=raster(8,8,()=>0,0),invisibleB=raster(8,8,(x,y)=>(x*17+y*29)%256,0);
assert.equal(b.blockhashData(invisibleA,4,2),b.blockhashData(invisibleB,4,2));
console.log(JSON.stringify({status:'observed_original_upstream_edge_controls',method1_edge_omission:{base:evenBase,changed:evenEdge},method2_edge_included:{base:weightedBase,changed:weightedEdge},invalid_hex_distance:invalidDistance,partial_alpha_ignored_hash:b.blockhashData(partial,4,2),invisible_rgb_canonical_hash:b.blockhashData(invisibleA,4,2),weighted_fixtures:[base,edge].map(im=>({width:im.width,height:im.height,data:Array.from(im.data),hash:b.blockhashData(im,4,2)}))},null,2));
