# Unchanged original module; unavailable Faraday replaced with a rejecting adapter.
module Faraday
  def self.send(*)
    raise 'Network calls forbidden in authored offline control'
  end
end
$LOADED_FEATURES << 'faraday.rb'
load '.dedup-work/travis131-20261021/lib/travis_dedup.rb'
def row(id,pr,branch)
  {'id'=>id,'pull_request_number'=>pr,'branch'=>branch}
end
def check(name,rows,expected)
  actual=TravisDedup.send(:duplicate_builds,rows).map{|r|r.fetch('id')}
  raise "#{name}: #{actual.inspect}" unless actual==expected
  puts "PASS #{name} #{actual.inspect}"
end
TravisDedup.branches=nil
check('same PR keeps first incoming row',[row(1,7,'a'),row(2,7,'a')],[2])
check('reversed input keeps different build',[row(2,7,'a'),row(1,7,'a')],[1])
check('branch dedup disabled',[row(1,nil,'a'),row(2,nil,'a')],[])
TravisDedup.branches=true
check('branch dedup enabled',[row(1,nil,'a'),row(2,nil,'a')],[2])
check('distinct PR identifiers stay distinct',[row(1,7,'a'),row(2,8,'a')],[])
