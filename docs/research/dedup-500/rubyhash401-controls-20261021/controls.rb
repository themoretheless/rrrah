require 'image_hash'
raise 'length asymmetry not observed' unless ImageHash.distance('00','0000') == 0 && ImageHash.distance('0000','00') == 8
puts 'PASS unequal digest lengths produce asymmetric distances0 and8'
raise 'invalid hex policy differs' unless ImageHash.distance('zz','00') == 0
puts 'PASS invalid hex zz coerces to00 and distance0'
raise 'empty policy differs' unless ImageHash.binary_distance('','').nil?
puts 'PASS empty binary distance returnsnil rather than integer0 or refusal'
