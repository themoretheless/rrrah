// Unchanged original parse_size plus authored controls.
pub fn parse_size (
	size_string: & str,
) -> Result <u64, String> {

	let (multiplier, suffix_length) =

		if size_string.ends_with ("KiB") {
			(1024, 3)
		} else if size_string.ends_with ("MiB") {
			(1024 * 1024, 3)
		} else if size_string.ends_with ("GiB") {
			(1024 * 1024 * 1024, 3)
		} else if size_string.ends_with ("TiB") {
			(1024 * 1024 * 1024 * 1024, 3)

		} else if size_string.ends_with ("KB") {
			(1000, 2)
		} else if size_string.ends_with ("MB") {
			(1000 * 1000, 2)
		} else if size_string.ends_with ("GB") {
			(1000 * 1000 * 1000, 2)
		} else if size_string.ends_with ("TB") {
			(1000 * 1000 * 1000 * 1000, 2)

		} else if size_string.ends_with ("K") {
			(1024, 1)
		} else if size_string.ends_with ("M") {
			(1024 * 1024, 1)
		} else if size_string.ends_with ("G") {
			(1024 * 1024 * 1024, 1)
		} else if size_string.ends_with ("T") {
			(1024 * 1024 * 1024 * 1024, 1)

		} else if size_string.ends_with ("B") {
			(1, 1)

		} else {

			return Err (
				"Units not specified or recognised".to_owned ());

		};

	let quantity_string =
		& size_string [
			0 ..
			size_string.len () - suffix_length];

	let quantity_integer =
		try! (
			quantity_string.parse::<u64> (
			).map_err (
				|_|
				"Unable to parse integer value".to_owned ()
			));

	Ok (
		multiplier * quantity_integer
	)

}

// ex: noet ts=4 filetype=rust

#[test]fn binary_decimal_units(){assert_eq!(parse_size("2KiB"),Ok(2048));assert_eq!(parse_size("2KB"),Ok(2000));}
#[test]fn zero_allowed(){assert_eq!(parse_size("0B"),Ok(0));}
#[test]fn unknown_units_refused(){assert!(parse_size("20").is_err());}
#[test]fn overflow_profile(){let r=std::panic::catch_unwind(||parse_size("18446744073709551615KiB"));if cfg!(debug_assertions){assert!(r.is_err());}else{assert_eq!(r.unwrap(),Ok(u64::MAX-1023));}}
