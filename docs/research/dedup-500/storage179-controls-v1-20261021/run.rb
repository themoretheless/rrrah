# Original modules with authored Concern/ActiveJob/model adapters, no Rails/DB/storage.
require 'ostruct'
module ActiveSupport
  module Concern
    def class_methods(&body)
      const_set(:ClassMethods, Module.new(&body))
    end
  end
end
class Log
  def method_missing(*args); end
end
module Rails
  def self.logger; @logger ||= Log.new; end
end
module ActiveJob
  class Base
    def self.queue_as(*args); end
  end
end
module ActiveStorageDedup
  def self.enabled?; true; end
end
root=File.expand_path('../../../..',__dir__)
evidence=File.join(root,'docs/research/dedup-500/evidence/coderhs--active_storage_dedup')
load File.join(evidence,'03-blob_deduplication.rb')
load File.join(evidence,'04-deduplication_job.rb')
class BlobAdapter
  extend ActiveStorageDedup::BlobDeduplication::ClassMethods
  class << self
    attr_accessor :query
    def service; OpenStruct.new(name:'synthetic'); end
    def find_by(**args); self.query=args; OpenStruct.new(id:1,byte_size:1); end
  end
end
blob=BlobAdapter.create_before_direct_upload!(filename:'synthetic',byte_size:999,checksum:'authored-checksum')
raise unless blob.byte_size==1 && BlobAdapter.query=={checksum:'authored-checksum',service_name:'synthetic'}
puts 'PASS original direct-upload hook reuses metadata match despite differing supplied byte_size; no actual digest collision'
class Attachments
  attr_reader :blob_id
  def count; 2; end
  def update_all(blob_id:); @blob_id=blob_id; end
end
class Keeper
  attr_reader :id,:reference_count
  def initialize(fail); @id=1;@reference_count=0;@fail=fail; end
  def increment!(*args); raise 'authored counter failure' if @fail; @reference_count+=2; end
end
class Duplicate
  attr_reader :id,:attachments,:deleted
  def initialize; @id=2;@attachments=Attachments.new;@deleted=false; end
  def delete; @deleted=true; end
end
job=ActiveStorageDedup::DeduplicationJob.new
k=Keeper.new(true);d=Duplicate.new;job.send(:merge_duplicate,k,d)
raise unless d.attachments.blob_id==1 && !d.deleted && k.reference_count==0
puts 'PASS injected counter failure leaves moved attachments with duplicate record retained; error swallowed'
k=Keeper.new(false);d=Duplicate.new;job.send(:merge_duplicate,k,d)
raise unless d.attachments.blob_id==1 && d.deleted && k.reference_count==2
puts 'PASS successful control moves attachments, updates counter and deletes duplicate record'
puts '3 original-method controls passed; no Rails, transaction engine, database, uploads or stored file operations'
