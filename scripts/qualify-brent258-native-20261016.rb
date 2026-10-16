require 'json'
require 'digest'
gem 'builder', '= 3.0.0'
gem 'multi_json', '= 1.3.6'
gem 'i18n', '= 0.6.0'
gem 'activesupport', '= 3.2.5'
gem 'activemodel', '= 3.2.5'
require 'active_support/all'
require 'active_model'
$LOAD_PATH.unshift(File.join(ARGV.fetch(0), 'source/lib'))
require 'dedupe'
def model
 Class.new do
  include ActiveModel::Validations
  include Dedupe
  attr_accessor :duplicate_state
  def duplicate?; duplicate_state; end
 end
end
def verify(name, observed, expected, rows)
 raise "#{name}: #{observed.inspect} != #{expected.inspect}" unless observed == expected
 rows << {name: name, observed: observed, expected: expected}
end
def run
 rows=[]
 type=model;type.validates_uniqueness(:using=>:owned_finder)
 r=type.new;r.duplicate_state=false
 verify('unique_record_valid',r.valid?,true,rows)
 r.duplicate_state=true
 verify('duplicate_record_invalid',r.valid?,false,rows)
 verify('actual_active_model_base_error',r.errors[:base],['Duplicates existing record'],rows)
 r.valid?
 verify('validation_resets_errors',r.errors[:base].size,1,rows)
 def r.duplicate?;raise IOError,'owned synthetic lookup failure';end
 exception=begin;r.valid?;nil;rescue=>e;e.class.name;end
 verify('lookup_failure_propagates',exception,'IOError',rows)
 options={:using=>:owned_other_finder,:message=>'CUSTOM_OWNED_MESSAGE'}
 type.validates_uniqueness(options)
 verify('macro_mutates_caller_options',options,{:message=>'CUSTOM_OWNED_MESSAGE'},rows)
 r2=type.new;r2.duplicate_state=true;r2.valid?
 verify('custom_message_option_ignored',r2.errors[:base],['Duplicates existing record'],rows)
 parent=model;first=Class.new(parent);second=Class.new(parent)
 first.validates_uniqueness(:using=>:first_finder)
 second.validates_uniqueness(:using=>:second_finder)
 verify('inherited_cattr_scope_shared',[parent.dedupe_scope_name,first.dedupe_scope_name,second.dedupe_scope_name],[:second_finder,:second_finder,:second_finder],rows)
 absent=model;absent.send(:undef_method,:duplicate?);absent.validates_uniqueness(:using=>:owned_finder)
 exception=begin;absent.new.valid?;nil;rescue=>e;e.class.name;end
 verify('missing_orm_predicate_fails',exception,'NoMethodError',rows)
 rows
end
runs=[run,run];raise 'Nonrepeatable' unless runs[0]==runs[1]
dependencies=Gem.loaded_specs.values.sort_by(&:name).map{|s|{name:s.name,version:s.version.to_s,root:s.full_gem_path}}
puts JSON.generate({runtime:RUBY_DESCRIPTION,runs:runs,dependencies:dependencies,loaded_files:$LOADED_FEATURES.select{|p|p.include?('/gems/')||p.include?('/source/lib/')}.map{|p|{path:p,sha256:Digest::SHA256.file(p).hexdigest}}})
