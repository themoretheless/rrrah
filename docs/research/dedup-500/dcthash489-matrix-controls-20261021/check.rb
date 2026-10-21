require 'json'
require 'digest'
root='/Users/themoretheless/.codex/tmp/rrrah-dcthash-full-source-20261013'
require File.join(root,'lib/dcthash')
# Canonical direct two-dimensional DCT: result[vertical_frequency][horizontal_frequency].
n=8
input=Array.new(n){|y|Array.new(n){|x|((x*37+y*19+x*y*11)%251).to_f}}
expected=Array.new(n){|v|Array.new(n){|u|(0...n).sum{|y|(0...n).sum{|x|input[y][x]*Math.cos(Math::PI*(2*x+1)*u/(2*n))*Math.cos(Math::PI*(2*y+1)*v/(2*n))}}}}
actual=Dcthash.send(:dct_2d,input)
error=(0...n).flat_map{|y|(0...n).map{|x|(actual[y][x]-expected[x][y]).abs}}.max
untransposed=(0...n).flat_map{|y|(0...n).map{|x|(actual[y][x]-expected[y][x]).abs}}.max
raise 'DCT mismatch' unless error<1e-8 && untransposed>1
bit_checks=(0...64).map{|i|raise 'bit mismatch' unless Dcthash.distance('0',(1<<i).to_s(16))==1;i}.length
fixtures=JSON.parse(File.read(File.join(root,'spec/fixtures/hashes.json')))
pairs=[]
fixtures.each_with_index{|a,i|fixtures.each_with_index{|b,j|next if i>=j
 x=a['hash'].to_i(16);y=b['hash'].to_i(16)
 oracle=(0...64).count{|bit|((x>>bit)&1)!=((y>>bit)&1)}
 got=Dcthash.distance(a['hash'],b['hash']);raise 'distance mismatch' unless got==oracle
 pairs << {'a'=>a['file'],'b'=>b['file'],'distance'=>got}
}}
result={'ruby'=>RUBY_DESCRIPTION,'source_sha256'=>Digest::SHA256.file(File.join(root,'lib/dcthash.rb')).hexdigest,'dct_matrix'=>{'size'=>n,'maximum_error_against_transposed_direct_oracle'=>error,'maximum_error_against_untransposed_oracle'=>untransposed,'interpretation'=>'dct_2d returns horizontal frequency as outer dimension; valid transpose convention, recipe compatibility must account for bit order'},'single_bit_positions_checked'=>bit_checks,'golden_hash_pair_distances'=>pairs,'limits'=>['Ruby2.6 below gem required>=2.7; this is unchanged arithmetic only, no gem compatibility qualification.','No RMagick, image hashing, original RSpec, decoding or visual review performed.','Golden hash distances do not independently recompute image hashes.']}
File.write(File.join(__dir__,'result.json'),JSON.pretty_generate(result)+"\n")
puts JSON.generate({'dct_error'=>error,'transpose_difference'=>untransposed,'bit_positions'=>bit_checks,'pairs'=>pairs.length})
