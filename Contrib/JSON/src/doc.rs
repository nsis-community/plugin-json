//! Everything that does not touch the installer: key paths, lookup, rendering
//! and the setters. Plain functions, so they are tested directly.

use jsonc_parser::ParseOptions;
use jsonc_parser::cst::{CstInputValue, CstNode, CstRootNode};

/// A failure, as the message `LastError` will report.
pub type Res<T> = Result<T, String>;

/// One step of a key path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Seg {
	/// An object key, already unquoted.
	Key(String),
	/// An array index.
	Index(usize),
}

/// JSONC: JSON plus comments and trailing commas. The JSON5 extras stay off,
/// so the raw strings and numbers `Get` passes through are always valid JSON.
fn options() -> ParseOptions {
	ParseOptions {
		allow_comments: true,
		allow_trailing_commas: true,
		allow_loose_object_property_names: false,
		allow_missing_commas: false,
		allow_single_quoted_strings: false,
		allow_hexadecimal_numbers: false,
		allow_unary_plus_numbers: false,
		allow_bare_decimal_point_numbers: false,
		allow_non_finite_numbers: false,
		allow_extended_string_escapes: false,
	}
}

/// Parses a whole document.
pub fn parse(text: &str) -> Res<CstRootNode> {
	CstRootNode::parse(text, &options()).map_err(|e| e.to_string())
}

/// Parses a value written in JSON, such as `[1, {"a": 2}]`, for `SetRaw`.
pub fn literal(s: &str) -> Res<CstInputValue> {
	let root = parse(s).map_err(|e| format!("`{s}` is not a JSON value: {e}"))?;
	let value = root
		.value()
		.ok_or_else(|| format!("`{s}` is not a JSON value"))?;
	Ok(input(&value))
}

/// A parsed node as a value to insert elsewhere. Comments inside it are lost.
fn input(node: &CstNode) -> CstInputValue {
	if let Some(o) = node.as_object() {
		CstInputValue::Object(
			o.properties()
				.iter()
				.filter_map(|p| Some((p.decoded_name()?, input(&p.value()?))))
				.collect(),
		)
	} else if let Some(a) = node.as_array() {
		CstInputValue::Array(a.elements().iter().map(input).collect())
	} else if let Some(s) = node.as_string_lit() {
		CstInputValue::String(s.decoded_value().unwrap_or_default())
	} else if let Some(b) = node.as_boolean_lit() {
		CstInputValue::Bool(b.value())
	} else if let Some(n) = node.as_number_lit() {
		CstInputValue::Number(n.to_string())
	} else {
		CstInputValue::Null
	}
}

/// Parses `servers[1].host`, `env."app.mode"`, `[0].name` and the like. The
/// empty path is the document root.
///
/// A bare key runs up to the next `.`, `[`, `]` or `"`, so it may hold spaces.
/// A quoted key takes `\"` and `\\` for a quote and a backslash.
pub fn parse_path(path: &str) -> Res<Vec<Seg>> {
	let c: Vec<char> = path.chars().collect();
	let mut segs = Vec::new();
	let mut need_key = false;
	let mut i = 0;
	while i < c.len() {
		let key_here = need_key || segs.is_empty();
		match c[i] {
			'.' if !key_here => {
				need_key = true;
				i += 1;
			}
			'[' if !need_key => {
				let close = c[i..]
					.iter()
					.position(|&x| x == ']')
					.map(|p| i + p)
					.ok_or_else(|| format!("unclosed `[` in path `{path}`"))?;
				let digits: String = c[i + 1..close].iter().collect();
				if digits.is_empty() || !digits.bytes().all(|d| d.is_ascii_digit()) {
					return Err(format!(
						"`[{digits}]` is not an array index in path `{path}`"
					));
				}
				let index = digits
					.parse()
					.map_err(|_| format!("index `{digits}` is too large in path `{path}`"))?;
				segs.push(Seg::Index(index));
				i = close + 1;
			}
			'"' if key_here => {
				let mut key = String::new();
				i += 1;
				loop {
					match c.get(i) {
						None => return Err(format!("unterminated quote in path `{path}`")),
						Some('"') => break,
						Some('\\') => {
							i += 1;
							key.extend(c.get(i));
						}
						Some(&x) => key.push(x),
					}
					i += 1;
				}
				i += 1;
				segs.push(Seg::Key(key));
				need_key = false;
			}
			x if key_here && !is_delimiter(x) => {
				let start = i;
				while i < c.len() && !is_delimiter(c[i]) {
					i += 1;
				}
				segs.push(Seg::Key(c[start..i].iter().collect()));
				need_key = false;
			}
			x => return Err(format!("unexpected `{x}` in path `{path}`")),
		}
	}
	if need_key {
		return Err(format!("path `{path}` ends with `.`"));
	}
	Ok(segs)
}

fn is_delimiter(c: char) -> bool {
	matches!(c, '.' | '[' | ']' | '"')
}

/// A path as a script would write it, for messages.
fn show(path: &[Seg]) -> String {
	let mut out = String::new();
	for seg in path {
		match seg {
			Seg::Key(k) => {
				if !out.is_empty() {
					out.push('.');
				}
				if k.is_empty() || k.chars().any(is_delimiter) {
					out.push('"');
					out.push_str(&k.replace('\\', "\\\\").replace('"', "\\\""));
					out.push('"');
				} else {
					out.push_str(k);
				}
			}
			Seg::Index(n) => out.push_str(&format!("[{n}]")),
		}
	}
	if out.is_empty() {
		"the root".into()
	} else {
		format!("`{out}`")
	}
}

fn not_found(path: &[Seg]) -> String {
	format!("{} not found", show(path))
}

fn child(node: &CstNode, seg: &Seg) -> Option<CstNode> {
	match seg {
		Seg::Key(k) => node.as_object()?.get(k)?.value(),
		Seg::Index(n) => node.as_array()?.elements().get(*n).cloned(),
	}
}

/// Resolves a path without creating anything.
pub fn lookup(root: &CstRootNode, path: &[Seg]) -> Res<CstNode> {
	let mut node = root
		.value()
		.ok_or_else(|| "the document is empty".to_string())?;
	for i in 0..path.len() {
		node = child(&node, &path[i]).ok_or_else(|| not_found(&path[..=i]))?;
	}
	Ok(node)
}

/// The JSON type name `Type` reports. Numbers without a fraction or an
/// exponent are `integer`, the rest `float`.
pub fn type_name(node: &CstNode) -> &'static str {
	if node.as_object().is_some() {
		"object"
	} else if node.as_array().is_some() {
		"array"
	} else if node.as_string_lit().is_some() {
		"string"
	} else if node.as_boolean_lit().is_some() {
		"boolean"
	} else if let Some(n) = node.as_number_lit() {
		if n.to_string().contains(['.', 'e', 'E']) {
			"float"
		} else {
			"integer"
		}
	} else {
		"null"
	}
}

fn not_a(node: &CstNode, path: &[Seg], wanted: &str) -> String {
	let name = type_name(node);
	let article = match name {
		"array" | "integer" | "object" => "an",
		_ => "a",
	};
	format!("{} is {article} {name}, not {wanted}", show(path))
}

/// Array length or number of object keys.
pub fn count(node: &CstNode, path: &[Seg]) -> Res<usize> {
	if let Some(o) = node.as_object() {
		Ok(o.properties().len())
	} else if let Some(a) = node.as_array() {
		Ok(a.elements().len())
	} else {
		Err(not_a(node, path, "an array or an object"))
	}
}

/// The `i`th entry: its key (the index, for arrays) and its value.
pub fn entry(node: &CstNode, path: &[Seg], i: usize) -> Res<(String, CstNode)> {
	let found = if let Some(o) = node.as_object() {
		o.properties()
			.get(i)
			.and_then(|p| Some((p.decoded_name()?, p.value()?)))
	} else if let Some(a) = node.as_array() {
		a.elements().get(i).map(|v| (i.to_string(), v.clone()))
	} else {
		return Err(not_a(node, path, "an array or an object"));
	};
	found.ok_or_else(|| format!("index {i} is out of range for {}", show(path)))
}

/// A value as a script sees it: strings decoded, numbers exactly as written
/// (so integers beyond 64 bits survive), containers as compact JSON.
pub fn render(node: &CstNode) -> String {
	match node.as_string_lit() {
		Some(s) => s.decoded_value().unwrap_or_default(),
		None => compact(node),
	}
}

/// One-line JSON without comments. Strings and keys keep their original
/// escapes, which are valid JSON because the parser rejects JSON5's.
fn compact(node: &CstNode) -> String {
	if let Some(o) = node.as_object() {
		let props: Vec<String> = o
			.properties()
			.iter()
			.filter_map(|p| {
				Some(format!(
					"{}:{}",
					p.name()?.as_string_lit()?,
					compact(&p.value()?)
				))
			})
			.collect();
		format!("{{{}}}", props.join(","))
	} else if let Some(a) = node.as_array() {
		let elements: Vec<String> = a.elements().iter().map(compact).collect();
		format!("[{}]", elements.join(","))
	} else {
		node.to_string()
	}
}

/// `nsishelper_str_to_ptr` from `Contrib/ExDLL/pluginapi.c`, widened to 64
/// bits.
///
/// `nsis_plugin::int::str_to_ptr` returns `isize`, which is 32 bits in an x86
/// installer and would silently wrap 64-bit integers. Everything else is the
/// same: `0x` hex, leading-zero octal, a sign only on decimal, stop at the first
/// unrecognised character, wrap on overflow, garbage is `0`. The window is
/// `popintptr`'s `TCHAR buf[128]`.
pub fn str_to_i64(s: &str) -> i64 {
	let s: Vec<u32> = s.chars().take(127).map(u32::from).collect();
	let at = |i: usize| s.get(i).copied().unwrap_or(0);
	let digit = |c: u32| char::from_u32(c).and_then(|c| c.to_digit(16));
	let zero = u32::from(b'0');
	let mut v: i64 = 0;

	if at(0) == zero && (at(1) == u32::from(b'x') || at(1) == u32::from(b'X')) {
		let mut i = 2;
		while let Some(d) = digit(at(i)) {
			v = (v << 4).wrapping_add(i64::from(d));
			i += 1;
		}
		return v;
	}

	if at(0) == zero && (zero..=u32::from(b'7')).contains(&at(1)) {
		let mut i = 1;
		while (zero..=u32::from(b'7')).contains(&at(i)) {
			v = (v << 3).wrapping_add(i64::from(at(i) - zero));
			i += 1;
		}
		return v;
	}

	let negative = at(0) == u32::from(b'-');
	let mut i = usize::from(negative);
	while (zero..=u32::from(b'9')).contains(&at(i)) {
		v = v.wrapping_mul(10).wrapping_add(i64::from(at(i) - zero));
		i += 1;
	}
	if negative { v.wrapping_neg() } else { v }
}

/// Writes `value` at `path`, creating missing objects and arrays on the way.
/// The empty path replaces the whole document.
///
/// The tree may be half-changed on failure; the caller keeps the old text.
pub fn set(root: &CstRootNode, path: &[Seg], value: CstInputValue) -> Res<()> {
	let Some((_, parents)) = path.split_last() else {
		root.set_value(value);
		return Ok(());
	};
	let mut node = match root.value() {
		Some(node) => node,
		None => {
			root.set_value(empty_for(&path[0], &[])?);
			root.value().ok_or("the root could not be created")?
		}
	};
	for (i, seg) in parents.iter().enumerate() {
		node = match child(&node, seg) {
			Some(next) => next,
			None => put(&node, &path[..=i], empty_for(&path[i + 1], &path[..=i])?)?,
		};
	}
	put(&node, path, value).map(drop)
}

/// The empty container that `next` needs. `path` is where it goes.
fn empty_for(next: &Seg, path: &[Seg]) -> Res<CstInputValue> {
	match next {
		Seg::Key(_) => Ok(CstInputValue::Object(Vec::new())),
		Seg::Index(0) => Ok(CstInputValue::Array(Vec::new())),
		Seg::Index(n) => Err(out_of_range(*n, 0, path)),
	}
}

fn out_of_range(n: usize, len: usize, path: &[Seg]) -> String {
	format!(
		"index {n} is out of range for {}, which has {len} element(s); use {len} to append",
		show(path)
	)
}

/// Writes `value` into `parent` at the last segment of `path`.
fn put(parent: &CstNode, path: &[Seg], value: CstInputValue) -> Res<CstNode> {
	let (last, parent_path) = path.split_last().expect("set handles the empty path");
	let created = match last {
		Seg::Key(k) => {
			let object = parent
				.as_object()
				.ok_or_else(|| not_a(parent, parent_path, "an object"))?;
			match object.get(k) {
				Some(prop) => {
					prop.set_value(value);
					prop.value()
				}
				None => object.append(k, value).value(),
			}
		}
		Seg::Index(n) => {
			let array = parent
				.as_array()
				.ok_or_else(|| not_a(parent, parent_path, "an array"))?;
			let elements = array.elements();
			let len = elements.len();
			if *n > len {
				return Err(out_of_range(*n, len, parent_path));
			}
			match elements.get(*n) {
				Some(old) => replace(old.clone(), value),
				None => Some(array.append(value)),
			}
		}
	};
	created.ok_or_else(|| format!("{} could not be created", show(path)))
}

/// Replaces a node in place, keeping the comments around it.
fn replace(node: CstNode, value: CstInputValue) -> Option<CstNode> {
	if let Some(n) = node.as_object() {
		n.replace_with(value)
	} else if let Some(n) = node.as_array() {
		n.replace_with(value)
	} else if let Some(n) = node.as_string_lit() {
		n.replace_with(value)
	} else if let Some(n) = node.as_number_lit() {
		n.replace_with(value)
	} else if let Some(n) = node.as_boolean_lit() {
		n.replace_with(value)
	} else {
		node.as_null_keyword()?.replace_with(value)
	}
}

/// Removes the entry at `path`.
pub fn remove(root: &CstRootNode, path: &[Seg]) -> Res<()> {
	let Some((last, parent_path)) = path.split_last() else {
		return Err("the document root cannot be removed".into());
	};
	let parent = lookup(root, parent_path)?;
	match last {
		Seg::Key(k) => parent
			.as_object()
			.and_then(|o| o.get(k))
			.map(|p| p.remove()),
		Seg::Index(_) => child(&parent, last).map(CstNode::remove),
	}
	.ok_or_else(|| not_found(path))
}

#[cfg(test)]
mod tests {
	use super::*;

	fn key(k: &str) -> Seg {
		Seg::Key(k.into())
	}

	fn path(p: &str) -> Vec<Seg> {
		parse_path(p).unwrap()
	}

	fn get(root: &CstRootNode, p: &str) -> String {
		render(&lookup(root, &path(p)).unwrap())
	}

	#[test]
	fn parses_paths() {
		assert_eq!(path(""), []);
		assert_eq!(path("a.b"), [key("a"), key("b")]);
		assert_eq!(
			path("servers[1].host"),
			[key("servers"), Seg::Index(1), key("host")]
		);
		assert_eq!(path("[0][2]"), [Seg::Index(0), Seg::Index(2)]);
		assert_eq!(path(r#"env."app.mode""#), [key("env"), key("app.mode")]);
		assert_eq!(path(r#""q\"[".x"#), [key("q\"["), key("x")]);
		assert_eq!(path(r#""""#), [key("")]);
		assert_eq!(path("a b.grüße"), [key("a b"), key("grüße")]);
	}

	#[test]
	fn rejects_bad_paths() {
		for bad in [
			"a.", ".a", "a[", "a[]", "a[x]", "a[-1]", "a[0]b", "a[0].", "a..b", "\"open", "a]",
			"a.[0]", "\"a\"b",
		] {
			assert!(parse_path(bad).is_err(), "`{bad}` should not parse");
		}
	}

	#[test]
	fn looks_up_and_renders() {
		let d = parse(
			r#"{
	// comment
	"title": "x\ty", /* block */
	"big": 12345678901234567890,
	"f": 1.5e300,
	"yes": true,
	"none": null,
	"list": [1, {"a": "b"},],
	"o": {"k\"ey": []},
}"#,
		)
		.unwrap();
		assert_eq!(get(&d, "title"), "x\ty");
		assert_eq!(get(&d, "big"), "12345678901234567890");
		assert_eq!(get(&d, "f"), "1.5e300");
		assert_eq!(get(&d, "none"), "null");
		assert_eq!(get(&d, "list"), r#"[1,{"a":"b"}]"#);
		assert_eq!(get(&d, "o"), r#"{"k\"ey":[]}"#);
		assert_eq!(get(&d, r#"o."k\"ey""#), "[]");
		assert_eq!(get(&d, "list[1].a"), "b");

		let ty = |p: &str| type_name(&lookup(&d, &path(p)).unwrap());
		assert_eq!(
			["title", "big", "f", "yes", "none", "list", "o"].map(ty),
			[
				"string", "integer", "float", "boolean", "null", "array", "object"
			]
		);
		assert_eq!(count(&d.value().unwrap(), &[]).unwrap(), 7);
		assert!(lookup(&d, &path("missing")).is_err());
		assert!(lookup(&d, &path("list[2]")).is_err());
		assert!(lookup(&d, &path("title.x")).is_err());
	}

	#[test]
	fn json5_is_rejected() {
		for bad in [
			"{a: 1}", "{'a': 1}", "[0x10]", "[+1]", "[.5]", "[NaN]", "[1 2]",
		] {
			assert!(parse(bad).is_err(), "{bad}");
		}
	}

	#[test]
	fn entries_are_key_and_value() {
		let d = parse(r#"{"a": 1, "b": ["x", "y"]}"#).unwrap();
		let (k, v) = entry(&d.value().unwrap(), &[], 1).unwrap();
		assert_eq!((k.as_str(), render(&v).as_str()), ("b", r#"["x","y"]"#));
		let b = lookup(&d, &path("b")).unwrap();
		let (k, v) = entry(&b, &path("b"), 1).unwrap();
		assert_eq!((k.as_str(), render(&v).as_str()), ("1", "y"));
		assert!(entry(&b, &path("b"), 2).is_err());
	}

	#[test]
	fn replacing_a_value_keeps_comments() {
		let d = parse("{\n  // keep\n  \"host\": \"old\", // me\n  \"port\": 80\n}\n").unwrap();
		set(&d, &path("host"), "new".into()).unwrap();
		assert_eq!(
			d.to_string(),
			"{\n  // keep\n  \"host\": \"new\", // me\n  \"port\": 80\n}\n"
		);
	}

	#[test]
	fn replaces_array_elements_in_place() {
		let d = parse("[\n  1, // one\n  2,\n  3\n]").unwrap();
		set(&d, &path("[0]"), "x".into()).unwrap();
		set(&d, &path("[3]"), 4.into()).unwrap();
		assert_eq!(d.to_string(), "[\n  \"x\", // one\n  2,\n  3,\n  4\n]");
	}

	#[test]
	fn creates_containers_on_demand() {
		let d = parse("").unwrap();
		set(&d, &path("a.b.c"), "x".into()).unwrap();
		set(&d, &path("ports[0]"), 80.into()).unwrap();
		set(&d, &path("ports[1]"), 443.into()).unwrap();
		set(&d, &path("servers[0].host"), "a".into()).unwrap();
		let again = parse(&d.to_string()).unwrap();
		assert_eq!(get(&again, "a.b.c"), "x");
		assert_eq!(get(&again, "ports"), "[80,443]");
		assert_eq!(get(&again, "servers"), r#"[{"host":"a"}]"#);
	}

	#[test]
	fn the_root_can_be_anything() {
		let d = parse("{}").unwrap();
		set(&d, &[], CstInputValue::Array(Vec::new())).unwrap();
		set(&d, &path("[0]"), true.into()).unwrap();
		assert_eq!(get(&d, ""), "[true]");
		set(&d, &[], "plain".into()).unwrap();
		assert_eq!(get(&d, ""), "plain");
	}

	#[test]
	fn bad_writes_are_errors() {
		let d = parse(r#"{"a": 1, "list": [1]}"#).unwrap();
		assert!(set(&d, &path("list[2]"), 0.into()).is_err());
		assert!(set(&d, &path("new[3]"), 0.into()).is_err());
		assert!(set(&d, &path("a.b"), 0.into()).is_err());
		assert!(set(&d, &path("a[0]"), 0.into()).is_err());
	}

	#[test]
	fn crlf_is_kept_for_new_lines() {
		let d = parse("{\r\n  \"a\": 1\r\n}\r\n").unwrap();
		set(&d, &path("b"), 2.into()).unwrap();
		set(&d, &path("c.d"), 3.into()).unwrap();
		let text = d.to_string();
		assert!(!text.replace("\r\n", "").contains('\n'), "{text:?}");
	}

	#[test]
	fn removes() {
		let d = parse(r#"{"a": 1, "b": [1, 2], "c": 3}"#).unwrap();
		remove(&d, &path("a")).unwrap();
		remove(&d, &path("b[0]")).unwrap();
		assert!(remove(&d, &path("a")).is_err());
		assert!(remove(&d, &path("b[1]")).is_err());
		assert!(remove(&d, &path("zz.y")).is_err());
		assert!(remove(&d, &[]).is_err());
		assert_eq!(get(&d, ""), r#"{"b":[2],"c":3}"#);
	}

	#[test]
	fn literals_are_json() {
		let d = parse("{}").unwrap();
		set(&d, &path("x"), literal(r#" [1, {"a": null}] "#).unwrap()).unwrap();
		assert_eq!(get(&d, "x"), r#"[1,{"a":null}]"#);
		assert!(literal("bare words").is_err());
		assert!(literal("").is_err());
	}

	#[test]
	fn integers_follow_nsis() {
		assert_eq!(str_to_i64("42"), 42);
		assert_eq!(str_to_i64("-42"), -42);
		assert_eq!(str_to_i64("0x7fffffffff"), 0x7f_ffff_ffff);
		assert_eq!(str_to_i64("0755"), 0o755);
		assert_eq!(str_to_i64("abc"), 0);
	}
}
