require 'minitest/autorun'
require 'dedup'

class ReviewTest < Minitest::Test
  def test_every_binary_byte_preserves_content_encoding_and_intern_identity
    256.times do |byte|
      source = byte.chr(Encoding::BINARY) * 3
      a = Dedup::Ruby.deep_intern!(source.dup)
      b = Dedup::Ruby.deep_intern!(source.dup)
      assert_equal source.bytes, a.bytes
      assert_equal source.encoding, a.encoding
      assert_predicate a, :frozen?
      assert_same a, b
    end
  end

  def test_identity_hash_keys_collapse_after_interning
    a = String.new('same-key'); b = String.new('same-key')
    hash = {}.compare_by_identity
    hash[a] = :first; hash[b] = :second
    assert_equal 2, hash.size
    Dedup::Ruby.deep_intern!(hash)
    assert_equal 1, hash.size
    assert_equal [:second], hash.values
  end

  def test_frozen_container_skips_mutable_descendants
    child = String.new('mutable-child')
    outer = [child].freeze
    assert_same outer, Dedup::Ruby.deep_intern!(outer)
    refute_predicate child, :frozen?
    child << '-changed'
    assert_equal 'mutable-child-changed', outer.first
  end

  def test_cyclic_ruby_array_has_no_cycle_guard
    array = []; array << array
    assert_raises(SystemStackError) { Dedup::Ruby.deep_intern!(array) }
  end

  def test_nonempty_equal_arrays_keep_separate_identity
    a = Dedup::Ruby.deep_intern!([1]); b = Dedup::Ruby.deep_intern!([1])
    assert_equal a, b
    refute_same a, b
    assert_predicate a, :frozen?
  end

  def test_empty_hash_default_is_preserved_on_this_ruby_fallback
    hash = Hash.new(:fallback)
    result = Dedup::Ruby.deep_intern!(hash)
    assert_equal :fallback, result[:missing]
  end
end
