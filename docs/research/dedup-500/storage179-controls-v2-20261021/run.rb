# Original configuration and declaration methods, authored attachment superclass only.
root=File.expand_path('../../../..',__dir__)
source=File.join(root,'.dedup-work/storage179-20261021/lib/active_storage_dedup')
load File.join(source,'configuration.rb')
load File.join(source,'attachment_options.rb')
class AttachmentBase
  def self.has_one_attached(*args,**kwargs); end
  def self.has_many_attached(*args,**kwargs); end
end
class ConfiguredModel < AttachmentBase
  extend ActiveStorageDedup::AttachmentOptions
end
class ChildConfiguredModel < ConfiguredModel; end
ActiveStorageDedup.configuration.deduplicate_by_default=false
raise unless ActiveStorageDedup.deduplicate_enabled_for?(ConfiguredModel.new,:unregistered)==false
puts 'PASS unregistered attachment follows disabled global default'
ConfiguredModel.has_one_attached(:asset)
raise unless ActiveStorageDedup.deduplicate_enabled_for?(ConfiguredModel.new,:asset)==true
puts 'PASS omitted declaration option registers true and overrides disabled global default'
ConfiguredModel.has_many_attached(:private_files,deduplicate:false)
raise unless ActiveStorageDedup.deduplicate_enabled_for?(ConfiguredModel.new,:private_files)==false
ActiveStorageDedup.configuration.deduplicate_by_default=true
raise unless ActiveStorageDedup.deduplicate_enabled_for?(ChildConfiguredModel.new,:private_files)==true
puts 'PASS exact-class lookup does not inherit parent false setting under authored subclass'
ActiveStorageDedup.configuration.enabled=false
raise unless ActiveStorageDedup.deduplicate_enabled_for?(ConfiguredModel.new,:asset)==false
puts 'PASS global master switch overrides registered true'
puts '4 controls passed; no Rails inheritance/association registration or storage lifecycle executed'
