use sqlxo_traits::{
	JoinPath,
	JoinSegment,
};

fn segments_match(a: &JoinSegment, b: &JoinSegment) -> bool {
	a.descriptor == b.descriptor
}

fn path_matches(a: &JoinPath, b: &JoinPath) -> bool {
	let a_segments = a.segments();
	let b_segments = b.segments();
	if a_segments.len() != b_segments.len() {
		return false;
	}

	a_segments
		.iter()
		.zip(b_segments.iter())
		.all(|(left, right)| segments_match(left, right))
}

fn path_starts_with(path: &JoinPath, prefix: &JoinPath) -> bool {
	let prefix_segments = prefix.segments();
	if prefix_segments.is_empty() {
		return true;
	}

	let path_segments = path.segments();
	if prefix_segments.len() > path_segments.len() {
		return false;
	}

	path_segments
		.iter()
		.zip(prefix_segments.iter())
		.all(|(left, right)| segments_match(left, right))
}

fn alias_for_path(path: &JoinPath) -> String {
	path.alias()
}

pub fn ensure_join_alias(
	joins: Option<&[JoinPath]>,
	required: &JoinPath,
	label: &'static str,
) -> String {
	if required.is_empty() {
		panic!(
			"full-text search join `{}` does not define a join path",
			label
		);
	}

	let Some(paths) = joins else {
		panic!(
			"full-text search join `{}` requires a matching `.join(...)` or \
			 `.join_path(...)` call",
			label
		);
	};

	let required_len = required.len();

	for existing in paths {
		if path_matches(existing, required) {
			return alias_for_path(existing);
		}

		if path_starts_with(existing, required) {
			return existing.alias_prefix(required_len);
		}
	}

	panic!(
		"full-text search join `{}` requires a matching `.join(...)` or \
		 `.join_path(...)` call",
		label
	);
}

pub fn nested_join_paths(
	joins: Option<&[JoinPath]>,
	prefix: &JoinPath,
) -> Option<Vec<JoinPath>> {
	let paths = joins?;

	let prefix_len = prefix.len();
	let mut nested: Vec<JoinPath> = Vec::new();

	for path in paths {
		if !path_starts_with(path, prefix) {
			continue;
		}

		if let Some(remainder) = path.strip_prefix(prefix_len) {
			if remainder.is_empty() {
				continue;
			}
			nested.push(remainder);
		}
	}

	if nested.is_empty() {
		None
	} else {
		Some(nested)
	}
}
