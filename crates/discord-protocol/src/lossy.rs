//! Login snapshots degrade per element: one malformed channel, DM, contact or member row is
//! dropped (and reported as a startup warning) instead of rejecting the whole account.
use serde::{
	Deserialize, Deserializer,
	de::{SeqAccess, Visitor},
};
use serde_json::value::RawValue;
use std::marker::PhantomData;

/// `#[serde(default)]` covers an absent field; this also maps an explicit `null` to the default.
pub(crate) fn null_default<'de, D: Deserializer<'de>, T: Default + Deserialize<'de>>(
	d: D,
) -> Result<T, D::Error> {
	Option::<T>::deserialize(d).map(Option::unwrap_or_default)
}

/// Permission bit strings; some payloads send the same value as a JSON number.
pub(crate) fn text_or_number<'de, D: Deserializer<'de>>(d: D) -> Result<String, D::Error> {
	struct Text;
	impl Visitor<'_> for Text {
		type Value = String;
		fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
			f.write_str("a string or unsigned integer")
		}
		fn visit_str<E: serde::de::Error>(self, value: &str) -> Result<String, E> {
			Ok(value.to_owned())
		}
		fn visit_u64<E: serde::de::Error>(self, value: u64) -> Result<String, E> {
			Ok(value.to_string())
		}
	}
	d.deserialize_any(Text)
}

pub(crate) fn recipients<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<crate::UserDto>, D::Error> {
	Lossy::<crate::UserDto, 64, true>::deserialize(d).map(|rows| rows.items)
}

trait BoundedList<'de, T>: Default {
	const LIMIT: usize;
	const TRUNCATE: bool;
	fn reserve(&mut self, count: usize);
	fn push_parsed(&mut self, value: Option<T>);
	fn mark_truncated(&mut self);
}

fn deserialize_bounded<'de, D: Deserializer<'de>, T: Deserialize<'de>, L: BoundedList<'de, T>>(
	d: D,
) -> Result<L, D::Error> {
	struct Elements<L, T>(PhantomData<(L, T)>);
	impl<'de, T: Deserialize<'de>, L: BoundedList<'de, T>> Visitor<'de> for Elements<L, T> {
		type Value = L;
		fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
			f.write_str("a bounded list")
		}
		fn visit_unit<E>(self) -> Result<Self::Value, E> {
			Ok(L::default())
		}
		fn visit_none<E>(self) -> Result<Self::Value, E> {
			Ok(L::default())
		}
		fn visit_some<D: Deserializer<'de>>(self, d: D) -> Result<Self::Value, D::Error> {
			d.deserialize_seq(self)
		}
		fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
			let mut out = L::default();
			if let Some(hint) = seq.size_hint() {
				out.reserve(hint.min(L::LIMIT).min(1024));
			}
			let mut seen = 0_usize;
			while let Some(raw) = seq.next_element::<&'de RawValue>()? {
				if seen == L::LIMIT {
					if L::TRUNCATE {
						out.mark_truncated();
						continue;
					}
					return Err(serde::de::Error::custom("List capacity exceeded"));
				}
				seen += 1;
				out.push_parsed(serde_json::from_str(raw.get()).ok());
			}
			Ok(out)
		}
	}
	d.deserialize_option(Elements::<L, T>(PhantomData))
}

/// Positions are kept: a malformed element becomes `None`, so index-aligned lists
/// (`merged_members`) still line up. More than `N` elements is an error unless `TRUNCATE`.
/// Elements are split as borrowed raw values, so only use it on borrowed JSON input.
pub(crate) struct Slots<T, const N: usize, const TRUNCATE: bool = false> {
	pub(crate) items: Vec<Option<T>>,
	pub(crate) truncated: bool,
}
impl<T, const N: usize, const TRUNCATE: bool> Default for Slots<T, N, TRUNCATE> {
	fn default() -> Self {
		Self {
			items: Vec::new(),
			truncated: false,
		}
	}
}
impl<'de, T: Deserialize<'de>, const N: usize, const TRUNCATE: bool> BoundedList<'de, T>
	for Slots<T, N, TRUNCATE>
{
	const LIMIT: usize = N;
	const TRUNCATE: bool = TRUNCATE;
	fn reserve(&mut self, count: usize) {
		self.items.reserve(count);
	}
	fn push_parsed(&mut self, value: Option<T>) {
		self.items.push(value);
	}
	fn mark_truncated(&mut self) {
		self.truncated = true;
	}
}
impl<'de, T: Deserialize<'de>, const N: usize, const TRUNCATE: bool> Deserialize<'de>
	for Slots<T, N, TRUNCATE>
{
	fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
		deserialize_bounded::<D, T, Self>(d)
	}
}

/// Malformed elements are dropped; `skipped` reports that anything was lost.
pub(crate) struct Lossy<T, const N: usize, const TRUNCATE: bool = false> {
	pub(crate) items: Vec<T>,
	pub(crate) skipped: bool,
}
impl<T, const N: usize, const TRUNCATE: bool> Default for Lossy<T, N, TRUNCATE> {
	fn default() -> Self {
		Self {
			items: Vec::new(),
			skipped: false,
		}
	}
}
impl<'de, T: Deserialize<'de>, const N: usize, const TRUNCATE: bool> BoundedList<'de, T>
	for Lossy<T, N, TRUNCATE>
{
	const LIMIT: usize = N;
	const TRUNCATE: bool = TRUNCATE;
	fn reserve(&mut self, count: usize) {
		self.items.reserve(count);
	}
	fn push_parsed(&mut self, value: Option<T>) {
		match value {
			Some(item) => self.items.push(item),
			None => self.skipped = true,
		}
	}
	fn mark_truncated(&mut self) {
		self.skipped = true;
	}
}
impl<'de, T: Deserialize<'de>, const N: usize, const TRUNCATE: bool> Deserialize<'de>
	for Lossy<T, N, TRUNCATE>
{
	fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
		deserialize_bounded::<D, T, Self>(d)
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[derive(Deserialize)]
	struct Row {
		id: u8,
		#[serde(default, deserialize_with = "null_default")]
		flag: bool,
	}
	#[derive(Deserialize)]
	struct Rows {
		#[serde(default)]
		rows: Lossy<Row, 3>,
		#[serde(default)]
		slots: Slots<Row, 3>,
		#[serde(default)]
		capped: Lossy<Row, 1, true>,
	}

	#[test]
	fn drops_only_malformed_elements_and_keeps_positions_in_slots() {
		let rows: Rows = serde_json::from_slice(
			br#"{"rows":[{"id":1,"flag":null},{"id":"bad"},{"id":3}],"slots":[{"id":"bad"},{"id":2}],"capped":[{"id":1},{"id":2}]}"#,
		)
		.unwrap();
		assert_eq!(
			rows.rows.items.iter().map(|r| r.id).collect::<Vec<_>>(),
			[1, 3]
		);
		assert!(rows.rows.skipped && !rows.rows.items[0].flag);
		assert!(rows.slots.items[0].is_none() && rows.slots.items[1].is_some());
		assert!(rows.capped.skipped && rows.capped.items.len() == 1);
		let rows: Rows = serde_json::from_slice(br#"{"rows":null}"#).unwrap();
		assert!(rows.rows.items.is_empty() && !rows.rows.skipped);
		assert!(
			serde_json::from_slice::<Rows>(br#"{"rows":[{"id":1},{"id":2},{"id":3},{"id":4}]}"#)
				.is_err()
		);
	}
}
