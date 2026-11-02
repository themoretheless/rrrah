const assert=require('node:assert/strict');
let data=null;const clusterMap=new Map();
function classifyCluster(cluster){
  return cluster.similarity>=0.90?'duplicate':'related';
}
function buildClusterMap(){
  clusterMap.clear();
  if(!data||!data.duplicateClusters)return;
  data.duplicateClusters.forEach(cluster=>{
    const type=classifyCluster(cluster);
    cluster.prs.forEach(pr=>{
      clusterMap.set(pr.number,{clusterId:cluster.id,type:type});
    });
  });
}
function mapAPIPR(p,repoName){
  return{
    _repo:repoName||'',
    number:p.pr_number||p.number,
    title:p.title||'',
    author:p.author||'',
    totalScore:p.total_score||p.totalScore||0,
    isSpam:p.is_spam||p.isSpam||false,
    isDraft:p.is_draft||p.isDraft||false,
    filesChanged:p.files_changed||p.filesChanged||0,
    ageInDays:p.age_days||p.ageInDays,
    mergeable:p.mergeable||'',
    milestone:p.milestone||'',
    requestedReviewers:p.requested_reviewers||p.requestedReviewers||0,
    llmScore:p.llm_score||p.llmScore,
    visionScore:p.vision_score||p.visionScore,
    visionAlignment:p.vision_alignment||p.visionAlignment||'unchecked',
    changedFiles:p.changed_files||p.changedFiles||[],
    body:p.body||'',
  };
}
const mapped=mapAPIPR({number:1,llm_score:0,vision_score:0,age_days:0},'synthetic/repo');
assert.equal(mapped.llmScore,undefined);assert.equal(mapped.visionScore,undefined);assert.equal(mapped.ageInDays,undefined);
assert.equal(classifyCluster({similarity:0.89,type:'duplicate'}),'related');
data={duplicateClusters:[{id:1,similarity:.95,prs:[{number:7,_repo:'one/repo'}]},{id:2,similarity:.81,prs:[{number:7,_repo:'two/repo'}]}]};buildClusterMap();assert.equal(clusterMap.size,1);assert.equal(clusterMap.get(7).clusterId,2);
console.log(JSON.stringify({controls:3,passed:3,observations:['snake_case zero scores/age become undefined','hardcoded .90 threshold overrides provided cluster type','same PR number across repositories overwrites cluster map'],scope:'three original inline function bodies extracted; synthetic values; no DOM/browser/render/network'}));
